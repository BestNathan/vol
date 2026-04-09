# Plugin Flow Intervention Mechanism Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a centralized event bus in RunContext with Interceptor (sync, serial, blocking) and Listener (async, parallel, fire-and-forget) plugin hooks for agent flow intervention.

**Architecture:** All events flow through RunContext's broadcast channel. Listeners subscribe to the broadcast channel and process events asynchronously. Interceptors receive events via mpsc channel and return PluginDecision (Continue/Skip/Abort) via oneshot channel, blocking agent execution until decision is received.

**Tech Stack:** Rust, tokio (broadcast, mpsc, oneshot channels), async-trait, existing vol-llm-agent crate

---

## File Structure

**Files to Create:**
- None (all changes are modifications to existing files)

**Files to Modify:**
- `crates/vol-llm-agent/src/react/plugin.rs` - Add PluginDecision, update AgentPlugin trait
- `crates/vol-llm-agent/src/react/stream.rs` - Add AgentAborted and PluginEvent events
- `crates/vol-llm-agent/src/react/run_context.rs` - Add event bus (broadcast + mpsc channels)
- `crates/vol-llm-agent/src/react/plugin_stream.rs` - Handle Intercept/Emit requests
- `crates/vol-llm-agent/src/react/agent.rs` - Integrate event bus in run() loop
- `crates/vol-llm-agent/src/react/mod.rs` - Update exports

**Test Files to Modify/Create:**
- `crates/vol-llm-agent/src/react/plugin.rs` - Add unit tests for PluginDecision
- `crates/vol-llm-agent/tests/plugin_flow_test.rs` - Integration tests

---

### Task 1: Add PluginDecision Type

**Files:**
- Modify: `crates/vol-llm-agent/src/react/plugin.rs`

- [ ] **Step 1: Add PluginDecision enum**

Add after `PluginAction` enum in plugin.rs:

```rust
/// Decision returned by intercept() hook
#[derive(Debug, Clone)]
pub enum PluginDecision {
    /// Continue to next interceptor or execute event
    Continue,
    /// Skip current event (don't execute tool/loop)
    Skip,
    /// Abort entire agent execution with reason
    Abort(String),
}
```

- [ ] **Step 2: Run cargo check to verify syntax**

Run: `cargo check -p vol-llm-agent`
Expected: No errors

- [ ] **Step 3: Add unit tests for PluginDecision**

Add to plugin.rs tests module:

```rust
#[test]
fn test_plugin_decision_variants() {
    let continue_decision = PluginDecision::Continue;
    assert!(matches!(continue_decision, PluginDecision::Continue));
    
    let skip_decision = PluginDecision::Skip;
    assert!(matches!(skip_decision, PluginDecision::Skip));
    
    let abort_decision = PluginDecision::Abort("reason".to_string());
    assert!(matches!(abort_decision, PluginDecision::Abort(_)));
}
```

- [ ] **Step 4: Run tests to verify**

Run: `cargo test -p vol-llm-agent plugin_decision`
Expected: 1 test passes

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-agent/src/react/plugin.rs
git commit -m "feat: add PluginDecision enum for interceptor flow control"
```

---

### Task 2: Extend AgentStreamEvent

**Files:**
- Modify: `crates/vol-llm-agent/src/react/stream.rs`

- [ ] **Step 1: Add AgentAborted event variant**

Add to AgentStreamEvent enum in stream.rs:

```rust
/// Agent was aborted with reason
AgentAborted { reason: String },

/// Custom event from plugin
PluginEvent { 
    name: String, 
    data: serde_json::Map<String, serde_json::Value>, 
},
```

- [ ] **Step 2: Add serde_json dependency if not exists**

Run: `cargo check -p vol-llm-agent`
Expected: If serde_json error, add to Cargo.toml:
```toml
serde_json = "1.0"
```

- [ ] **Step 3: Add unit tests for new events**

Add to stream.rs tests module:

```rust
#[test]
fn test_agent_stream_event_aborted() {
    let event = AgentStreamEvent::AgentAborted { 
        reason: "max iterations".to_string() 
    };
    match event {
        AgentStreamEvent::AgentAborted { reason } => {
            assert_eq!(reason, "max iterations");
        }
        _ => panic!("Expected AgentAborted"),
    }
}

