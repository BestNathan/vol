# Listener Concurrency & Async Wait Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace fire-and-forget listener spawns with per-plugin persistent tasks tracked via JoinSet, with clean shutdown through Arc-wrapped broadcast sender.

**Architecture:** Wrap `RunContext.event_tx` in `Arc` so that cloning `RunContext` only copies the Arc pointer (no new sender). Listener tasks subscribe to the broadcast and process events sequentially. After agent completes, dropping the original sender closes the broadcast — all listeners see `RecvError` and exit naturally.

**Tech Stack:** Rust, tokio (broadcast channel, JoinSet), async_trait

---

### Task 1: Wrap `event_tx` in `Arc` in `RunContext` and update `run_interceptor_loop` signature

**Files:**
- Modify: `crates/vol-llm-agent/src/react/run_context.rs:59`
- Modify: `crates/vol-llm-agent/src/react/plugin_stream.rs:54-58` (`run_interceptor_loop` signature)
- Test: `crates/vol-llm-agent/src/react/run_context.rs` (existing tests)

- [ ] **Step 1: Change `event_tx` type to `Arc<broadcast::Sender<TracedEvent<AgentStreamEvent>>>`**

In `run_context.rs`, update the struct field:

```rust
// Line 59: change from:
// pub event_tx: broadcast::Sender<TracedEvent<AgentStreamEvent>>,
// to:
pub event_tx: Arc<broadcast::Sender<TracedEvent<AgentStreamEvent>>>,
```

- [ ] **Step 2: Update `RunContext::new()` to wrap in `Arc`**

```rust
// In new(), line 125, change from:
// let (event_tx, _) = broadcast::channel(1024);
// to:
let (event_tx, _) = broadcast::channel::<TracedEvent<AgentStreamEvent>>(1024);
let event_tx = Arc::new(event_tx);
```

- [ ] **Step 3: Update `Clone` impl for `RunContext`**

The clone already does `event_tx: self.event_tx.clone()` (line 393). Since `event_tx` is now `Arc<Sender>`, `Arc::clone` copies the pointer — no new sender is created. No code change needed, but update the doc comment to clarify:

```rust
// Update doc comment around lines 92-100:
/// ## Broadcast Channel Close Sequence
///
/// The `event_tx` is wrapped in `Arc`, so cloning `RunContext` only copies the
/// Arc pointer — it does NOT create new broadcast senders. The sender count
/// is exactly 1, held by the Arc in the original `RunContext`.
/// When this Arc is dropped (via `std::mem::take` in agent.rs), the broadcast
/// closes and all listeners see `RecvError`, exiting cleanly.
```

- [ ] **Step 4: Update `emit()` to dereference through Arc**

The `emit()` method at line 278-281 uses `self.event_tx.send(...)` — this works unchanged since `Arc` implements `Deref`. No code change needed.

- [ ] **Step 5: Update `run_interceptor_loop` signature in `plugin_stream.rs`**

The function currently takes `event_tx: broadcast::Sender<TracedEvent<AgentStreamEvent>>`. Change it to accept `Arc<broadcast::Sender<TracedEvent<AgentStreamEvent>>>`:

```rust
// plugin_stream.rs, line 54-58:
// Change from:
// pub async fn run_interceptor_loop(
//     mut plugin_rx: mpsc::Receiver<PluginRequest>,
//     plugins: Vec<Arc<dyn AgentPlugin>>,
//     event_tx: broadcast::Sender<TracedEvent<AgentStreamEvent>>,
//     ctx: RunContext,
// ) {
// To:
pub async fn run_interceptor_loop(
    mut plugin_rx: mpsc::Receiver<PluginRequest>,
    plugins: Vec<Arc<dyn AgentPlugin>>,
    event_tx: Arc<broadcast::Sender<TracedEvent<AgentStreamEvent>>>,
    ctx: RunContext,
) {
```

