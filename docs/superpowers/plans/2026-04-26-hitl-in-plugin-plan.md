# Move HITL Logic into Plugin System Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove all HITL/approval logic from the run loop and RunContext, consolidating it into HitlPlugin via the plugin intercept/decide pattern.

**Architecture:** `RunContext::intercept()` → plugins decide `PluginDecision::Continue/Skip/Abort`. `HitlPlugin.intercept()` carries its own `Arc<ToolRegistry>` to check sensitivity, uses a configurable `ApprovalCallback` to wait for user response. Run loop has zero HITL knowledge.

**Constraint:** `AgentPlugin` trait and `PluginContext` live in `vol-llm-core`. `PluginContext` cannot contain `ToolRegistry` (vol-llm-tool → vol-llm-core dep). The trait signature `intercept(&self, event, ctx: &PluginContext)` stays as-is. HitlPlugin carries its own `tools: Arc<ToolRegistry>` field.

**Tech Stack:** Rust, async, vol-llm-agent, vol-llm-tui, vol-llm-agents crates

---

### Task 1: Remove approval channel from RunContext

**Files:**
- Modify: `crates/vol-llm-agent/src/react/run_context.rs`

- [ ] **Step 1: Delete approval-related code**

Read `crates/vol-llm-agent/src/react/run_context.rs`. Remove:
- `CONTINUE_SENTINEL` constant
- `ApprovalRequest` / `ApprovalResponse` structs
- `approval_tx` field from `RunContext` struct
- `request_tool_approval()` method
- `request_continue_approval()` method
- Update `Clone` impl to remove `approval_tx` clone

Update `RunContext::new()` to return 2-tuple:

```rust
pub fn new(
    run_id: String,
    user_input: String,
    session_id: String,
    session: Arc<Session>,
    tools: Arc<ToolRegistry>,
    config: AgentConfig,
) -> (Self, mpsc::Receiver<PluginRequest>) {
    let (event_tx, _) = broadcast::channel(1024);
    let (plugin_event_tx, plugin_event_rx) = mpsc::channel(100);

    let ctx = Self {
        run_id,
        user_input,
        session_id,
        iteration: AtomicU32::new(0),
        all_tool_calls: Arc::new(RwLock::new(Vec::new())),
        current_tool_calls: Arc::new(RwLock::new(Vec::new())),
        data: Arc::new(RwLock::new(HashMap::new())),
        session,
        tools,
        config,
        event_tx,
        plugin_event_tx,
        reasoning_chain: Arc::new(RwLock::new(Vec::new())),
        tool_call_records: Arc::new(RwLock::new(Vec::new())),
        final_content: Arc::new(RwLock::new(None)),
        error: Arc::new(RwLock::new(None)),
        last_message_id: Arc::new(std::sync::Mutex::new(None)),
    };

    (ctx, plugin_event_rx)
}
```

- [ ] **Step 2: Update all test helpers**

Pattern: `(ctx, _rx, _approval_rx)` → `(ctx, _rx)`. Search:
```bash
grep -rn "approval_rx\|_approval_rx" crates/ --include="*.rs"
```

Update all matches in:
- `crates/vol-llm-agent/src/react/run_context.rs` tests
- `crates/vol-llm-agent/src/react/tests.rs`
- `crates/vol-llm-agent/src/react/agent.rs` (if any test-related code)
- `crates/vol-llm-agent/src/observability/plugin.rs` tests
- `crates/vol-llm-agent/tests/` (all test files using RunContext::new)

- [ ] **Step 3: Remove approval handler spawning from agent.rs**

In `agent.rs` `run()`, delete the entire Phase 1.5 block and update destructuring:

```rust
let (run_ctx, plugin_rx) = RunContext::new(...);
// No more approval_rx
```

- [ ] **Step 4: Verify and commit**

```bash
cargo test -p vol-llm-agent -- --test-threads=1
```

```bash
git add crates/vol-llm-agent/src/react/run_context.rs crates/vol-llm-agent/src/react/agent.rs
git commit -m "refactor: remove approval channel from RunContext and run loop"
```

---

### Task 2: Rewrite HitlPlugin with own tools + ApprovalCallback

**Files:**
- Rewrite: `crates/vol-llm-agent/src/react/hitl.rs`
- Modify: `crates/vol-llm-agent/src/react/mod.rs`

- [ ] **Step 1: Rewrite hitl.rs**

Replace entire content:

```rust
//! HitlPlugin — human-in-the-loop via plugin intercept.
//!
//! Carries its own Arc<ToolRegistry> to check tool sensitivity,
//! since PluginContext (in vol-llm-core) cannot depend on vol-llm-tool.

use std::sync::Arc;
use async_trait::async_trait;

use super::plugin::PluginDecision;
use super::stream::AgentStreamEvent;
use super::plugin::PluginContext;
use vol_llm_tool::{ToolRegistry, ToolSensitivity};

#[derive(Debug, Clone)]
pub struct ApprovalRequest {
    pub tool_name: String,
    pub reason: String,
    pub arguments: String,
}

/// Async callback for custom approval UIs (e.g., TUI, HTTP).
#[async_trait]
pub trait ApprovalCallback: Send + Sync {
    /// Show approval prompt and wait for user response.
    async fn approve(&self, request: &ApprovalRequest) -> bool;
}

#[derive(Default)]
pub struct HitlConfig {
    pub triggers: Vec<ApprovalTrigger>,
    pub timeout_secs: u64,
    pub on_timeout: TimeoutBehavior,
    pub approval_callback: Option<Arc<dyn ApprovalCallback>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ApprovalTrigger {
    ToolExecution { tools: Option<Vec<String>> },
    AfterIteration,
    BeforeFinalAnswer,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TimeoutBehavior {
    Approve,
    Reject { reason: String },
    Stop,
}

/// HitlPlugin — intercepts ToolCallBegin and IterationComplete events.
/// Carries its own tools reference to check sensitivity.
pub struct HitlPlugin {
    config: HitlConfig,
    tools: Arc<ToolRegistry>,
    unsafe_mode: bool,
}

impl HitlPlugin {
    pub fn new(config: HitlConfig, tools: Arc<ToolRegistry>, unsafe_mode: bool) -> Self {
        Self { config, tools, unsafe_mode }
    }

    fn needs_tool_approval(&self, tool_name: &str) -> bool {
        self.config.triggers.iter().any(|t| {
            if let ApprovalTrigger::ToolExecution { tools } = t {
                match tools { None => true, Some(list) => list.contains(&tool_name.to_string()) }
            } else { false }
        })
    }

    fn needs_iteration_pause(&self) -> bool {
        self.config.triggers.iter().any(|t| matches!(t, ApprovalTrigger::AfterIteration))
    }

    async fn wait_approval(&self, request: &ApprovalRequest) -> bool {
        if let Some(ref cb) = self.config.approval_callback {
            cb.approve(request).await
        } else {
            Self::cli_approve(request).await
        }
    }

    async fn cli_approve(request: &ApprovalRequest) -> bool {
        use std::io::{self, BufRead, Write};
        println!("\n\u{26a0} Approval required:");
        println!("  Tool: {}", request.tool_name);
        println!("  Reason: {}", request.reason);
        println!("  Args: {}", request.arguments);
        print!("  Approve? [y/n] > ");
        let _ = io::stdout().flush();
        let mut line = String::new();
        match io::stdin().lock().read_line(&mut line) {
            Ok(_) => { let t = line.trim().to_lowercase(); t == "y" || t == "yes" || t.is_empty() }
            Err(_) => false,
        }
    }

    async fn handle_tool_approval(&self, tool_name: &str, arguments: &str) -> PluginDecision {
        let args: serde_json::Value = serde_json::from_str(arguments)
            .unwrap_or(serde_json::json!({}));
        let sensitivity = self.tools.tool_sensitivity(tool_name, &args);
        match sensitivity {
            ToolSensitivity::RequiresApproval { reason } if self.needs_tool_approval(tool_name) => {
                let req = ApprovalRequest {
                    tool_name: tool_name.to_string(), reason, arguments: arguments.to_string(),
                };
                if self.wait_approval(&req).await {
                    PluginDecision::Continue
                } else {
                    PluginDecision::Abort(format!("User rejected tool: {}", tool_name))
                }
            }
            _ => PluginDecision::Continue,
        }
    }
}

#[async_trait]
impl super::plugin::AgentPlugin for HitlPlugin {
    fn id(&self) -> String { "human_in_loop".to_string() }
    fn priority(&self) -> u32 { 25 }

    async fn intercept(&self, event: &AgentStreamEvent, _ctx: &PluginContext) -> PluginDecision {
        if self.unsafe_mode { return PluginDecision::Continue; }
        match event {
            AgentStreamEvent::ToolCallBegin { tool_call_id, tool_name, arguments, .. } => {
                self.handle_tool_approval(tool_name, arguments).await
            }
            AgentStreamEvent::IterationComplete { iteration, final_answer, .. } => {
                if self.needs_iteration_pause() && final_answer.is_none() {
                    let req = ApprovalRequest {
                        tool_name: "continue".to_string(),
                        reason: format!("Iteration {} complete. Continue?", iteration),
                        arguments: String::new(),
                    };
                    if self.wait_approval(&req).await {
                        PluginDecision::Continue
                    } else {
                        PluginDecision::Abort(format!("User declined continuation after iteration {}", iteration))
                    }
                } else {
                    PluginDecision::Continue
                }
            }
            _ => PluginDecision::Continue,
        }
    }

    async fn listen(&self, _event: &AgentStreamEvent, _ctx: &PluginContext) {}
}

/// CLI approval callback using spawn_blocking.
pub struct CliApprovalCallback;

#[async_trait]
impl ApprovalCallback for CliApprovalCallback {
    async fn approve(&self, request: &ApprovalRequest) -> bool {
        let request = request.clone();
        tokio::task::spawn_blocking(move || {
            use std::io::{self, BufRead, Write};
            println!("\n\u{26a0} Approval required:");
            println!("  Tool: {}", request.tool_name);
            println!("  Reason: {}", request.reason);
            println!("  Args: {}", request.arguments);
            print!("  Approve? [y/n] > ");
            let _ = io::stdout().flush();
            let mut line = String::new();
            match io::stdin().lock().read_line(&mut line) {
                Ok(_) => { let t = line.trim().to_lowercase(); t == "y" || t == "yes" || t.is_empty() }
                Err(_) => false,
            }
        }).await.unwrap_or(false)
    }
}
```