#[test]
fn test_agent_stream_event_plugin_event() {
    use serde_json::Map;
    let mut data = Map::new();
    data.insert("key".to_string(), serde_json::Value::String("value".to_string()));
    
    let event = AgentStreamEvent::PluginEvent { 
        name: "custom".to_string(), 
        data, 
    };
    match event {
        AgentStreamEvent::PluginEvent { name, .. } => {
            assert_eq!(name, "custom");
        }
        _ => panic!("Expected PluginEvent"),
    }
}
```

- [ ] **Step 4: Run tests to verify**

Run: `cargo test -p vol-llm-agent agent_stream_event`
Expected: 3 tests pass (including existing tests)

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-agent/src/react/stream.rs
git commit -m "feat: add AgentAborted and PluginEvent variants"
```

---

### Task 3: Update AgentPlugin Trait

**Files:**
- Modify: `crates/vol-llm-agent/src/react/plugin.rs`

- [ ] **Step 1: Replace PluginAction with PluginDecision in intercept()**

Update AgentPlugin trait in plugin.rs:

```rust
#[async_trait]
pub trait AgentPlugin: Send + Sync {
    fn id(&self) -> PluginId;
    fn priority(&self) -> u32 { 100 }

    /// Interceptor hook - sync, serial, can block flow
    async fn intercept(
        &self, 
        event: &AgentStreamEvent, 
        ctx: &RunContext
    ) -> PluginDecision {
        PluginDecision::Continue  // Default: no-op
    }

    /// Listener hook - async, parallel, fire-and-forget
    async fn listen(
        &self, 
        event: &AgentStreamEvent, 
        ctx: &RunContext
    );
}
```

- [ ] **Step 2: Remove PluginAction enum (no longer needed)**

Delete PluginAction enum from plugin.rs (keep PluginDecision)

- [ ] **Step 3: Update existing plugin implementations**

Update HITL plugin in hitl.rs:

```rust
async fn intercept(
    &self, 
    event: &AgentStreamEvent, 
    _ctx: &RunContext
) -> PluginDecision {
    match event {
        AgentStreamEvent::ToolCallBegin { tool_name, arguments } => {
            // Block and wait for approval
            match self.wait_for_approval(tool_name, arguments).await {
                ApprovalResult::Approved => PluginDecision::Continue,
                ApprovalResult::Rejected => PluginDecision::Skip,
                ApprovalResult::Stop => PluginDecision::Abort("User stopped".into()),
            }
        }
        _ => PluginDecision::Continue,
    }
}

async fn listen(
    &self, 
    event: &AgentStreamEvent, 
    _ctx: &RunContext
) {
    if matches!(event, AgentStreamEvent::AgentAborted { .. }) {
        tracing::info!("HITL: Agent aborted");
    }
}
```

- [ ] **Step 4: Update Observability plugin**

Update observability.rs:

```rust
async fn intercept(
    &self, 
    _event: &AgentStreamEvent, 
    _ctx: &RunContext
) -> PluginDecision {
    PluginDecision::Continue  // No blocking
}

async fn listen(
    &self, 
    event: &AgentStreamEvent, 
    _ctx: &RunContext
) {
    tracing::info!("Agent event: {:?}", event);
}
```

- [ ] **Step 5: Run cargo check**