The body uses `event_tx.send(...)` which works through `Arc::Deref` — no body changes needed.

- [ ] **Step 6: Run existing tests to verify**

Run: `cargo test -p vol-llm-agent react::run_context::` 
Expected: All tests pass (no behavioral change for existing tests)

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-agent/src/react/run_context.rs crates/vol-llm-agent/src/react/plugin_stream.rs
git commit -m "refactor: wrap event_tx in Arc; update run_interceptor_loop signature"
```

---

### Task 2: Replace `spawn_listener_task` with `spawn_listener_tasks`

**Files:**
- Modify: `crates/vol-llm-agent/src/react/plugin_stream.rs`
- Modify: `crates/vol-llm-agent/src/react/mod.rs` (update exports)
- Test: `crates/vol-llm-agent/src/react/tests.rs` (existing plugin_stream tests)

- [ ] **Step 1: Write `spawn_listener_tasks` in `plugin_stream.rs`**

Add the new function (keep `spawn_listener_task` for backwards compat, or replace it):

```rust
/// Spawn one listener task per plugin, each subscribing to the event broadcast
/// channel and processing events sequentially.
///
/// Each task exits when the broadcast channel closes (all senders dropped),
/// guaranteeing all buffered events are processed before exit.
///
/// Returns a `JoinSet` that tracks all listener tasks for await.
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

- [ ] **Step 2: Remove the old `spawn_listener_task` function**

Delete the entire `spawn_listener_task` function from `plugin_stream.rs`. The new `spawn_listener_tasks` replaces it entirely.

- [ ] **Step 3: Update exports in `mod.rs`**

In `crates/vol-llm-agent/src/react/mod.rs`, change:

```rust
// From:
pub use plugin_stream::{
    create_shortcircuit_stream, create_skip_stream, run_interceptor_loop, spawn_listener_task,
};
// To:
pub use plugin_stream::{
    create_shortcircuit_stream, create_skip_stream, run_interceptor_loop, spawn_listener_tasks,
};
```

- [ ] **Step 4: Run existing tests**

Run: `cargo test -p vol-llm-agent react::tests::`
Expected: All tests pass (interceptor tests still work)

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-agent/src/react/plugin_stream.rs crates/vol-llm-agent/src/react/mod.rs
git commit -m "feat: replace spawn_listener_task with spawn_listener_tasks using JoinSet"
```

---

### Task 3: Update `agent.rs` to use `spawn_listener_tasks` and remove timeout

**Files:**
- Modify: `crates/vol-llm-agent/src/react/agent.rs:197-556`
- Test: `crates/vol-llm-agent/tests/agent_run_tests.rs`
- Test: `crates/vol-llm-agent/tests/code_agent_simulation.rs`

- [ ] **Step 1: Update listener spawning in `agent.rs`**

Replace lines 197-228 (Phase 2.6 listener + interceptor spawn) with:

```rust
// === Phase 2.6: Spawn listener and interceptor tasks ===
use super::plugin_stream::{run_interceptor_loop, spawn_listener_tasks};

// Build plugin list from registry only
let plugins = config.plugin_registry.plugins().to_vec();

// Spawn listener tasks — one per plugin, tracked in JoinSet
let mut listener_set = spawn_listener_tasks(plugins, run_ctx.clone());

// Spawn interceptor loop task
let interceptor_event_tx = run_ctx.event_tx.clone(); // Arc clone — cheap
let interceptor_plugins = config.plugin_registry.plugins().to_vec();
let interceptor_ctx = run_ctx.clone();
let interceptor_handle = tokio::spawn(async move {
    run_interceptor_loop(
        plugin_rx,
        interceptor_plugins,
        interceptor_event_tx, // Arc<Sender>, passed directly
        interceptor_ctx,
    )
    .await;
});
```

- [ ] **Step 2: Replace timeout-based shutdown with JoinSet await**

Replace lines 526-556 (the timeout-based wait for interceptor and listener) with:

```rust
// Drop the original event_tx sender to close the broadcast channel.
// Listener tasks (which only have Arc clones) will see RecvError and exit.
let _ = std::mem::take(&mut run_ctx.event_tx);

