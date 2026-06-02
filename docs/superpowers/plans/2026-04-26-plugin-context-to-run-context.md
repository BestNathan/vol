# PluginContext → RunContext Migration Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace PluginContext with RunContext in the AgentPlugin trait, move the trait from vol-llm-core to vol-llm-agent, remove vol-llm-observability from vol-llm-agent's deps, and delete dead variables.

**Architecture:** AgentPlugin trait and RunContext both live in vol-llm-agent. vol-llm-core has no plugin concept. vol-llm-observability depends on vol-llm-agent for the trait. vol-session has no plugin code. vol-llm-agent has no observability module.

**Tech Stack:** Rust, tokio, async-trait

---

### Task 1: Delete PluginContext from vol-llm-core, move AgentPlugin trait to vol-llm-agent

**Files:**
- Modify: `crates/vol-llm-core/src/plugin.rs` (DELETE content, keep minimal re-exports if needed)
- Modify: `crates/vol-llm-core/src/lib.rs:8,20`
- Modify: `crates/vol-llm-agent/src/react/plugin.rs` (REWRITE)
- Modify: `crates/vol-llm-agent/src/react/mod.rs:50,56`

**Steps:**

- [ ] **Step 1.1: Delete vol-llm-core/src/plugin.rs**

Delete the entire file. The AgentPlugin trait, PluginContext, PluginDecision, PluginId, and PluginRegistry move to vol-llm-agent.

Run: `rm crates/vol-llm-core/src/plugin.rs`

- [ ] **Step 1.2: Remove plugin module from vol-llm-core/src/lib.rs**

Change:
```rust
// Line 8: Remove this line
pub mod plugin;

// Line 20: Remove this line
pub use plugin::*;
```

- [ ] **Step 1.3: Rewrite vol-llm-agent/src/react/plugin.rs**

Replace the entire file content. It currently re-exports from vol_llm_core::plugin. After the change it defines the types locally:

```rust
//! Plugin system for ReAct Agent.
//!
//! Defines the AgentPlugin trait, PluginDecision, and PluginRegistry.
//! RunContext (defined in run_context.rs) is the context type passed to plugin hooks.

pub use vol_llm_core::AgentStreamEvent;
pub use super::run_context::RunContext;

/// Plugin unique identifier
pub type PluginId = String;

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

/// Plugin trait for extending agent functionality
#[async_trait::async_trait]
pub trait AgentPlugin: Send + Sync {
    fn id(&self) -> PluginId;

    fn priority(&self) -> u32 {
        100
    }

    /// Interceptor hook - can modify or block the event.
    /// Returns PluginDecision to continue, skip, or abort.
    async fn intercept(&self, _event: &AgentStreamEvent, _ctx: &RunContext) -> PluginDecision {
        PluginDecision::Continue
    }

    /// Listener hook - async, parallel, fire-and-forget.
    /// Called after event execution. Used for observability, logging, etc.
    async fn listen(&self, _event: &AgentStreamEvent, _ctx: &RunContext);
}

/// Plugin registry - manages plugin lifecycle and execution order
#[derive(Clone)]
pub struct PluginRegistry {
    plugins: Vec<std::sync::Arc<dyn AgentPlugin>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self { plugins: Vec::new() }
    }

    pub fn register<P: AgentPlugin + 'static>(&mut self, plugin: P) {
        let plugin = std::sync::Arc::new(plugin);
        let pos = self
            .plugins
            .iter()
            .position(|p| p.priority() > plugin.priority())
            .unwrap_or(self.plugins.len());
        self.plugins.insert(pos, plugin);
    }

    pub fn plugins(&self) -> &[std::sync::Arc<dyn AgentPlugin>] {
        &self.plugins
    }

    pub fn get(&self, id: &str) -> Option<&std::sync::Arc<dyn AgentPlugin>> {
        self.plugins.iter().find(|p| p.id() == id)
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 1.4: Update vol-llm-agent/src/react/mod.rs re-exports**

Change line 50 from:
```rust
pub use plugin::{AgentPlugin, PluginContext, PluginDecision, PluginRegistry, PluginId};
```
to:
```rust
pub use plugin::{AgentPlugin, PluginDecision, PluginId, PluginRegistry};
```

Change line 56 from:
```rust
pub use run_context::{PluginRequest, RunContext, plugin_context_from_run_ctx};
```
to:
```rust
pub use run_context::{PluginRequest, RunContext};
```

- [ ] **Step 1.5: Build to verify compilation of this task**

Run: `cargo build -p vol-llm-core -p vol-llm-agent --lib 2>&1 | head -30`
Expected: Many errors (expected — other files still reference PluginContext). Just confirm the files we changed compile structurally.

- [ ] **Step 1.6: Commit**

```bash
git add crates/vol-llm-core/src/plugin.rs crates/vol-llm-core/src/lib.rs crates/vol-llm-agent/src/react/plugin.rs crates/vol-llm-agent/src/react/mod.rs
git commit -m "refactor: move AgentPlugin trait from vol-llm-core to vol-llm-agent, replace PluginContext with RunContext in trait signatures