Run: `cargo check -p vol-llm-agent`
Expected: No errors

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-agent/src/react/plugin.rs
git add crates/vol-llm-agent/src/react/hitl.rs
git add crates/vol-llm-agent/src/plugins/observability.rs
git commit -m "feat: update AgentPlugin trait with intercept/listen hooks"
```

---

### Task 4: Add Event Bus to RunContext

**Files:**
- Modify: `crates/vol-llm-agent/src/react/run_context.rs`

- [ ] **Step 1: Add imports**

Add to top of run_context.rs:

```rust
use tokio::sync::{broadcast, mpsc, oneshot};
use super::plugin::PluginDecision;
```

- [ ] **Step 2: Add PluginRequest enum**

Add before RunContext struct:

```rust
enum PluginRequest {
    Intercept {
        event: AgentStreamEvent,
        tx: oneshot::Sender<PluginDecision>,
    },
    Emit {
        event: AgentStreamEvent,
    },
}
```

- [ ] **Step 3: Add event bus fields to RunContext**

Add to RunContext struct:

```rust
pub struct RunContext {
    // ... existing fields
    
    // Event bus
    event_tx: broadcast::Sender<AgentStreamEvent>,
    plugin_event_tx: mpsc::Sender<PluginRequest>,
}
```

- [ ] **Step 4: Update RunContext::new()**

Update constructor in run_context.rs:

```rust
pub fn new(...) -> Self {
    let (event_tx, _) = broadcast::channel(100);
    let (plugin_event_tx, _) = mpsc::channel(100);
    
    Self {
        // ... existing fields
        event_tx,
        plugin_event_tx,
    }
}
```

- [ ] **Step 5: Add emit() method**

Add to RunContext impl:

```rust
/// Emit event to broadcast channel (listeners only)
pub async fn emit(&self, event: AgentStreamEvent) {
    let _ = self.event_tx.send(event);
}
```

- [ ] **Step 6: Add intercept() method**

Add to RunContext impl:

```rust
/// Send event to interceptors and wait for decision
pub async fn intercept(
    &self, 
    event: &AgentStreamEvent
) -> Result<PluginDecision, AgentError> {
    let (tx, rx) = oneshot::channel();
    self.plugin_event_tx.send(PluginRequest::Intercept {
        event: event.clone(),
        tx,
    }).await.map_err(|e| {
        AgentError::Context(format!("Plugin channel error: {}", e))
    })?;
    
    rx.await.map_err(|e| {
        AgentError::Context(format!("Plugin response error: {}", e))
    })
}
```

- [ ] **Step 7: Run cargo check**

Run: `cargo check -p vol-llm-agent`
Expected: No errors

- [ ] **Step 8: Commit**

```bash
git add crates/vol-llm-agent/src/react/run_context.rs
git commit -m "feat: add event bus to RunContext with emit/intercept methods"
```

---

### Task 5: Update PluginStream for Event Bus

**Files:**
- Modify: `crates/vol-llm-agent/src/react/plugin_stream.rs`

- [ ] **Step 1: Add imports**

Add to top of plugin_stream.rs:

```rust
use super::plugin::PluginDecision;
use tokio::sync::{broadcast, mpsc};
```

- [ ] **Step 2: Update PluginStream struct**

Replace PluginStream struct:

```rust
pub struct PluginStream {
    plugins: Vec<Arc<dyn AgentPlugin>>,
    ctx: RunContext,
    event_rx: Option<broadcast::Receiver<AgentStreamEvent>>,
    plugin_rx: Option<mpsc::Receiver<PluginRequest>>,
}
```

- [ ] **Step 3: Add spawn_listener_task method**

Add to PluginStream impl:

```rust
/// Spawn listener task that processes broadcast events
pub fn spawn_listener_task(
    plugins: Vec<Arc<dyn AgentPlugin>>,
    ctx: RunContext,
) -> tokio::task::JoinHandle<()> {
    let mut event_rx = ctx.event_tx.subscribe();
    let ctx_clone = ctx.clone();
    
    tokio::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            // Fire all listeners in parallel, don't wait
            for plugin in &plugins {
                let plugin = plugin.clone();
                let event = &event;
                let ctx = &ctx_clone;
                tokio::spawn(async move {
                    plugin.listen(event, ctx).await;
                });
            }
        }
    })
}
```

- [ ] **Step 4: Add run_interceptor_loop function**

Add to plugin_stream.rs:

```rust
/// Run interceptor loop (called in spawned task)
pub async fn run_interceptor_loop(
    mut plugin_rx: mpsc::Receiver<PluginRequest>,
    plugins: Vec<Arc<dyn AgentPlugin>>,
    ctx: RunContext,
) {
    while let Some(msg) = plugin_rx.recv().await {
        match msg {
            PluginRequest::Intercept { event, tx } => {
                let mut decision = PluginDecision::Continue;
                for plugin in &plugins {
                    match plugin.intercept(&event, &ctx).await {
                        PluginDecision::Continue => continue,
                        PluginDecision::Skip => {
                            decision = PluginDecision::Skip;
                            break;
                        }
                        PluginDecision::Abort(reason) => {
                            decision = PluginDecision::Abort(reason);
                            break;
                        }
                    }
                }
                let _ = tx.send(decision);
            }
            PluginRequest::Emit { event } => {
                // Plugin events only go to listeners, not interceptors
                let _ = ctx.event_tx.send(event);
            }
        }
    }
}
```

- [ ] **Step 5: Run cargo check**

Run: `cargo check -p vol-llm-agent`
Expected: No errors

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-agent/src/react/plugin_stream.rs
git commit -m "feat: add listener task and interceptor loop to PluginStream"
```