// Await all listener tasks — they drain buffers and exit naturally
while let Some(result) = listener_set.join_next().await {
    if let Err(e) = result {
        tracing::warn!(%e, "Listener task panicked");
    }
}

// Wait for interceptor with short timeout (it uses mpsc, not broadcast)
let interceptor_result =
    tokio::time::timeout(std::time::Duration::from_secs(5), interceptor_handle).await;
match interceptor_result {
    Ok(Ok(())) => {}
    Ok(Err(join_err)) => {
        tracing::warn!(%join_err, "Interceptor task panicked");
    }
    Err(_timeout) => {
        tracing::warn!(
            "Interceptor task timeout after 5s - task may be hanging, proceeding anyway"
        );
    }
}
```

Note: The interceptor still uses a timeout because it communicates via `mpsc::Receiver<PluginRequest>`, not broadcast. The listener no longer needs a timeout — it exits via broadcast close.

- [ ] **Step 3: Run the full test suite**

Run: `cargo test -p vol-llm-agent`
Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent/src/react/agent.rs
git commit -m "feat: use JoinSet for listener shutdown, remove listener timeout"
```

---

### Task 4: Update integration tests for new API

**Files:**
- Modify: `crates/vol-llm-agent/tests/agent_run_tests.rs`
- Modify: `crates/vol-llm-agent/tests/code_agent_simulation.rs`
- Modify: `crates/vol-llm-agent/tests/react_mock_test.rs`
- Modify: `crates/vol-llm-agent/tests/plugin_flow_test.rs`

- [ ] **Step 1: Update any test that references `spawn_listener_task`**

Search for `spawn_listener_task` in test files:

```bash
grep -rn "spawn_listener_task" crates/vol-llm-agent/tests/
```

If any tests directly call the old function, update to `spawn_listener_tasks`.

- [ ] **Step 2: Add test for listener processes all events**

Add to `crates/vol-llm-agent/tests/agent_run_tests.rs` following the existing `CountingPlugin` pattern (lines 134-150):

```rust
#[tokio::test]
async fn test_listener_processes_all_tool_call_events() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use vol_llm_agent::react::plugin::AgentPlugin;
    use vol_llm_agent::react::plugin::{PluginDecision, PluginId};

    struct EventCounter {
        count: Arc<AtomicUsize>,
    }

    #[async_trait::async_trait]
    impl AgentPlugin for EventCounter {
        fn id(&self) -> PluginId { "counter".to_string() }
        fn priority(&self) -> u32 { 100 }
        async fn intercept(&self, _: &AgentStreamEvent, _: &RunContext) -> PluginDecision {
            PluginDecision::Continue
        }
        async fn listen(&self, event: &AgentStreamEvent, _: &RunContext) {
            if matches!(event, AgentStreamEvent::ToolCallBegin { .. }) {
                self.count.fetch_add(1, Ordering::SeqCst);
            }
        }
    }

    let counter = Arc::new(AtomicUsize::new(0));
    let plugin = EventCounter { count: counter.clone() };

    let mut registry = PluginRegistry::new();
    registry.register(plugin);

    let (mock, _call_count) = MultiCallMock::new();

    let config = AgentConfig {
        llm: Arc::new(mock),
        plugin_registry: registry,
        ..Default::default()
    };

    let agent = ReActAgent::new(config);
    let _result = agent.run("do something").await;

    // Listener should have processed ToolCallBegin events before run() returned
    assert!(counter.load(Ordering::SeqCst) > 0,
        "Listener should have processed ToolCallBegin events");
}
```

- [ ] **Step 3: Run full test suite**