Plugin is an agent concept, not a core LLM concept. The AgentPlugin trait,
PluginDecision, PluginId, and PluginRegistry are now defined in vol-llm-agent.
PluginContext is deleted; trait methods now accept &RunContext directly."
```

---

### Task 2: Clean up RunContext — delete plugin_context_from_run_ctx, remove dead variables

**Files:**
- Modify: `crates/vol-llm-agent/src/react/run_context.rs:1-32`
- Modify: `crates/vol-llm-agent/src/react/agent.rs:1-6,200-225,227-235`
- Modify: `crates/vol-llm-agent/src/react/plugin_stream.rs:3,26-34,62-72`

**Steps:**

- [ ] **Step 2.1: Delete plugin_context_from_run_ctx from run_context.rs**

In `crates/vol-llm-agent/src/react/run_context.rs`:

Remove the import (line 16):
```rust
use vol_llm_core::PluginContext;
```

Remove the function (lines 22-32):
```rust
/// Create a PluginContext from a RunContext
pub fn plugin_context_from_run_ctx(ctx: &RunContext) -> PluginContext {
    PluginContext {
        run_id: ctx.run_id.clone(),
        user_input: ctx.user_input.clone(),
        session_id: ctx.session_id.clone(),
        all_tool_calls: ctx.all_tool_calls.clone(),
        current_tool_calls: ctx.current_tool_calls.clone(),
        data: ctx.data.clone(),
    }
}
```

Also update the doc comment on RunContext struct (line 48): change "This replaces the old PluginContext" to just describe RunContext without referencing the old type.

- [ ] **Step 2.2: Remove dead variables and update plugin_ctx usage in agent.rs**

In `crates/vol-llm-agent/src/react/agent.rs`:

Remove import of `plugin_context_from_run_ctx` (line 5):
```rust
// Before:
use super::{
    AgentResponse, AgentStreamEvent, PluginDecision, PluginRegistry, RunContext,
    plugin_context_from_run_ctx,
};
// After:
use super::{
    AgentResponse, AgentStreamEvent, PluginDecision, PluginRegistry, RunContext,
};
```

Remove dead variables (lines 231-234):
```rust
// DELETE these 3 lines:
let _session_id = self.session.id.clone();
let _session = self.session.clone();
let _run_id_clone = run_id.clone();
```

Replace `plugin_context_from_run_ctx` calls (lines 205, 216):
```rust
// Before (line ~205):
let plugin_ctx = plugin_context_from_run_ctx(&run_ctx);
let listener_handle = spawn_listener_task(
    self.config.plugin_registry.plugins().to_vec(),
    plugin_ctx,
    listener_event_rx,
);

// After:
let listener_handle = spawn_listener_task(
    self.config.plugin_registry.plugins().to_vec(),
    run_ctx.clone(),
    listener_event_rx,
);
```

```rust
// Before (line ~216):
let interceptor_plugin_ctx = plugin_context_from_run_ctx(&run_ctx);
let interceptor_handle = tokio::spawn(async move {
    run_interceptor_loop(
        plugin_rx,
        interceptor_plugins,
        interceptor_event_tx,
        interceptor_plugin_ctx,
    )
    .await;
});

// After:
let interceptor_ctx = run_ctx.clone();
let interceptor_handle = tokio::spawn(async move {
    run_interceptor_loop(
        plugin_rx,
        interceptor_plugins,
        interceptor_event_tx,
        interceptor_ctx,
    )
    .await;
});
```

- [ ] **Step 2.3: Update plugin_stream.rs signatures**

In `crates/vol-llm-agent/src/react/plugin_stream.rs`:

Change imports (line 3):
```rust
// Before:
use super::plugin::{AgentPlugin, PluginContext, PluginDecision};
// After:
use super::plugin::{AgentPlugin, PluginDecision};
```

Change `spawn_listener_task` (lines 30-34):
```rust
// Before:
pub fn spawn_listener_task(
    plugins: Vec<Arc<dyn AgentPlugin>>,
    plugin_ctx: PluginContext,
    mut event_rx: broadcast::Receiver<TracedEvent<AgentStreamEvent>>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        while let Ok(traced_event) = event_rx.recv().await {
            let event = traced_event.value();
            for plugin in &plugins {
                let plugin = plugin.clone();
                let event = event.clone();
                let plugin_ctx = plugin_ctx.clone();
                tokio::spawn(async move {
                    plugin.listen(&event, &plugin_ctx).await;
                });
            }
        }
    })
}

// After:
pub fn spawn_listener_task(
    plugins: Vec<Arc<dyn AgentPlugin>>,
    ctx: RunContext,
    mut event_rx: broadcast::Receiver<TracedEvent<AgentStreamEvent>>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        while let Ok(traced_event) = event_rx.recv().await {
            let event = traced_event.value();
            for plugin in &plugins {
                let plugin = plugin.clone();
                let event = event.clone();
                let ctx = ctx.clone();
                tokio::spawn(async move {
                    plugin.listen(&event, &ctx).await;
                });
            }
        }
    })
}
```

Change `run_interceptor_loop` (lines 67-72):
```rust
// Before:
pub async fn run_interceptor_loop(
    mut plugin_rx: mpsc::Receiver<PluginRequest>,
    plugins: Vec<Arc<dyn AgentPlugin>>,
    event_tx: broadcast::Sender<TracedEvent<AgentStreamEvent>>,
    plugin_ctx: PluginContext,
) {
    while let Some(msg) = plugin_rx.recv().await {
        match msg {
            PluginRequest::Intercept { event, tx } => {
                let mut decision = PluginDecision::Continue;
                for plugin in &plugins {
                    match plugin.intercept(event.value(), &plugin_ctx).await {
                        // ...
                    }
                }
                let _ = tx.send(decision);
            }
            // ...
        }
    }
}