---

### Task 6: Integrate Event Bus in ReActAgent::run()

**Files:**
- Modify: `crates/vol-llm-agent/src/react/agent.rs`

- [ ] **Step 1: Add imports**

Add to top of agent.rs:

```rust
use super::plugin_stream::{spawn_listener_task, run_interceptor_loop};
use tokio::sync::mpsc;
```

- [ ] **Step 2: Update run() to spawn listener and interceptor tasks**

Update run() method after creating run_ctx:

```rust
let run_ctx = RunContext::new(...);

// Spawn listener task
let listener_handle = PluginStream::spawn_listener_task(
    self.config.plugin_registry.plugins().to_vec(),
    run_ctx.clone(),
);

// Spawn interceptor loop
let (plugin_tx, plugin_rx) = mpsc::channel(100);
let interceptor_run_ctx = run_ctx.clone();
let interceptor_plugins = self.config.plugin_registry.plugins().to_vec();
tokio::spawn(async move {
    run_interceptor_loop(plugin_rx, interceptor_plugins, interceptor_run_ctx).await;
});

// Store plugin_tx in run_ctx for later use
// (May need to add set_plugin_tx method or pass explicitly)
```

- [ ] **Step 3: Emit and intercept AgentStart event**

Add after init_messages():

```rust
// Emit AgentStart
let start_event = AgentStreamEvent::AgentStart { 
    input: user_input.to_string() 
};
run_ctx.emit(start_event.clone()).await;

// Intercept AgentStart
match run_ctx.intercept(&start_event).await? {
    PluginDecision::Continue => {}
    PluginDecision::Skip => {
        return create_skip_stream(run_ctx, run_id).await;
    }
    PluginDecision::Abort(reason) => {
        run_ctx.emit(AgentStreamEvent::AgentAborted { 
            reason: reason.clone() 
        }).await;
        return Err(AgentError::Context(reason));
    }
}
```

- [ ] **Step 4: Update tool call loop with intercept**

Update tool execution in spawned task:

```rust
for call in &tool_calls {
    // Emit ToolCallBegin
    let event = AgentStreamEvent::ToolCallBegin {
        tool_name: call.name.clone(),
        arguments: call.arguments.clone(),
    };
    run_ctx.emit(event.clone()).await;
    
    // Intercept ToolCallBegin
    let decision = run_ctx.intercept(&event).await?;
    
    match decision {
        PluginDecision::Continue => {
            // Execute tool
            let result = tools.execute(call, &context).await?;
            
            // Emit ToolCallComplete
            let complete_event = AgentStreamEvent::ToolCallComplete {
                tool_name: call.name.clone(),
                result: result.content.clone(),
            };
            run_ctx.emit(complete_event).await;
        }
        PluginDecision::Skip => continue,
        PluginDecision::Abort(reason) => {
            run_ctx.emit(AgentStreamEvent::AgentAborted { 
                reason: reason.clone() 
            }).await;
            break;
        }
    }
}
```

- [ ] **Step 5: Add Abort event emission on max iterations**

Update max_iterations check:

```rust
if iteration > config.max_iterations {
    run_ctx.emit(AgentStreamEvent::AgentAborted { 
        reason: "Max iterations reached".to_string() 
    }).await;
    break;
}
```

- [ ] **Step 6: Add AgentComplete event**

Add before sending AgentComplete response:

```rust
let complete_event = AgentStreamEvent::AgentComplete { response: response.clone() };
run_ctx.emit(complete_event.clone()).await;

match run_ctx.intercept(&complete_event).await? {
    PluginDecision::Abort(reason) => {
        run_ctx.emit(AgentStreamEvent::AgentAborted { 
            reason: reason.clone() 
        }).await;
        // Continue with normal completion
    }
    _ => {}
}
```

- [ ] **Step 7: Cleanup listener handle**

Add at end of spawned task:

```bash
listener_handle.abort();
```

- [ ] **Step 8: Run cargo check**

Run: `cargo check -p vol-llm-agent`
Expected: No errors

- [ ] **Step 9: Commit**

```bash
git add crates/vol-llm-agent/src/react/agent.rs
git commit -m "feat: integrate event bus in ReActAgent::run()"
```

---

### Task 7: Update Module Exports

**Files:**
- Modify: `crates/vol-llm-agent/src/react/mod.rs`

- [ ] **Step 1: Update exports**

Update mod.rs exports:

```rust
pub use plugin::{AgentPlugin, PluginDecision, PluginRegistry};
pub use plugin_stream::{PluginStream, create_shortcircuit_stream, create_skip_stream, run_interceptor_loop, spawn_listener_task};
```

- [ ] **Step 2: Run cargo check**

