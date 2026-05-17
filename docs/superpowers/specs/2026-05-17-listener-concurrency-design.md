---
name: Listener Concurrency and Async Wait Design
description: Per-plugin broadcast-subscribe listener tasks with JoinSet tracking for precise shutdown
type: project
---

# Design: Per-Plugin Listener Tasks with Broadcast Subscribe

**Date:** 2026-05-17
**Status:** Proposed

## Problem

Current `spawn_listener_task` in `plugin_stream.rs` creates fire-and-forget `tokio::spawn` calls for every event × plugin combination. Issues:

1. **No concurrency limit** — 10 plugins × 50 events = 500 concurrent tasks
2. **No completion tracking** — fire-and-forget, unknown if listeners finished or failed
3. **Imprecise shutdown** — relies on 5s timeout, may drop events still being processed

## Design

### Architecture

Each plugin gets one persistent listener task that subscribes to the broadcast channel and processes events sequentially. All listener tasks are tracked in a `JoinSet`, and `run()` awaits all of them before returning.

```
Agent run()
  ├── broadcast channel (existing)
  ├── for each plugin:
  │     subscribe broadcast → spawn listener task (JoinSet)
  │         loop: recv event → plugin.listen(event) → repeat
  ├── agent_task.await()
  ├── drop broadcast sender → all receivers see close
  ├── JoinSet.await_all() → wait for all listeners
  └── run() returns (all events fully processed)
```

### Key Changes

#### 1. `spawn_listener_task` → `spawn_listener_tasks`

```rust
pub fn spawn_listener_tasks(
    plugins: Vec<Arc<dyn AgentPlugin>>,
    ctx: RunContext,
) -> tokio::task::JoinSet<()> {
    let mut join_set = tokio::task::JoinSet::new();
    for plugin in plugins {
        let mut event_rx = ctx.event_tx.subscribe();
        // Clone event_tx for this task's shutdown participation.
        // When the broadcast closes (RecvError), we drop this sender
        // to help close the channel completely.
        let event_tx = ctx.event_tx.clone();
        let plugin = plugin.clone();
        let ctx = ctx.clone();
        join_set.spawn(async move {
            while let Ok(traced_event) = event_rx.recv().await {
                let event = traced_event.value();
                let _ = std::panic::AssertUnwindSafe(plugin.listen(&event, &ctx))
                    .catch_unwind()
                    .await;
            }
            // Drop our sender clone — when all tasks do this, channel closes
            drop(event_tx);
        });
    }
    join_set
}
```

- One task per plugin, not one task per event
- Each task subscribes to broadcast channel
- Sequential processing within each task (no inner `tokio::spawn`)
- Returns `JoinSet<()>` for tracking

#### 2. Shutdown in `agent.rs`

```rust
// Create JoinSet instead of single JoinHandle
let mut listener_set = spawn_listener_tasks(plugins, run_ctx.clone());

// ... agent_task.await() ...

// Shutdown: drop broadcast sender first
drop(run_ctx.event_tx.clone()); // drop any remaining clones
// After agent_task and interceptor exit, the only remaining senders
// are in the listener tasks. They drop their senders when the loop
// exits (broadcast RecvError), causing channel close and final exit.
```

**Shutdown sequence:**
1. `agent_task` completes — drops its `run_ctx` clone (including its `event_tx` clone)
2. Interceptor exits — drops its `event_tx` clone
3. Listener tasks receive `RecvError` from broadcast — exit their loops
4. Listener tasks drop their `event_tx` clones — last senders gone, channel fully closed
5. `join_next().await()` returns for each task — all listeners confirmed done
6. `run()` returns

No timeout needed — shutdown is natural and precise.

#### 3. `RunContext` clone for listeners

Listener tasks need access to `event_tx` to participate in the shutdown signal chain, but only as a clone. The `spawn_listener_tasks` function creates a fresh `RunContext` clone for each plugin task, ensuring each has its own `event_tx` handle that gets dropped when the task exits.

#### 4. Remove 5s timeout

No longer needed — broadcast channel close causes all listener tasks to exit naturally. All tasks complete before `run()` returns.

### Error Handling

- Each `plugin.listen()` wrapped in `catch_unwind`, logs panic, continues processing
- Individual listener failures don't affect other listeners or agent flow
- Panicked tasks logged as warnings

### Concurrency Model

- Concurrency limit = number of plugins (one task each)
- Events processed sequentially within each plugin's task
- Natural backpressure via broadcast channel capacity

### Files Modified

- `crates/vol-llm-agent/src/react/plugin_stream.rs` — new `spawn_listener_tasks` function
- `crates/vol-llm-agent/src/react/agent.rs` — use JoinSet, remove timeout, update shutdown sequence
- `crates/vol-llm-agent/src/react/mod.rs` — update exports
- `crates/vol-llm-agent/src/react/tests.rs` — update tests to use new API