- [ ] **Step 2: Update mod.rs exports**

Export: `HitlPlugin`, `HitlConfig`, `ApprovalTrigger`, `TimeoutBehavior`, `ApprovalRequest`, `ApprovalCallback`, `CliApprovalCallback`.
Remove: `ApprovalChannel`, `ApprovalHandler`, `BoxedApprovalHandler`, `spawn_custom_approval_handler`, `run_cli_approval_loop`.

- [ ] **Step 3: Verify and commit**

```bash
cargo check -p vol-llm-agent
```

```bash
git add crates/vol-llm-agent/src/react/hitl.rs crates/vol-llm-agent/src/react/mod.rs
git commit -m "refactor: rewrite HitlPlugin with ApprovalCallback and own tools reference"
```

---

### Task 3: Remove unsafe_mode/approval_handler from AgentConfig, update run loop

**Files:**
- Modify: `crates/vol-llm-agent/src/react/agent.rs`

- [ ] **Step 1: Remove unsafe_mode and approval_handler from AgentConfig**

```rust
pub struct AgentConfig {
    pub max_iterations: u32,
    pub max_history_messages: usize,
    pub context_builder: ContextBuilder,
    pub plugin_registry: PluginRegistry,
    pub agent_id: String,
    pub working_dir: PathBuf,
}
```

Update `Default` impl accordingly.

- [ ] **Step 2: Remove max-iterations request_continue_approval, replace with intercept**

```rust
if iteration > config.max_iterations {
    run_ctx.emit(AgentStreamEvent::max_iterations_reached(iteration, config.max_iterations)).await;
    match run_ctx.intercept(&AgentStreamEvent::iteration_complete(iteration, vec![], None)).await {
        Ok(PluginDecision::Continue) => {
            run_ctx.emit(AgentStreamEvent::iteration_continued(iteration)).await;
            run_ctx.reset_iteration();
            continue;
        }
        _ => {
            let reason = format!("Max iterations ({}) reached", config.max_iterations);
            run_ctx.emit(AgentStreamEvent::agent_aborted(reason.clone())).await;
            return Err(crate::AgentError::MaxIterationsReached { max: config.max_iterations });
        }
    }
}
```

- [ ] **Step 3: Remove tool sensitivity + approval from run loop**

Delete the `ToolSensitivity::RequiresApproval` → `request_tool_approval()` block. After `PluginDecision::Continue`, execute tool directly.

- [ ] **Step 4: Update tests**

Remove `unsafe_mode` / `approval_handler` from `test_agent_config_custom`.

- [ ] **Step 5: Verify and commit**

```bash
cargo test -p vol-llm-agent -- --test-threads=1
```

```bash
git add crates/vol-llm-agent/src/react/agent.rs
git commit -m "refactor: remove unsafe_mode from AgentConfig, HITL via plugin only"
```