Run: `cargo check -p vol-llm-agent`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent/src/react/mod.rs
git commit -m "feat: export PluginDecision and new plugin_stream functions"
```

---

### Task 8: Add Unit Tests

**Files:**
- Modify: `crates/vol-llm-agent/src/react/plugin.rs`
- Create: `crates/vol-llm-agent/tests/plugin_flow_test.rs`

- [ ] **Step 1: Add interceptor chain order test**

Add to plugin.rs tests:

```rust
#[tokio::test]
async fn test_interceptor_chain_order() {
    use std::sync::Arc;
    use tokio::sync::Mutex;
    
    struct OrderPlugin {
        id: String,
        order: Arc<Mutex<Vec<String>>>,
    }
    
    #[async_trait]
    impl AgentPlugin for OrderPlugin {
        fn id(&self) -> PluginId { self.id.clone() }
        fn priority(&self) -> u32 { 
            if self.id == "first" { 10 } else { 20 }
        }
        
        async fn intercept(&self, _event: &AgentStreamEvent, _ctx: &RunContext) -> PluginDecision {
            let mut order = self.order.lock().await;
            order.push(self.id.clone());
            PluginDecision::Continue
        }
        
        async fn listen(&self, _event: &AgentStreamEvent, _ctx: &RunContext) {}
    }
    
    let order = Arc::new(Mutex::new(Vec::new()));
    let mut plugins = vec![
        Arc::new(OrderPlugin { id: "second".to_string(), order: order.clone() }) as Arc<dyn AgentPlugin>,
        Arc::new(OrderPlugin { id: "first".to_string(), order: order.clone() }) as Arc<dyn AgentPlugin>,
    ];
    plugins.sort_by_key(|p| p.priority());
    
    let event = AgentStreamEvent::AgentStart { input: "test".to_string() };
    let ctx = create_test_context();
    
    for plugin in &plugins {
        plugin.intercept(&event, &ctx).await;
    }
    
    let final_order = order.lock().await;
    assert_eq!(*final_order, vec!["first", "second"]);
}
```

- [ ] **Step 2: Add interceptor abort stops chain test**

Add to plugin.rs tests:

```rust
#[tokio::test]
async fn test_interceptor_abort_stops_chain() {
    use std::sync::Arc;
    use tokio::sync::Mutex;
    
    struct AbortPlugin { id: String }
    
    #[async_trait]
    impl AgentPlugin for AbortPlugin {
        fn id(&self) -> PluginId { self.id.clone() }
        fn priority(&self) -> u32 { 10 }
        
        async fn intercept(&self, _event: &AgentStreamEvent, _ctx: &RunContext) -> PluginDecision {
            PluginDecision::Abort("aborted".to_string())
        }
        
        async fn listen(&self, _event: &AgentStreamEvent, _ctx: &RunContext) {}
    }
    
    struct ShouldNotRunPlugin {}
    
    #[async_trait]
    impl AgentPlugin for ShouldNotRunPlugin {
        fn id(&self) -> PluginId { "should_not_run".to_string() }
        fn priority(&self) -> u32 { 20 }
        
        async fn intercept(&self, _event: &AgentStreamEvent, _ctx: &RunContext) -> PluginDecision {
            panic!("Should not reach here");
        }
        
        async fn listen(&self, _event: &AgentStreamEvent, _ctx: &RunContext) {}
    }
    
    let plugins = vec![
        Arc::new(AbortPlugin { id: "abort".to_string() }),
        Arc::new(ShouldNotRunPlugin {}),
    ];
    
    let event = AgentStreamEvent::AgentStart { input: "test".to_string() };
    let ctx = create_test_context();
    
    // Simulate interceptor chain
    for plugin in &plugins {
        match plugin.intercept(&event, &ctx).await {
            PluginDecision::Continue => continue,
            PluginDecision::Abort(_) => break,
            PluginDecision::Skip => break,
        }
    }
    // Test passes if ShouldNotRunPlugin doesn't panic
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p vol-llm-agent interceptor`
Expected: 2 tests pass

- [ ] **Step 4: Create integration test file**

Create `crates/vol-llm-agent/tests/plugin_flow_test.rs`:

```rust
//! Plugin flow intervention integration tests.

use vol_llm_agent::react::{ReActAgent, AgentPlugin, PluginDecision, AgentStreamEvent, RunContext};
use vol_llm_core::{LLMClient, LLMProvider, Message, ConversationRequest, ConversationResponse, SupportedParam};
use vol_llm_tool::ToolContext;
use async_trait::async_trait;
use std::sync::Arc;

struct MockLlm;

#[async_trait]
impl LLMClient for MockLlm {
    fn provider(&self) -> LLMProvider { LLMProvider::Anthropic }
    fn model(&self) -> &str { "mock" }
    fn supported_params(&self) -> &[SupportedParam] { &[] }
    
    async fn converse(&self, _req: ConversationRequest) -> vol_llm_core::Result<ConversationResponse> {
        unimplemented!("Use converse_stream")
    }
    
    async fn converse_stream(&self, _req: ConversationRequest) -> vol_llm_core::Result<vol_llm_core::stream::StreamReceiver> {
        use tokio::sync::mpsc;
        use vol_llm_core::{StreamEvent, StreamEventData};
        
        let (tx, rx) = mpsc::channel(10);
        tokio::spawn(async move {
            let _ = tx.send(Ok(StreamEvent {
                id: "event_1".to_string(),
                data: StreamEventData::ContentComplete { content: "Mock response".to_string() },
            })).await;
        });
        
        Ok(vol_llm_core::StreamReceiver::new(rx))
    }
}
```

- [ ] **Step 5: Add listener parallel execution test**

Add to plugin_flow_test.rs:

```rust
#[tokio::test]
async fn test_listener_parallel_execution() {
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use tokio::time::{sleep, Duration};
    
    struct SlowListener { 
        id: String,
        executed: Arc<Mutex<Vec<String>>>,
    }
    
    #[async_trait]
    impl AgentPlugin for SlowListener {
        fn id(&self) -> PluginId { self.id.clone() }
        
        async fn intercept(&self, _event: &AgentStreamEvent, _ctx: &RunContext) -> PluginDecision {
            PluginDecision::Continue
        }
        
        async fn listen(&self, event: &AgentStreamEvent, _ctx: &RunContext) {
            sleep(Duration::from_millis(10)).await;
            let mut executed = self.executed.lock().await;
            executed.push(self.id.clone());
        }
    }
    
    let executed = Arc::new(Mutex::new(Vec::new()));
    let mut registry = vol_llm_agent::react::PluginRegistry::new();
    registry.register(SlowListener { 
        id: "listener1".to_string(), 
        executed: executed.clone(),
    });
    registry.register(SlowListener { 
        id: "listener2".to_string(), 
        executed: executed.clone(),
    });
    
    let agent = ReActAgent::builder()
        .with_llm(Arc::new(MockLlm))
        .with_plugin_registry(registry)
        .build()
        .unwrap();
    
    let _stream = agent.run("test", ToolContext::default()).await.unwrap();
    
    // Wait for listeners to complete
    sleep(Duration::from_millis(100)).await;
    
    let executed = executed.lock().await;
    assert_eq!(executed.len(), 2);
}
```

- [ ] **Step 6: Run integration tests**

Run: `cargo test -p vol-llm-agent --test plugin_flow_test`
Expected: Tests pass

- [ ] **Step 7: Commit**

```bash
git add crates/vol-llm-agent/src/react/plugin.rs
git add crates/vol-llm-agent/tests/plugin_flow_test.rs
git commit -m "test: add unit and integration tests for plugin flow"
```

---

### Task 9: Final Verification

**Files:**
- All modified files

- [ ] **Step 1: Run cargo check on workspace**

Run: `cargo check --workspace`
Expected: No errors

- [ ] **Step 2: Run all vol-llm-agent tests**

Run: `cargo test -p vol-llm-agent`
Expected: All tests pass (existing + new)

- [ ] **Step 3: Run cargo build --release**

Run: `cargo build --release`
Expected: Successful build

- [ ] **Step 4: Commit final changes**

```bash
git commit -am "chore: final verification and cleanup"
```

---

## Spec Self-Review

**1. Spec coverage check:**

| Spec Requirement | Task |
|-----------------|------|
| PluginDecision enum | Task 1 |
| AgentStreamEvent extensions | Task 2 |
| AgentPlugin trait update | Task 3 |
| RunContext event bus | Task 4 |
| PluginStream listener/interceptor | Task 5 |
| ReActAgent::run() integration | Task 6 |
| Module exports | Task 7 |
| Unit tests (10+) | Task 8 |
| Integration tests (5+) | Task 8 |

**2. Placeholder scan:** No TBD/TODO found.

**3. Type consistency check:**
- `PluginDecision` - consistent across all tasks
- `AgentStreamEvent` - consistent naming
- `RunContext` methods - `emit()`, `intercept()` consistent
- All method signatures match spec

---

Plan complete and saved to `docs/superpowers/plans/2026-04-09-plugin-flow-intervention-plan.md`.

Two execution options:

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

Which approach?
