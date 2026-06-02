---
name: Listener Concurrency and Async Wait Design
description: Per-plugin broadcast-subscribe listener tasks with Arc-wrapped sender for clean shutdown
type: project
---

# Design: Per-Plugin Listener Tasks with Arc-Wrapped Sender

**Date:** 2026-05-17
**Status:** Proposed

## Problem

Current `spawn_listener_task` in `plugin_stream.rs` creates fire-and-forget `tokio::spawn` calls for every event × plugin combination. Issues:

1. **No concurrency limit** — 10 plugins × 50 events = 500 concurrent tasks
2. **No completion tracking** — fire-and-forget, unknown if listeners finished or failed
3. **Imprecise shutdown** — relies on 5s timeout, may drop events still being processed

## Root Cause

`RunContext.event_tx` is a `broadcast::Sender`. Cloning it creates a new sender, incrementing the sender count. Listener tasks that clone `RunContext` inadvertently become senders, preventing the broadcast from closing.

## Design

### Architecture

Wrap `event_tx` in `Arc` in `RunContext`. Cloning `RunContext` clones the `Arc` (cheap pointer clone), not the sender. Only dropping the original sender closes the broadcast. Listener tasks subscribe via `event_tx.subscribe()` and process events sequentially in a `JoinSet`.

```
Arc<Sender> in run_ctx ─── Arc pointer clone (no sender increment)
              │
  event_tx.subscribe() ──→ Receiver (only receives, no sender)
              │
  listener task: loop recv() → plugin.listen() → repeat
              │
  agent_task completes → drop original sender → broadcast closes
  listeners see RecvError → drain buffer → exit
  JoinSet.await_all() → run() returns
```

### Key Changes

#### 1. `RunContext.event_tx` → `Arc<broadcast::Sender<...>>`

```rust
// run_context.rs
pub struct RunContext {
    // ...
    pub event_tx: Arc<broadcast::Sender<TracedEvent<AgentStreamEvent>>>,
    // ...
}
```

Construction:
```rust
let (event_tx, _) = broadcast::channel(1024);
// wrap in Arc
event_tx: Arc::new(event_tx),
```

Emit (unchanged behavior):
```rust
pub async fn emit(&self, event: AgentStreamEvent) {
    let traced_event = TracedEvent::new(event);
    let _ = self.event_tx.send(traced_event);
}
```

Clone (no sender increment):
```rust
fn clone(&self) -> Self {
    Self {
        // ...
        event_tx: self.event_tx.clone(), // Arc clone — cheap pointer copy
        // ...
    }
}
```

#### 2. `spawn_listener_task` → `spawn_listener_tasks`

```rust
pub fn spawn_listener_tasks(
    plugins: Vec<Arc<dyn AgentPlugin>>,
    ctx: RunContext,
) -> tokio::task::JoinSet<()> {
    let mut join_set = tokio::task::JoinSet::new();
    for plugin in plugins {
        let mut event_rx = ctx.event_tx.subscribe();
        let plugin = plugin.clone();
        let ctx = ctx.clone();
        join_set.spawn(async move {
            while let Ok(traced_event) = event_rx.recv().await {
                let event = traced_event.value();
                let _ = std::panic::AssertUnwindSafe(
                    plugin.listen(&event, &ctx)
                ).catch_unwind().await;
            }
        });
    }
    join_set
}
```

- One task per plugin, sequential processing
- Each task subscribes to broadcast via `event_tx.subscribe()`
- No inner `tokio::spawn` — just the task's own async loop
- `ctx.clone()` only clones the `Arc` pointer — doesn't create new senders
- Returns `JoinSet<()>` for tracking

#### 3. Shutdown in `agent.rs`

```rust
// Spawn listener tasks
let plugins = config.plugin_registry.plugins().to_vec();
let mut listener_set = spawn_listener_tasks(plugins, run_ctx.clone());

// ... agent_task with run_ctx.clone() ...

let agent_result = agent_task.await?;

// Shutdown: drop the original sender
drop(std::mem::take(&mut run_ctx.event_tx));

// Await all listener tasks
while let Some(result) = listener_set.join_next().await {
    if let Err(e) = result {
        tracing::warn!(%e, "Listener task panicked");
    }
}
```

**Shutdown sequence:**
1. `agent_task` completes — drops its `Arc` clone (no sender drop, just Arc refcount -1)
2. Interceptor exits — drops its `Arc` clone
3. `drop(std::mem::take(&mut run_ctx.event_tx))` — drops the **last Arc**, which drops the sender
4. Broadcast closes — all receivers see `RecvError`
5. Listeners exit their loops
6. `join_next().await()` returns for each task
7. `run()` returns

No timeout needed. All buffered events processed before exit.

#### 4. Remove 5s timeout

The old 5s timeout on listener/interceptor join handles is replaced by natural broadcast close + JoinSet await.

### Error Handling

- Each `plugin.listen()` wrapped in `catch_unwind`, logs panic, continues processing
- Individual listener failures don't affect other listeners or agent flow

### Concurrency Model

- Concurrency limit = number of plugins (one task each)
- Events processed sequentially within each plugin's task
- Natural backpressure via broadcast channel capacity

### Files Modified

- `crates/vol-llm-agent/src/react/run_context.rs` — `event_tx` → `Arc<Sender>`, update construction/clone
- `crates/vol-llm-agent/src/react/plugin_stream.rs` — new `spawn_listener_tasks` function
- `crates/vol-llm-agent/src/react/agent.rs` — use JoinSet, drop sender for shutdown, remove timeout
- `crates/vol-llm-agent/src/react/mod.rs` — update exports
- `crates/vol-llm-agent/src/react/tests.rs` — update tests to use new API