---

### Task 4: Update callers — CodingAgent, TUI, plugins, tests

**Files:**
- Modify: `crates/vol-llm-agents/src/coding/config.rs`
- Modify: `crates/vol-llm-agents/src/coding/agent.rs`
- Modify: `crates/vol-llm-agents/src/coding/tests.rs`
- Modify: `crates/vol-llm-tui/src/approval.rs`
- Modify: `crates/vol-llm-tui/src/app.rs`
- Modify: `crates/vol-llm-tui/src/main.rs`
- Delete: `crates/vol-llm-agent/src/plugins/hitl_cli.rs`
- Delete: `crates/vol-llm-agent/src/plugins/hitl_http.rs`
- Modify: `crates/vol-llm-agent/src/plugins/mod.rs`
- Update test files

- [ ] **Step 1: Update CodingAgent config**

Read `crates/vol-llm-agents/src/coding/config.rs`. Replace `unsafe_mode`, `approval_handler`, `hitl_enabled` with:
```rust
pub hitl_config: Option<HitlConfig>,
```

Update Default: `hitl_config: Some(HitlConfig::default())`.
Update Debug impl.

- [ ] **Step 2: Update CodingAgent agent.rs**

Read `crates/vol-llm-agents/src/coding/agent.rs`. Remove `hitl_enabled()`, `unsafe_mode()`, `approval_handler()` builder methods. Add:
```rust
pub fn hitl_config(mut self, config: HitlConfig) -> Self {
    self.config.hitl_config = Some(config);
    self
}
```

In the AgentConfig construction (where `unsafe_mode` and `approval_handler` were set), remove them. Instead, if hitl_config is Some, create and register HitlPlugin:
```rust
if let Some(hitl_config) = self.config.hitl_config.clone() {
    // tools will be available at construction time
    // HitlPlugin is added after tools registry is built
}
```

Note: HitlPlugin needs Arc<ToolRegistry>. In CodingAgent, the tools registry is built before the agent is created. The hitl plugin should be registered to plugin_registry during agent construction.

- [ ] **Step 3: Update CodingAgent tests**

Read `crates/vol-llm-agents/src/coding/tests.rs`. Update `hitl_enabled(true)` calls → `hitl_config(HitlConfig::default())`.

- [ ] **Step 4: Update TUI approval.rs**

Read `crates/vol-llm-tui/src/approval.rs`. Replace `ApprovalHandler` impl with `ApprovalCallback`:

```rust
use vol_llm_agent::react::hitl::{ApprovalCallback, ApprovalRequest};

pub struct TuiApprovalCallback {
    state: ApprovalState,
}

#[async_trait]
impl ApprovalCallback for TuiApprovalCallback {
    async fn approve(&self, request: &ApprovalRequest) -> bool {
        if self.state.unsafe_mode.load(Ordering::Relaxed) { return true; }
        *self.state.tool_name.lock().await = Some(request.tool_name.clone());
        *self.state.reason.lock().await = Some(request.reason.clone());
        *self.state.arguments.lock().await = Some(request.arguments.clone());
        self.state.notify.notified().await;
        let resp = self.state.response.lock().await.take();
        matches!(resp, Some((true, _)))
    }
}
```

Update `ApprovalState` to create `TuiApprovalCallback` instead of `BoxedApprovalHandler`.

- [ ] **Step 5: Update TUI main.rs**

Replace `.unsafe_mode(unsafe_mode).approval_handler(...)` with creating `HitlConfig` with TUI callback and adding `HitlPlugin` to the agent's plugin registry.

- [ ] **Step 6: Delete hitl_cli.rs and hitl_http.rs**

Remove from `plugins/mod.rs`.

- [ ] **Step 7: Update all test files**

```bash
grep -rn "ApprovalHandler\|BoxedApprovalHandler\|approval_handler\|run_cli_approval_loop\|spawn_custom_approval\|ApprovalChannel" crates/ --include="*.rs"
```

Update all remaining matches.

- [ ] **Step 8: Verify and commit**

```bash
cargo check --workspace
cargo test --workspace -- --test-threads=1
```

```bash
git add crates/vol-llm-agents/src/coding/ crates/vol-llm-tui/src/ crates/vol-llm-agent/src/plugins/hitl_cli.rs crates/vol-llm-agent/src/plugins/hitl_http.rs crates/vol-llm-agent/src/plugins/mod.rs
git commit -m "refactor: update callers for plugin-based HITL system"
```