// After:
pub async fn run_interceptor_loop(
    mut plugin_rx: mpsc::Receiver<PluginRequest>,
    plugins: Vec<Arc<dyn AgentPlugin>>,
    event_tx: broadcast::Sender<TracedEvent<AgentStreamEvent>>,
    ctx: RunContext,
) {
    while let Some(msg) = plugin_rx.recv().await {
        match msg {
            PluginRequest::Intercept { event, tx } => {
                let mut decision = PluginDecision::Continue;
                for plugin in &plugins {
                    match plugin.intercept(event.value(), &ctx).await {
                        // ...
                    }
                }
                let _ = tx.send(decision);
            }
            // ...
        }
    }
}
```

Update comments (lines 26-29, 62-66) — remove references to "PluginContext does NOT contain sender references". Replace with brief RunContext references.

- [ ] **Step 2.4: Build and verify no more compile errors in these files**

Run: `cargo build -p vol-llm-agent --lib 2>&1 | grep -E "^error|^warning: unused" | head -20`
Expected: Errors from plugin implementations (hitl.rs, caching.rs, etc.) — those are fixed in Task 3.

- [ ] **Step 2.5: Commit**

```bash
git add crates/vol-llm-agent/src/react/run_context.rs crates/vol-llm-agent/src/react/agent.rs crates/vol-llm-agent/src/react/plugin_stream.rs
git commit -m "refactor: remove plugin_context_from_run_ctx, replace with RunContext.clone; delete dead variables