Run: `cargo test -p vol-llm-agent`
Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent/tests/agent_run_tests.rs
git commit -m "test: add listener event processing test"
```

---

### Task 5: Add test for `spawn_listener_tasks` shutdown behavior

**Files:**
- Modify: `crates/vol-llm-agent/src/react/tests.rs`

- [ ] **Step 1: Add test for broadcast close shutdown**

Add to `react/tests.rs`:

```rust
#[tokio::test]
async fn test_spawn_listener_tasks_shutdown_on_close() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct ListenPlugin {
        count: Arc<AtomicUsize>,
    }
    #[async_trait::async_trait]
    impl plugin::AgentPlugin for ListenPlugin {
        fn id(&self) -> plugin::PluginId { "listen".to_string() }
        fn priority(&self) -> u32 { 100 }
        async fn intercept(&self, _: &AgentStreamEvent, _: &RunContext) -> plugin::PluginDecision {
            plugin::PluginDecision::Continue
        }
        async fn listen(&self, _: &AgentStreamEvent, _: &RunContext) {
            self.count.fetch_add(1, Ordering::SeqCst);
        }
    }

    let (event_tx, _) = tokio::sync::broadcast::channel(1024);
    let event_tx = Arc::new(event_tx);
    let (run_ctx, _rx) = RunContext::new(
        "test".to_string(),
        "test".to_string(),
        AgentConfig::default(),
    );

    let count1 = Arc::new(AtomicUsize::new(0));
    let count2 = Arc::new(AtomicUsize::new(0));

    let plugins: Vec<Arc<dyn plugin::AgentPlugin>> = vec![
        Arc::new(ListenPlugin { count: count1.clone() }),
        Arc::new(ListenPlugin { count: count2.clone() }),
    ];

    // Emit a few events BEFORE spawning listeners
    event_tx.send(vol_tracing::TracedEvent::without_span(
        AgentStreamEvent::agent_start("test".to_string())
    )).unwrap();

    // Spawn listeners
    let mut listener_set = spawn_listener_tasks(plugins, run_ctx.clone());

    // Emit more events while listeners are running
    event_tx.send(vol_tracing::TracedEvent::without_span(
        AgentStreamEvent::agent_complete_with_response(serde_json::json!({}))
    )).unwrap();

    // Give listeners a moment to process
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Drop sender — this should close broadcast and trigger listener exit
    drop(event_tx);

    // Wait for all listeners to exit
    while let Some(result) = listener_set.join_next().await {
        assert!(result.is_ok(), "Listener task should not panic");
    }

    // Both plugins should have processed events
    assert!(count1.load(Ordering::SeqCst) > 0);
    assert!(count2.load(Ordering::SeqCst) > 0);
}
```

- [ ] **Step 2: Add `use std::time::Duration` if needed**

Add import at the top of `tests.rs`:

```rust
use std::time::Duration;
```

- [ ] **Step 3: Run the new test**

Run: `cargo test -p vol-llm-agent react::tests::test_spawn_listener_tasks_shutdown_on_close`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent/src/react/tests.rs
git commit -m "test: verify listener tasks exit cleanly on broadcast close"
```

---

### Task 6: Wiki update and Feishu sync

**Files:**
- Update wiki at `docs/wiki/`

- [ ] **Step 1: Run wiki-ingest skill to update project wiki**

Use the `wiki-ingest` skill to document the listener concurrency changes.

- [ ] **Step 2: Upload spec to Feishu wiki**

Run:
```bash
lark-cli docs +update \
    --new-title "Listener Concurrency and Async Wait Design" \
    --mode overwrite \
    --markdown "$(cat docs/superpowers/specs/2026-05-17-listener-concurrency-design.md)" \
    --doc "OWtYwOjwtiOAPpk6kPzcIKVKnBb"
```

- [ ] **Step 3: Commit**

```bash
git add docs/wiki/
git commit -m "docs: update wiki for listener concurrency changes"
```