Remove plugin_context_from_run_ctx from run_context.rs. Agent run() now passes
run_ctx.clone() directly to plugin stream functions. Delete unused _session_id,
_session, _run_id_clone variables."
```

---

### Task 3: Update plugin implementations in vol-llm-agent

**Files:**
- Modify: `crates/vol-llm-agent/src/plugins/caching.rs`
- Modify: `crates/vol-llm-agent/src/plugins/rate_limiter.rs`
- Modify: `crates/vol-llm-agent/src/plugins/retry.rs`
- Modify: `crates/vol-llm-agent/src/react/hitl.rs`

**Steps:**

For each of the 4 files above, apply the same pattern:

- [ ] **Step 3.1: Update plugin trait impl signatures**

In each file, change `&PluginContext` → `&RunContext` in the `impl AgentPlugin` block:

**caching.rs** (lines 122, 127):
```rust
// Before:
async fn intercept(&self, _event: &AgentStreamEvent, _ctx: &PluginContext) -> PluginDecision {
async fn listen(&self, event: &AgentStreamEvent, ctx: &PluginContext) {

// After:
async fn intercept(&self, _event: &AgentStreamEvent, _ctx: &RunContext) -> PluginDecision {
async fn listen(&self, event: &AgentStreamEvent, ctx: &RunContext) {
```

**rate_limiter.rs** (lines 34, 39):
```rust
// Before:
async fn intercept(&self, _event: &AgentStreamEvent, _ctx: &PluginContext) -> PluginDecision {
async fn listen(&self, event: &AgentStreamEvent, ctx: &PluginContext) {

// After:
async fn intercept(&self, _event: &AgentStreamEvent, _ctx: &RunContext) -> PluginDecision {
async fn listen(&self, event: &AgentStreamEvent, ctx: &RunContext) {
```

**retry.rs** (lines 50, 55):
```rust
// Before:
async fn intercept(&self, _event: &AgentStreamEvent, _ctx: &PluginContext) -> PluginDecision {
async fn listen(&self, event: &AgentStreamEvent, ctx: &PluginContext) {

// After:
async fn intercept(&self, _event: &AgentStreamEvent, _ctx: &RunContext) -> PluginDecision {
async fn listen(&self, event: &AgentStreamEvent, ctx: &RunContext) {
```

**hitl.rs** (lines 117, 186, 248):
```rust
// Remove line 117:
use super::plugin::PluginContext;

// Change line 186:
// Before:
async fn intercept(&self, event: &AgentStreamEvent, ctx: &PluginContext) -> PluginDecision {
// After:
async fn intercept(&self, event: &AgentStreamEvent, ctx: &RunContext) -> PluginDecision {

// Change line 248:
// Before:
async fn listen(&self, event: &AgentStreamEvent, ctx: &PluginContext) {
// After:
async fn listen(&self, event: &AgentStreamEvent, ctx: &RunContext) {
```

- [ ] **Step 3.2: Update imports in each file**

In each file, replace:
```rust
// Before:
use crate::react::plugin::PluginContext;
// or:
use crate::react::plugin::*;
use crate::react::plugin::PluginContext;

// After:
use crate::react::plugin::RunContext;
```

Note: `RunContext` is now re-exported from `plugin.rs` (see Task 1), so `use crate::react::plugin::RunContext;` works.

- [ ] **Step 3.3: Build**

Run: `cargo build -p vol-llm-agent --lib 2>&1 | grep -E "^error" | head -10`
Expected: Errors from test code only (next task). The lib should compile.

- [ ] **Step 3.4: Commit**

```bash
git add crates/vol-llm-agent/src/plugins/caching.rs crates/vol-llm-agent/src/plugins/rate_limiter.rs crates/vol-llm-agent/src/plugins/retry.rs crates/vol-llm-agent/src/react/hitl.rs
git commit -m "refactor: update plugin implementations to use RunContext instead of PluginContext"
```

---

### Task 4: Update vol-llm-agent tests

**Files:**
- Modify: `crates/vol-llm-agent/src/react/tests.rs`
- Modify: `crates/vol-llm-agent/tests/agent_run_tests.rs`
- Modify: `crates/vol-llm-agent/tests/plugin_test.rs`
- Modify: `crates/vol-llm-agent/tests/plugin_flow_test.rs`
- Modify: `crates/vol-llm-agent/tests/react_mock_test.rs`
- Modify: `crates/vol-llm-agent/tests/code_agent_simulation.rs`
- Modify: `crates/vol-llm-agent/tests/session_recording_test.rs`
- Modify: `crates/vol-llm-agent/tests/observability_integration.rs` (DELETE)
- Modify: `crates/vol-llm-agent/src/plugins/caching.rs` (tests section)
- Modify: `crates/vol-llm-agent/src/plugins/rate_limiter.rs` (tests section)
- Modify: `crates/vol-llm-agent/src/plugins/retry.rs` (tests section)

**Steps:**

- [ ] **Step 4.1: Delete observability_integration.rs**

Run: `rm crates/vol-llm-agent/tests/observability_integration.rs`

- [ ] **Step 4.2: Update react/tests.rs**

Change the DummyPlugin impl (lines 67-68):
```rust
// Before:
async fn intercept(&self, _: &AgentStreamEvent, _: &PluginContext) -> plugin::PluginDecision { plugin::PluginDecision::Continue }
async fn listen(&self, _: &AgentStreamEvent, _: &PluginContext) {}

// After:
async fn intercept(&self, _: &AgentStreamEvent, _: &RunContext) -> plugin::PluginDecision { plugin::PluginDecision::Continue }
async fn listen(&self, _: &AgentStreamEvent, _: &RunContext) {}
```

For `test_run_interceptor_loop_continue_decision` (lines 254-269):
```rust
// Before:
struct ContinuePlugin;
#[async_trait::async_trait]
impl plugin::AgentPlugin for ContinuePlugin {
    fn id(&self) -> plugin::PluginId { "continue".to_string() }
    fn priority(&self) -> u32 { 10 }
    async fn intercept(&self, _: &AgentStreamEvent, _: &PluginContext) -> plugin::PluginDecision {
        plugin::PluginDecision::Continue
    }
    async fn listen(&self, _: &AgentStreamEvent, _: &PluginContext) {}
}

let (plugin_tx, plugin_rx) = tokio::sync::mpsc::channel(10);
let (event_tx, _) = tokio::sync::broadcast::channel(10);
let plugin_ctx = PluginContext {
    run_id: "test".to_string(),
    user_input: "test".to_string(),
    session_id: "test".to_string(),
    all_tool_calls: Arc::new(tokio::sync::RwLock::new(vec![])),
    current_tool_calls: Arc::new(tokio::sync::RwLock::new(vec![])),
    data: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
};
// After: use create_test_run_context() helper (see step 4.3)
```

Similarly for `test_run_interceptor_loop_skip_decision` (lines 291-312) and `test_run_interceptor_loop_emit_request` (lines 331-342).

Replace all three with a helper function + RunContext usage:

Add at top of tests.rs (after imports):
```rust
use super::run_context::RunContext;
use vol_session::{InMemoryEntryStore, Session};
use vol_llm_tool::ToolRegistry;
use super::agent::AgentConfig;

fn create_test_run_context() -> RunContext {
    let (ctx, _rx) = RunContext::new(
        "test".to_string(),
        "test".to_string(),
        "session-1".to_string(),
        Arc::new(Session::new(Arc::new(InMemoryEntryStore::new()))),
        Arc::new(ToolRegistry::new()),
        AgentConfig::default(),
    );
    ctx
}
```

Then in each test, replace `let plugin_ctx = PluginContext { ... };` with `let ctx = create_test_run_context();` and pass `ctx` to `run_interceptor_loop`.

- [ ] **Step 4.3: Update tests/agent_run_tests.rs**

Change all plugin impl blocks (lines 134-145, 264-282, 406-417):
```rust
// Pattern for each: change &PluginContext → &RunContext
// Example for CountingPlugin (lines 137, 140):
async fn intercept(&self, _: &AgentStreamEvent, _: &RunContext) -> PluginDecision {
async fn listen(&self, event: &AgentStreamEvent, _: &RunContext) {
```

Update imports (line 8):
```rust
// Before:
plugin::{AgentPlugin, PluginContext, PluginDecision, PluginId},
// After:
plugin::{AgentPlugin, PluginDecision, PluginId},
```

- [ ] **Step 4.4: Update tests/plugin_test.rs**

Change imports (line 4):
```rust
// Before:
use vol_llm_agent::react::PluginContext;
// After:
use vol_llm_agent::react::RunContext;
```

Change plugin impl (lines 29, 34):
```rust
async fn intercept(&self, _event: &AgentStreamEvent, _ctx: &RunContext) -> PluginDecision {
async fn listen(&self, _event: &AgentStreamEvent, _ctx: &RunContext) {
```

- [ ] **Step 4.5: Update tests/plugin_flow_test.rs**

Change imports (lines 7, 12):
```rust
// Before:
use vol_llm_agent::react::plugin_context_from_run_ctx;
use vol_llm_agent::react::{AgentPlugin, PluginContext, PluginDecision};
// After:
use vol_llm_agent::react::{AgentPlugin, PluginDecision, RunContext};
```

Change all plugin impl `&PluginContext` → `&RunContext`.

Replace the `create_test_plugin_context()` function (lines 316-331):
```rust
// Before:
fn create_test_plugin_context() -> PluginContext {
    let (ctx, _rx) = RunContext::new(...);
    plugin_context_from_run_ctx(&ctx)
}

// After:
fn create_test_plugin_context() -> RunContext {
    let (ctx, _rx) = RunContext::new(
        "test-run".to_string(),
        "test input".to_string(),
        "session-1".to_string(),
        Arc::new(Session::new(Arc::new(InMemoryEntryStore::new()))),
        Arc::new(vol_llm_tool::ToolRegistry::new()),
        AgentConfig::default(),
    );
    ctx
}
```

- [ ] **Step 4.6: Update tests/react_mock_test.rs**

Change imports (line 11):
```rust
// Before:
use vol_llm_agent::react::PluginContext;
// After:
use vol_llm_agent::react::RunContext;
```

Change plugin impl (lines 131, 136): `&PluginContext` → `&RunContext`.

- [ ] **Step 4.7: Update tests/code_agent_simulation.rs**

Change imports (line 12):
```rust
// Before:
use vol_llm_agent::react::PluginContext;
// After:
use vol_llm_agent::react::RunContext;
```

Change plugin impl (lines 270, 275): `&PluginContext` → `&RunContext`.

- [ ] **Step 4.8: Update tests/session_recording_test.rs**

Change imports (lines 10):
```rust
// Before:
AgentPlugin, AgentStreamEvent, PluginContext,
// After:
AgentPlugin, AgentStreamEvent, RunContext,
```

Change PluginContext construction (line 23) to RunContext:
```rust
// Before:
let plugin_ctx = PluginContext {
    run_id: "test-run".to_string(),
    ...
};
// After: Use create_test_run_context() or construct RunContext::new() directly
```

Since this test uses `SessionRecorderPlugin` which will be moved (Task 6), update to use the new import path after that task. For now, just change the context type.

- [ ] **Step 4.9: Update inline tests in plugin files**

**plugins/caching.rs** (tests section, lines 147-218):
```rust
// Change imports (line 150):
// Before:
use crate::react::{AgentConfig, PluginContext, RunContext};
// After:
use crate::react::{AgentConfig, RunContext};

// Replace create_test_plugin_context (lines 154-166):
// Before:
fn create_test_plugin_context() -> PluginContext {
    let (ctx, _rx) = RunContext::new(...);
    crate::react::plugin_context_from_run_ctx(&ctx)
}
// After:
fn create_test_plugin_context() -> RunContext {
    let (ctx, _rx) = RunContext::new(...);
    ctx
}
```

Apply the same pattern to **plugins/rate_limiter.rs** and **plugins/retry.rs**.

- [ ] **Step 4.10: Build and run tests**

Run: `cargo test -p vol-llm-agent --lib 2>&1 | tail -20`
Expected: Tests pass.

- [ ] **Step 4.11: Commit**

```bash
git add -A crates/vol-llm-agent/tests/ crates/vol-llm-agent/src/react/tests.rs crates/vol-llm-agent/src/plugins/
git commit -m "test: update all vol-llm-agent tests to use RunContext instead of PluginContext"
```

---

### Task 5: Remove observability module from vol-llm-agent, break the dep

**Files:**
- Delete: `crates/vol-llm-agent/src/observability/` (entire directory)
- Modify: `crates/vol-llm-agent/src/lib.rs:4,12`
- Modify: `crates/vol-llm-agent/Cargo.toml` (remove vol-llm-observability dep)
- Modify: `crates/vol-llm-agent/src/react/builder.rs:88-96`
- Modify: `crates/vol-llm-agent/examples/agent_cli_approval.rs`
- Modify: `crates/vol-llm-agent/examples/agent_observability_test.rs` (DELETE or strip)

**Steps:**

- [ ] **Step 5.1: Delete observability directory**

Run: `rm -rf crates/vol-llm-agent/src/observability/`

- [ ] **Step 5.2: Update lib.rs**

Remove lines 4 and 12:
```rust
// Before:
pub mod observability;
// ...
pub use observability::{append_log, LogEntry, LoggerPlugin, ObservabilityPlugin};

// After: (delete both lines)
```

- [ ] **Step 5.3: Remove vol-llm-observability from Cargo.toml**

Remove:
```toml
vol-llm-observability = { path = "../vol-llm-observability" }
```

- [ ] **Step 5.4: Remove with_observability_plugin from builder.rs**

Delete lines 88-96:
```rust
pub fn with_observability_plugin(mut self) -> Self {
    let log_base_path = self.config.working_dir.join("logs/agents");
    let plugin = crate::observability::ObservabilityPlugin::new(
        self.config.agent_id.clone(),
        log_base_path,
    );
    self.config.plugin_registry.register(plugin);
    self
}
```

- [ ] **Step 5.5: Update examples**

**examples/agent_cli_approval.rs**: Remove the `.with_observability_plugin()` call (line ~290).

**examples/agent_observability_test.rs**: This file's entire purpose is testing observability. Delete it:
```bash
rm crates/vol-llm-agent/examples/agent_observability_test.rs
```

- [ ] **Step 5.6: Build**

Run: `cargo build -p vol-llm-agent --lib 2>&1 | grep -E "^error" | head -10`
Expected: No errors.

- [ ] **Step 5.7: Commit**

```bash
git add -A crates/vol-llm-agent/src/observability/ crates/vol-llm-agent/src/lib.rs crates/vol-llm-agent/Cargo.toml crates/vol-llm-agent/src/react/builder.rs crates/vol-llm-agent/examples/
git commit -m "refactor: remove observability module from vol-llm-agent, break vol-llm-observability dep

Observability is a separate concern — LoggerPlugin's AgentPlugin impl lives
in vol-llm-observability which now depends on vol-llm-agent instead."
```

---

### Task 6: Move SessionRecorderPlugin from vol-session to vol-llm-agent

**Files:**
- Delete: `crates/vol-session/src/recorder.rs`
- Create: `crates/vol-llm-agent/src/plugins/session_recorder.rs`
- Modify: `crates/vol-session/src/lib.rs:11,24`
- Modify: `crates/vol-session/Cargo.toml`

**Steps:**

- [ ] **Step 6.1: Create session_recorder.rs in vol-llm-agent**

Create `crates/vol-llm-agent/src/plugins/session_recorder.rs` with the content from `vol-session/src/recorder.rs`, changing `&PluginContext` → `&RunContext` and updating imports:

```rust
//! SessionRecorderPlugin — records agent events to session via AgentPlugin::listen().
//!
//! This plugin is not registered by default — callers may register it externally.

use async_trait::async_trait;
use vol_session::entry::{SessionEntry, RUN_ID_KEY};
use vol_session::{Session, SessionEntryStore, SessionMessage};
use vol_llm_core::{Message, ToolCall};

use super::plugin::{AgentPlugin, PluginDecision, PluginId};
use crate::react::plugin::RunContext;
use crate::AgentStreamEvent;

/// Plugin that records key agent events to the session entry store.
pub struct SessionRecorderPlugin {
    session: std::sync::Arc<Session>,
    entry_store: std::sync::Arc<dyn SessionEntryStore>,
}

impl SessionRecorderPlugin {
    pub fn new(session: std::sync::Arc<Session>, entry_store: std::sync::Arc<dyn SessionEntryStore>) -> Self {
        Self { session, entry_store }
    }

    fn should_record(event: &AgentStreamEvent) -> bool {
        matches!(
            event,
            AgentStreamEvent::AgentStart { .. }
                | AgentStreamEvent::ThinkingComplete { .. }
                | AgentStreamEvent::ContentComplete { .. }
                | AgentStreamEvent::ToolCallBegin { .. }
                | AgentStreamEvent::ToolCallComplete { .. }
                | AgentStreamEvent::ToolCallError { .. }
                | AgentStreamEvent::ToolCallSkipped { .. }
                | AgentStreamEvent::IterationComplete { .. }
        )
    }

    fn event_to_session_message(&self, event: &AgentStreamEvent) -> Option<SessionMessage> {
        // ... copy the existing implementation from vol-session/src/recorder.rs,
        // keeping the same logic ...
    }
}

#[async_trait]
impl AgentPlugin for SessionRecorderPlugin {
    fn id(&self) -> PluginId {
        "session_recorder".to_string()
    }

    fn priority(&self) -> u32 {
        0
    }

    async fn listen(&self, event: &AgentStreamEvent, ctx: &RunContext) {
        if !Self::should_record(event) {
            return;
        }

        let Some(msg) = self.event_to_session_message(event) else {
            return;
        };
        let msg = msg.with_metadata(RUN_ID_KEY, &ctx.run_id);
        let entry = SessionEntry::from_message(msg);

        if let Err(e) = self.entry_store.save(entry).await {
            tracing::error!("Failed to save session entry: {}", e);
        }
    }
}
```

- [ ] **Step 6.2: Export from plugins/mod.rs**

In `crates/vol-llm-agent/src/plugins/mod.rs`, add:
```rust
pub mod session_recorder;
pub use session_recorder::SessionRecorderPlugin;
```

- [ ] **Step 6.3: Delete recorder.rs from vol-session**

Run: `rm crates/vol-session/src/recorder.rs`

- [ ] **Step 6.4: Update vol-session/src/lib.rs**

Remove:
```rust
// Line 11:
pub mod recorder;
// Line 24:
pub use recorder::SessionRecorderPlugin;
```

- [ ] **Step 6.5: Update vol-session/Cargo.toml**

Remove `async-trait` if no longer needed (check if other files use it):
```bash
grep -rn "async_trait\|async-trait" crates/vol-session/src/ --include="*.rs"
```
If only recorder.rs used it, remove from Cargo.toml.

- [ ] **Step 6.6: Build vol-session**

Run: `cargo build -p vol-session --lib 2>&1 | grep -E "^error" | head -10`
Expected: No errors.

- [ ] **Step 6.7: Commit**

```bash
git add crates/vol-llm-agent/src/plugins/session_recorder.rs crates/vol-llm-agent/src/plugins/mod.rs crates/vol-session/src/recorder.rs crates/vol-session/src/lib.rs crates/vol-session/Cargo.toml
git commit -m "refactor: move SessionRecorderPlugin from vol-session to vol-llm-agent

Session should not know about the plugin concept. SessionRecorderPlugin is now
a plugin in vol-llm-agent that uses vol-session types."
```

---

### Task 7: Update vol-llm-observability — make it depend on vol-llm-agent

**Files:**
- Modify: `crates/vol-llm-observability/src/plugin.rs`
- Modify: `crates/vol-llm-observability/src/lib.rs`
- Modify: `crates/vol-llm-observability/Cargo.toml`

**Steps:**

- [ ] **Step 7.1: Add vol-llm-agent dependency**

In `crates/vol-llm-observability/Cargo.toml`, add:
```toml
vol-llm-agent = { path = "../vol-llm-agent" }
```

- [ ] **Step 7.2: Update plugin.rs — change AgentPlugin impl**

Replace imports (line 8):
```rust
// Before:
use vol_llm_core::plugin::{AgentPlugin, PluginContext, PluginDecision};
// After:
use vol_llm_agent::react::{AgentPlugin, RunContext, PluginDecision};
use vol_llm_agent::AgentStreamEvent;
```

Change trait impl signatures (lines 194, 198):
```rust
// Before:
async fn intercept(&self, _event: &AgentStreamEvent, _ctx: &PluginContext) -> PluginDecision {
async fn listen(&self, event: &AgentStreamEvent, ctx: &PluginContext) {
// After:
async fn intercept(&self, _event: &AgentStreamEvent, _ctx: &RunContext) -> PluginDecision {
async fn listen(&self, event: &AgentStreamEvent, ctx: &RunContext) {
```

Update `create_test_context()` in tests (lines 221-232):
```rust
// Before:
fn create_test_context() -> PluginContext {
    PluginContext {
        run_id: "test-run".to_string(),
        ...
    }
}

// After: construct RunContext::new()
fn create_test_context() -> RunContext {
    use std::sync::Arc;
    use vol_session::{InMemoryEntryStore, Session};
    use vol_llm_tool::ToolRegistry;
    use vol_llm_agent::react::AgentConfig;

    let (ctx, _rx) = RunContext::new(
        "test-run".to_string(),
        "test input".to_string(),
        "session-1".to_string(),
        Arc::new(Session::new(Arc::new(InMemoryEntryStore::new()))),
        Arc::new(ToolRegistry::new()),
        AgentConfig::default(),
    );
    ctx
}
```

Note: This requires vol-session and vol-llm-tool as dev-dependencies. Add them:
```toml
[dev-dependencies]
vol-session = { path = "../vol-session" }
vol-llm-tool = { path = "../vol-llm-tool" }
vol-llm-agent = { path = "../vol-llm-agent", features = ["test-utils"] }
```

Actually, vol-llm-agent test-utils feature may not exist. Check:
```bash
grep "test-utils" crates/vol-llm-agent/Cargo.toml
```
If not present, just use the regular dep and construct types manually.

- [ ] **Step 7.3: Build and test**

Run: `cargo build -p vol-llm-observability --lib 2>&1 | grep -E "^error" | head -10`
Expected: No errors.

- [ ] **Step 7.4: Commit**

```bash
git add crates/vol-llm-observability/src/plugin.rs crates/vol-llm-observability/Cargo.toml crates/vol-llm-observability/src/lib.rs
git commit -m "refactor: update vol-llm-observability LoggerPlugin to use RunContext, depend on vol-llm-agent"
```

---

### Task 8: Update vol-llm-agents

**Files:**
- Modify: `crates/vol-llm-agents/src/coding/observer_plugin.rs`
- Modify: `crates/vol-llm-agents/tests/observer_plugin_unit.rs`
- Modify: `crates/vol-llm-agents/tests/session_recording_test.rs` (if exists)

**Steps:**

- [ ] **Step 8.1: Update observer_plugin.rs**

Change imports (line 5):
```rust
// Before:
use vol_llm_agent::react::{AgentPlugin, PluginContext};
// After:
use vol_llm_agent::react::{AgentPlugin, RunContext};
```

Change trait impl (line 38):
```rust
// Before:
async fn listen(&self, event: &AgentStreamEvent, _ctx: &PluginContext) {
// After:
async fn listen(&self, event: &AgentStreamEvent, _ctx: &RunContext) {
```

- [ ] **Step 8.2: Update observer_plugin_unit.rs**

Change imports (line 4):
```rust
// Before:
use vol_llm_core::{AgentStreamEvent, PluginContext};
// After:
use vol_llm_agent::react::RunContext;
use vol_llm_agent::AgentStreamEvent;
```

Change `create_test_plugin_context()` (lines 118-142):
```rust
// Before:
fn create_test_plugin_context() -> vol_llm_agent::react::PluginContext {
    let (ctx, _plugin_rx) = RunContext::new(...);
    vol_llm_agent::react::plugin_context_from_run_ctx(&ctx)
}

// After:
fn create_test_plugin_context() -> RunContext {
    let (ctx, _rx) = RunContext::new(
        "test-run".to_string(),
        "test input".to_string(),
        "session-1".to_string(),
        Arc::new(Session::new(Arc::new(InMemoryEntryStore::new()))),
        Arc::new(ToolRegistry::new()),
        AgentConfig::default(),
    );
    ctx
}
```

- [ ] **Step 8.3: Check vol-llm-agents coding agent**

In `crates/vol-llm-agents/src/coding/agent.rs` line 354:
```rust
let logger = vol_llm_observability::LoggerPlugin::new(self.config.store_dir.clone());
self.config.plugin_registry.register(logger);
```
This still works — `LoggerPlugin` still exists in `vol_llm_observability` and still implements `AgentPlugin` (now from vol-llm-agent). No changes needed.

- [ ] **Step 8.4: Check for other PluginContext usages**

Run: `grep -rn "PluginContext\|plugin_context" crates/vol-llm-agents/ --include="*.rs"`
Fix any remaining references.

- [ ] **Step 8.5: Build and test**

Run: `cargo build -p vol-llm-agents --lib 2>&1 | grep -E "^error" | head -10`

- [ ] **Step 8.6: Commit**

```bash
git add crates/vol-llm-agents/src/coding/observer_plugin.rs crates/vol-llm-agents/tests/
git commit -m "refactor: update vol-llm-agents to use RunContext instead of PluginContext"
```

---

### Task 9: Verify vol-llm-tui

**Files:**
- Read: `crates/vol-llm-tui/src/app.rs`

**Steps:**

- [ ] **Step 9.1: Check TUI for broken imports**

Run: `grep -rn "PluginContext\|plugin_context\|with_observability\|ObservabilityPlugin" crates/vol-llm-tui/ --include="*.rs"`
Expected: Empty. TUI only uses `vol_llm_observability::LogEntry` for reading logs — no plugin types.

- [ ] **Step 9.2: Build TUI**

Run: `cargo build -p vol-llm-tui 2>&1 | grep -E "^error" | head -10`
Expected: No errors. TUI reads LogEntry from vol-llm-observability which still exports it.

- [ ] **Step 9.3: Commit only if changes needed**

---

### Task 10: Full workspace build and test

**Steps:**

- [ ] **Step 10.1: Full workspace build**

Run: `cargo build --workspace 2>&1 | grep -E "^error" | head -30`
Expected: No errors.

- [ ] **Step 10.2: Run all tests**

Run: `cargo test --workspace 2>&1 | tail -40`
Expected: All tests pass.

- [ ] **Step 10.3: Check for any remaining PluginContext references**

Run: `grep -rn "PluginContext\|plugin_context_from_run_ctx" crates/ --include="*.rs" | grep -v target`
Expected: Empty output.

- [ ] **Step 10.4: Verify dependency graph**

Run: `cargo tree -p vol-llm-agent 2>&1 | grep -E "vol-llm-observability|vol-session"`
Expected: `vol-session` appears (needed for Session), `vol-llm-observability` does NOT appear.

Run: `cargo tree -p vol-llm-observability 2>&1 | grep "vol-llm-agent"`
Expected: `vol-llm-agent` appears (observability now depends on agent).

- [ ] **Step 10.5: Final commit**

```bash
git status
git add -A
git commit -m "refactor: complete PluginContext → RunContext migration

- AgentPlugin trait moved from vol-llm-core to vol-llm-agent
- PluginContext deleted; trait methods accept &RunContext
- vol-llm-agent no longer depends on vol-llm-observability
- SessionRecorderPlugin moved from vol-session to vol-llm-agent
- All plugin implementations updated
- All tests updated
- Dead variables removed from agent.rs"
```

---

## Dependency Order

Tasks must be executed in this order:
1 → 2 → 3 → 4 → 5 → 6 → 7 → 8 → 9 → 10

Task 5 (remove observability from agent) and Task 6 (move SessionRecorderPlugin) can be done in parallel after Task 4. Task 7 (update observability) must come after Task 5 (since it adds the dep on agent).

## File Summary

| File | Action | Task |
|------|--------|------|
| `vol-llm-core/src/plugin.rs` | DELETE | 1 |
| `vol-llm-core/src/lib.rs` | Remove plugin module | 1 |
| `vol-llm-agent/src/react/plugin.rs` | REWRITE - define trait here | 1 |
| `vol-llm-agent/src/react/mod.rs` | Update re-exports | 1 |
| `vol-llm-agent/src/react/run_context.rs` | Delete plugin_context_from_run_ctx | 2 |
| `vol-llm-agent/src/react/agent.rs` | Remove dead vars, update ctx usage | 2 |
| `vol-llm-agent/src/react/plugin_stream.rs` | Change to RunContext params | 2 |
| `vol-llm-agent/src/plugins/caching.rs` | Update trait impl + tests | 3, 4 |
| `vol-llm-agent/src/plugins/rate_limiter.rs` | Update trait impl + tests | 3, 4 |
| `vol-llm-agent/src/plugins/retry.rs` | Update trait impl + tests | 3, 4 |
| `vol-llm-agent/src/react/hitl.rs` | Update trait impl | 3 |
| `vol-llm-agent/src/plugins/session_recorder.rs` | CREATE (moved from vol-session) | 6 |
| `vol-llm-agent/src/plugins/mod.rs` | Add session_recorder export | 6 |
| `vol-llm-agent/src/observability/` | DELETE entire dir | 5 |
| `vol-llm-agent/src/lib.rs` | Remove observability re-exports | 5 |
| `vol-llm-agent/Cargo.toml` | Remove vol-llm-observability dep | 5 |
| `vol-llm-agent/src/react/builder.rs` | Remove with_observability_plugin | 5 |
| `vol-llm-agent/examples/agent_observability_test.rs` | DELETE | 5 |
| `vol-llm-agent/examples/agent_cli_approval.rs` | Remove observability call | 5 |
| `vol-llm-agent/src/react/tests.rs` | Update all tests | 4 |
| `vol-llm-agent/tests/agent_run_tests.rs` | Update PluginContext → RunContext | 4 |
| `vol-llm-agent/tests/plugin_test.rs` | Update PluginContext → RunContext | 4 |
| `vol-llm-agent/tests/plugin_flow_test.rs` | Update PluginContext → RunContext | 4 |
| `vol-llm-agent/tests/react_mock_test.rs` | Update PluginContext → RunContext | 4 |
| `vol-llm-agent/tests/code_agent_simulation.rs` | Update PluginContext → RunContext | 4 |
| `vol-llm-agent/tests/session_recording_test.rs` | Update PluginContext → RunContext | 4 |
| `vol-llm-agent/tests/observability_integration.rs` | DELETE | 4 |
| `vol-session/src/recorder.rs` | DELETE (moved to agent) | 6 |
| `vol-session/src/lib.rs` | Remove recorder export | 6 |
| `vol-session/Cargo.toml` | Remove async-trait if unused | 6 |
| `vol-llm-observability/src/plugin.rs` | Update to use RunContext | 7 |
| `vol-llm-observability/Cargo.toml` | Add vol-llm-agent dep | 7 |
| `vol-llm-agents/src/coding/observer_plugin.rs` | Update to use RunContext | 8 |
| `vol-llm-agents/tests/observer_plugin_unit.rs` | Update to use RunContext | 8 |
| `vol-llm-tui/` | Check and fix if needed | 9 |
