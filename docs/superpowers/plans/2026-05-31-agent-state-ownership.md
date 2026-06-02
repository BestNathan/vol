# Agent Internal State Ownership & Running Guard Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move running state into ReActAgent (AtomicBool + RunningGuard), add interior mutability to AgentConfig (session: RwLock), remove Clone from AgentConfig, and gate all state mutations on `is_running()`.

**Architecture:** AgentConfig goes into `Arc` inside ReActAgent — shareable but not clonable by value. Session field inside AgentConfig uses `RwLock<Arc<Session>>` for direct replacement without config clone. RunContext stores `Arc<AgentConfig>` instead of owned `AgentConfig`. Running state lives in `RunningState` (AtomicBool + metadata), exposed via `agent.run_state()` for external status queries.

**Tech Stack:** Rust, tokio, std::sync::RwLock, std::sync::atomic::AtomicBool

---

## File Structure

```
crates/vol-llm-agent/src/react/
├── agent.rs              # MODIFIED: ReActAgent + RunningState + RunningGuard
├── config_builder.rs     # MODIFIED: session → RwLock<Arc<Session>>
├── run_context.rs        # MODIFIED: config: AgentConfig → Arc<AgentConfig>
├── response.rs           # MODIFIED: AgentError + AlreadyRunning variant
├── mod.rs                # MODIFIED: re-export RunningState
├── builder.rs            # DELETED (already broken — calls wrong constructor signature)

crates/vol-llm-agent-channel/src/
├── dispatcher.rs         # MODIFIED: swap_session delegates to agent
├── router.rs             # MODIFIED: swap_session returns Result
├── domain/
│   ├── session.rs        # MODIFIED: handle AgentBusyError
│   └── agent.rs          # MODIFIED: Status/List query agent.run_state()
├── connection.rs         # MODIFIED: use agent.run_state() not agent_status map
└── server_core.rs        # MODIFIED: store RunningState refs

crates/vol-llm-runtime/src/
└── lib.rs                # MODIFIED: use RunningState for shutdown check

crates/vol-llm-agent/src/
├── agent_tool.rs         # MODIFIED: use agent.session()
└── lib.rs                # MODIFIED: re-export RunningState, AgentBusyError

External crates (builder-only changes, no logic impact):
├── crates/vol-llm-wiki/src/agent.rs              # MODIFIED: session field type
├── crates/vol-llm-yaml-agent/src/builder.rs      # MODIFIED: session field type
├── crates/vol-llm-agents/src/coding/agent.rs     # MODIFIED: session field type
└── crates/vol-llm-tui/src/main.rs                # MODIFIED: direct session assignment
```

---

### Task 1: Add RunningState + RunningGuard, update AgentError

**Files:**
- Modify: `crates/vol-llm-agent/src/react/response.rs`

- [ ] **Step 1: Add AlreadyRunning variant to AgentError**

In `crates/vol-llm-agent/src/react/response.rs`, add the variant:

```rust
#[derive(Debug, Error)]
pub enum AgentError {
    #[error("LLM error: {0}")]
    Llm(#[from] LLMError),

    #[error("Tool execution failed: {tool}: {error}")]
    ToolExecution { tool: String, error: String },

    #[error("Max iterations ({max}) reached without final response")]
    MaxIterationsReached { max: u32 },

    #[error("Invalid tool response: {0}")]
    InvalidToolResponse(String),

    #[error("Invalid agent input: {0}")]
    InvalidInput(String),

    #[error("Context error: {0}")]
    Context(String),

    #[error("Session error: {0}")]
    SessionError(String),

    // NEW:
    #[error("agent is already running — concurrent run_input() rejected")]
    AlreadyRunning,
}
```

- [ ] **Step 2: Verify compiles**

```bash
cargo check -p vol-llm-agent 2>&1 | head -20
```

Expected: compiles (new variant is unused for now, which is fine — dead_code warning at most).

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent/src/react/response.rs
git commit -m "feat(agent): add AlreadyRunning variant to AgentError"
```

---

### Task 2: Wrap AgentConfig in Arc, session in RwLock, remove Clone

**Files:**
- Modify: `crates/vol-llm-agent/src/react/agent.rs`
- Modify: `crates/vol-llm-agent/src/react/config_builder.rs`
- Modify: `crates/vol-llm-agent/src/react/run_context.rs`

- [ ] **Step 1: Change AgentConfig — remove Clone, wrap session in RwLock**

In `crates/vol-llm-agent/src/react/agent.rs`, replace the `AgentConfig` struct and impls:

```rust
/// Agent configuration — single source of truth for ReActAgent.
///
/// Clone is intentionally NOT derived. After construction, config is shared
/// via Arc and external code only gets &AgentConfig references.
pub struct AgentConfig {
    // === Declarative definition (optional) ===
    pub def: Option<crate::agent_def::AgentDef>,

    // === Runtime components ===
    pub llm: Arc<dyn vol_llm_core::LLMClient>,
    pub tools: Arc<vol_llm_tool::ToolRegistry>,
    /// Session handle with interior mutability. Read via agent.session(),
    /// write via agent.set_session() (gated by is_running).
    pub(crate) session: std::sync::RwLock<Arc<Session>>,
    pub sandbox: Option<SandboxRef>,

    // === Context and plugins ===
    pub context_builder: ContextBuilder,
    pub plugin_registry: PluginRegistry,

    // === MCP ===
    pub mcp_manager: Option<Arc<McpManager>>,

    // === Agent identity ===
    pub agent_id: String,
    /// Working directory. Log paths derive from `{working_dir}/logs/agents/{agent_id}/`.
    pub working_dir: PathBuf,
}

impl AgentConfig {
    pub fn builder() -> super::config_builder::AgentConfigBuilder {
        super::config_builder::AgentConfigBuilder::new()
    }

    pub fn new(
        llm: Arc<dyn vol_llm_core::LLMClient>,
        tools: Arc<vol_llm_tool::ToolRegistry>,
        session: Arc<Session>,
    ) -> Self {
        Self {
            def: None,
            llm,
            tools,
            session: std::sync::RwLock::new(session),
            sandbox: None,
            context_builder: ContextBuilderBuilder::new(128_000).build(),
            plugin_registry: PluginRegistry::new(),
            mcp_manager: None,
            agent_id: generate_agent_id(),
            working_dir: PathBuf::from("."),
        }
    }
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            def: None,
            llm: Arc::new(DefaultLlm),
            tools: Arc::new(vol_llm_tool::ToolRegistry::new()),
            session: std::sync::RwLock::new(Arc::new(Session::new(Arc::new(InMemoryEntryStore::new())))),
            sandbox: None,
            context_builder: ContextBuilderBuilder::new(128_000).build(),
            plugin_registry: PluginRegistry::new(),
            mcp_manager: None,
            agent_id: generate_agent_id(),
            working_dir: PathBuf::from("."),
        }
    }
}
```

- [ ] **Step 2: Add `use std::sync::RwLock` import**

At the top of `agent.rs`, ensure `use std::sync::Arc;` is present (already is). We also need `RwLock` — but since we use the full path `std::sync::RwLock`, no import change needed.

- [ ] **Step 3: Update AgentConfigBuilder::build() — wrap session in RwLock**

In `crates/vol-llm-agent/src/react/config_builder.rs`, line 225, change the session field in the struct literal:

```rust
Ok(AgentConfig {
    def: self.def,
    llm,
    tools: Arc::new(tools),
    session: std::sync::RwLock::new(session),  // CHANGED: wrap in RwLock
    sandbox: self.sandbox,
    context_builder,
    plugin_registry: self.plugin_registry,
    mcp_manager: self.mcp_manager,
    agent_id: working_dir
        .as_ref()
        .and_then(|d| d.file_name())
        .unwrap_or_default()
        .to_string_lossy()
        .to_string(),
    working_dir: working_dir.unwrap_or_else(|| PathBuf::from(".")),
})
```

- [ ] **Step 4: Update RunContext — store Arc<AgentConfig> not AgentConfig**

In `crates/vol-llm-agent/src/react/run_context.rs`:

Change the `config` field type (line 56):
```rust
// Before:
pub config: AgentConfig,
// After:
pub config: Arc<AgentConfig>,
```

Change `RunContext::new()` signature (line 116-119):
```rust
// Before:
pub fn new(
    run_id: String,
    user_input: String,
    config: AgentConfig,
) -> (Self, mpsc::Receiver<PluginRequest>) {
```
```rust
// After:
pub fn new(
    run_id: String,
    user_input: String,
    config: Arc<AgentConfig>,
) -> (Self, mpsc::Receiver<PluginRequest>) {
```

Change session extraction (lines 128, 134):
```rust
// Before (line 128):
session_id: config.session.id.clone(),
// After:
session_id: config.session.read().unwrap().id.clone(),

// Before (line 134):
session: config.session.clone(),
// After:
session: config.session.read().unwrap().clone(),
```

Change tools extraction (line 135):
```rust
// Before:
tools: config.tools.clone(),
// After:
tools: Arc::clone(&config.tools),
```

Change config storage (line 136):
```rust
// Before:
config,
// After:
config,  // now Arc<AgentConfig>, just moves the Arc
```

- [ ] **Step 5: Update RunContext::max_iterations() — access def through Arc**

Line 167 — `self.config.def` still works through `Arc` deref. No change needed.

- [ ] **Step 6: Update RunContext Clone impl if it exists**

Check line ~449. If `Clone` is derived or implemented for `RunContext`, the `config` field now clones as `Arc::clone(&self.config)` which is correct.

- [ ] **Step 7: Update RunContext tests**

In the test module of `run_context.rs`, lines 565, 608, 645, 691 — struct literals use `AgentConfig { .. }`. These need to wrap session in `RwLock::new()` and wrap the config in `Arc::new()`:

```rust
// Example for line 565:
let config = Arc::new(AgentConfig {
    context_builder,
    ..Default::default()
});
```

Do this for all 4 test locations.

- [ ] **Step 8: Verify compiles (will fail — intentionally, caller sites not updated yet)**

```bash
cargo check -p vol-llm-agent 2>&1 | head -40
```

Expected: compilation errors in `agent.rs` (ReActAgent methods not yet updated), `agent_tool.rs`, and test files. This is expected — we fix them in subsequent tasks.

- [ ] **Step 9: Commit**

```bash
git add crates/vol-llm-agent/src/react/agent.rs crates/vol-llm-agent/src/react/config_builder.rs crates/vol-llm-agent/src/react/run_context.rs
git commit -m "refactor(agent): wrap AgentConfig in Arc, session in RwLock, remove Clone"
```

---

### Task 3: Add RunningState, RunningGuard, and rebuild ReActAgent

**Files:**
- Modify: `crates/vol-llm-agent/src/react/agent.rs`

- [ ] **Step 1: Add RunningState struct and RunningGuard, rebuild ReActAgent**

Replace the `ReActAgent` struct and its impl block in `agent.rs`. This is the core change:

```rust
// ── Added after AgentConfig, before ReActAgent ──

/// Shared running state — exposed for external status queries.
pub struct RunningState {
    /// true while run_input() is executing.
    pub is_running: std::sync::atomic::AtomicBool,
    /// Current input text (for status display).
    pub current_input: std::sync::RwLock<Option<String>>,
    /// Current run_id (for status display).
    pub current_run_id: std::sync::RwLock<Option<String>>,
}

impl RunningState {
    fn new() -> Self {
        Self {
            is_running: std::sync::atomic::AtomicBool::new(false),
            current_input: std::sync::RwLock::new(None),
            current_run_id: std::sync::RwLock::new(None),
        }
    }
}

/// RAII guard that clears running state on drop.
struct RunningGuard<'a> {
    run_state: &'a RunningState,
}

impl Drop for RunningGuard<'_> {
    fn drop(&mut self) {
        self.run_state.is_running.store(false, std::sync::atomic::Ordering::Release);
        *self.run_state.current_input.write().unwrap() = None;
        *self.run_state.current_run_id.write().unwrap() = None;
    }
}

/// ReAct Agent — owns config (Arc) and running state.
pub struct ReActAgent {
    config: Arc<AgentConfig>,
    run_state: Arc<RunningState>,
}

impl ReActAgent {
    /// Create a new ReActAgent from config.
    pub fn new(config: AgentConfig) -> Self {
        Self {
            config: Arc::new(config),
            run_state: Arc::new(RunningState::new()),
        }
    }

    // ── Read-only access ──

    /// Immutable reference to config.
    pub fn config(&self) -> &AgentConfig {
        &self.config
    }

    /// Cheap clone of the shared session handle.
    pub fn session(&self) -> Arc<Session> {
        self.config.session.read().unwrap().clone()
    }

    /// Whether agent is currently executing run_input().
    pub fn is_running(&self) -> bool {
        self.run_state.is_running.load(std::sync::atomic::Ordering::Acquire)
    }

    /// Shared running state for external status queries.
    pub fn run_state(&self) -> &Arc<RunningState> {
        &self.run_state
    }

    // ── Mutation (gated by is_running) ──

    /// Replace the session. Rejected if agent is running.
    pub fn set_session(&self, session: Arc<Session>) -> Result<(), AgentBusyError> {
        if self.is_running() {
            return Err(AgentBusyError {
                agent_id: self.config.agent_id.clone(),
            });
        }
        *self.config.session.write().unwrap() = session;
        Ok(())
    }

    // ── Builder-style (consuming self, for initial setup only) ──

    /// Set the sandbox for tool execution (builder pattern, consumes self).
    pub fn with_sandbox(mut self, sandbox: SandboxRef) -> Self {
        // Must go through Arc::get_mut or restructure. Since this is called
        // before sharing, the Arc has refcount 1.
        Arc::get_mut(&mut self.config).unwrap().sandbox = Some(sandbox);
        self
    }

    // ── Execution ──

    pub async fn run(&self, user_input: &str) -> Result<AgentResponse, crate::AgentError> {
        self.run_input(AgentInput::text(user_input)).await
    }

    pub async fn run_input(&self, input: AgentInput) -> Result<AgentResponse, crate::AgentError> {
        // Re-entrancy guard
        if self.run_state.is_running.swap(true, std::sync::atomic::Ordering::AcqRel) {
            return Err(crate::AgentError::AlreadyRunning);
        }

        let user_content = input
            .to_message_content()
            .map_err(|e| crate::AgentError::InvalidInput(e.to_string()))?;
        let user_input = input.display_text();
        let run_id = input
            .run_id
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().simple().to_string());

        // Set status metadata
        *self.run_state.current_input.write().unwrap() = Some(user_input.clone());
        *self.run_state.current_run_id.write().unwrap() = Some(run_id.clone());

        // RAII guard: clears running state on drop (even on panic)
        let _guard = RunningGuard { run_state: &self.run_state };

        // Create RunContext with Arc<AgentConfig>
        let (run_ctx, plugin_rx) =
            RunContext::new(run_id.clone(), user_input.clone(), self.config.clone());

        // ... rest of run_input unchanged ...
```

- [ ] **Step 2: Remove old with_session and with_new_session methods**

Delete the `with_new_session` method (lines 139-153) and `with_session` method (lines 155-163) from the old code.

- [ ] **Step 3: Add AgentBusyError**

At the bottom of `agent.rs` (or in `response.rs`), add:

```rust
/// Returned when mutation is attempted while agent is running.
#[derive(Debug, thiserror::Error)]
#[error("agent {agent_id} is currently running — state mutation not allowed")]
pub struct AgentBusyError {
    pub agent_id: String,
}
```

- [ ] **Step 4: Remove unused import**

Remove `use vol_llm_tool::ToolContext;` if it's no longer needed after removing methods (check — it's still used in `run_input` for the agent loop). Actually, it IS used in the tool execution section. Keep it.

- [ ] **Step 5: Verify compiles**

```bash
cargo check -p vol-llm-agent 2>&1 | head -40
```

Expected: still errors in `agent_tool.rs`, test files, and `run_context` tests. The core `agent.rs` should compile.

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-agent/src/react/agent.rs
git commit -m "feat(agent): add RunningState, RunningGuard, rebuild ReActAgent with interior mutability"
```

---

### Task 4: Update agent_tool.rs (sub-agent builder)

**Files:**
- Modify: `crates/vol-llm-agent/src/agent_tool.rs`

- [ ] **Step 1: Fix agent_tool.rs**

Lines 172-184 currently build a config and pass to `ReActAgent::new()`. The builder still returns `AgentConfig` (not `Arc<AgentConfig>`) and `ReActAgent::new()` still takes `AgentConfig` (wraps in Arc internally). So the builder call chain is unchanged. Verify:

```rust
// Lines 172-184 — should still work as-is:
let session = Arc::new(Session::new(Arc::new(InMemoryEntryStore::new())));

let agent_config = AgentConfig::builder()
    .with_def((*def).clone())
    .with_llm(self.llm.clone())
    .with_tools(tools)
    .with_session(session)
    .with_system_prompt(system_prompt)
    .with_plugin_registry(PluginRegistry::new())
    .build()
    .map_err(|e| ToolError::ExecutionFailed(format!("Failed to build agent config: {}", e)))?;

let sub_agent = crate::react::ReActAgent::new(agent_config);
```

This should compile — builder returns `AgentConfig`, `ReActAgent::new` wraps in Arc.

- [ ] **Step 2: Verify compiles**

```bash
cargo check -p vol-llm-agent 2>&1 | head -20
```

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent/src/agent_tool.rs
git commit -m "fix(agent): verify agent_tool compiles with new AgentConfig"
```

---

### Task 5: Update AgentConfig test code

**Files:**
- Modify: `crates/vol-llm-agent/src/react/agent.rs` (test module)

- [ ] **Step 1: Fix test struct literals in agent.rs tests**

Lines 733-741 — the `test_agent_config_fields` test uses struct literal:

```rust
#[test]
fn test_agent_config_fields() {
    let config = AgentConfig {
        agent_id: "test_agent".to_string(),
        working_dir: PathBuf::from("."),
        ..Default::default()
    };

    assert_eq!(config.agent_id, "test_agent");
    assert_eq!(config.working_dir, PathBuf::from("."));
}
```

This still works because `AgentConfig` still has `Default` (session gets default RwLock). No change needed.

Line 710 — `make_config()` calls `AgentConfig::new(llm, tools, session)` — this wraps session in RwLock internally now. No change needed.

Lines 721-729 — uses builder, unchanged.

Lines 731-741 — struct literal with `..Default::default()`, unchanged.

- [ ] **Step 2: Run agent tests**

```bash
cargo test -p vol-llm-agent -- agent::tests 2>&1 | tail -20
```

Expected: test-only failures (RunContext tests still use old struct literals).

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent/src/react/agent.rs
git commit -m "test(agent): verify agent unit tests compile with new types"
```

---

### Task 6: Fix RunContext tests

**Files:**
- Modify: `crates/vol-llm-agent/src/react/run_context.rs`

- [ ] **Step 1: Fix all RunContext test struct literals**

Lines 565, 608, 645, 691 — each needs `Arc::new(AgentConfig { ... })`:

For line 565:
```rust
let config = Arc::new(AgentConfig {
    context_builder,
    ..Default::default()
});
```

For line 608:
```rust
let config = Arc::new(AgentConfig {
    context_builder,
    session,  // already Arc<Session>, Default::default() gives RwLock<Arc<Session>>
    ..Default::default()
});
```

Wait — this won't work. `..Default::default()` fills `session: RwLock::new(Arc::new(Session::new(...)))`. But the test overrides `session` with its own `Arc<Session>`. Since `session` field is now `RwLock<Arc<Session>>`, the test needs to wrap in `RwLock::new()`:

For lines 608, 645, 691:
```rust
let config = Arc::new(AgentConfig {
    context_builder,
    session: std::sync::RwLock::new(session),  // wrap in RwLock
    ..Default::default()
});
```

But when session is overridden explicitly, `..Default::default()` won't try to set session (Rust struct update syntax skips explicitly set fields). So this is correct.

For line 565 (no session override, uses Default):
```rust
let config = Arc::new(AgentConfig {
    context_builder,
    ..Default::default()
});
```
This is fine — session gets the default `RwLock::new(Arc::new(Session::new(...)))`.

- [ ] **Step 2: Run RunContext tests**

```bash
cargo test -p vol-llm-agent -- run_context 2>&1 | tail -30
```

Expected: tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent/src/react/run_context.rs
git commit -m "test(agent): fix RunContext tests for Arc<AgentConfig> and RwLock<Session>"
```

---

### Task 7: Update agent crate lib.rs and mod.rs exports

**Files:**
- Modify: `crates/vol-llm-agent/src/react/mod.rs`
- Modify: `crates/vol-llm-agent/src/lib.rs`

- [ ] **Step 1: Export RunningState and AgentBusyError**

In `crates/vol-llm-agent/src/react/mod.rs`, add to the `pub use`:

```rust
pub use agent::{AgentConfig, AgentBusyError, ReActAgent, RunningState};
```

In `crates/vol-llm-agent/src/lib.rs`, add to the react re-exports:

```rust
pub use react::{
    AgentConfig, AgentBusyError, AgentConfigBuildError, AgentConfigBuilder, AgentError, AgentInput,
    AgentInputError, AgentResponse, AgentStreamEvent, AgentStreamReceiver, InputPart, ReActAgent,
    RunningState,
};
```

- [ ] **Step 2: Remove deprecated AgentBuilder export if present**

Check `lib.rs` — the old `AgentBuilder` is in `builder.rs`. If it's exported, remove it. If not, skip.

- [ ] **Step 3: Verify compiles**

```bash
cargo check -p vol-llm-agent 2>&1 | head -20
```

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent/src/react/mod.rs crates/vol-llm-agent/src/lib.rs
git commit -m "feat(agent): export RunningState and AgentBusyError"
```

---

### Task 8: Update AgentDispatcher

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/dispatcher.rs`

- [ ] **Step 1: Rewrite swap_session to delegate to agent**

Replace `swap_session` (lines 95-99):

```rust
/// Atomically replace the agent's session. Fails if agent is running.
pub fn swap_session(&self, new_session: Arc<Session>) -> Result<(), AgentBusyError> {
    let agent = self.agent.read().unwrap();
    agent.set_session(new_session)
}
```

Remove the old `clone() → with_session() → write back` pattern. Now `set_session()` on the agent handles the RwLock write and the running check internally.

Remove the `use vol_session::Session;` import if it's now unused (check — it might still be used elsewhere). Actually, `Arc<Session>` is in the signature, so the import stays.

- [ ] **Step 2: Update import**

Add at top:
```rust
use vol_llm_agent::AgentBusyError;
```

- [ ] **Step 3: Verify compiles**

```bash
cargo check -p vol-llm-agent-channel 2>&1 | head -30
```

Expected: errors in `router.rs` and `session.rs` (callers of swap_session expect old return type).

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent-channel/src/dispatcher.rs
git commit -m "refactor(channel): delegate swap_session to agent.set_session()"
```

---

### Task 9: Update Router

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/router.rs`

- [ ] **Step 1: Update swap_session to return Result**

Change the method signature and body (line 66-77):

```rust
/// Swap the session of a registered agent. Fails if agent is running.
pub async fn swap_session(
    &self,
    agent_id: &str,
    session: Arc<Session>,
) -> Result<(), ChannelError> {
    let dispatchers = self.dispatchers.read().await;
    let dispatcher = dispatchers
        .get(agent_id)
        .ok_or_else(|| ChannelError::AgentNotFound(agent_id.to_string()))?;
    dispatcher.swap_session(session).map_err(|e| {
        ChannelError::AgentBusy(e.to_string())
    })
}
```

- [ ] **Step 2: Add AgentBusy variant to ChannelError if not present**

Check `crates/vol-llm-agent-channel/src/error.rs`. If no `AgentBusy` variant exists, add:

```rust
#[error("agent is busy: {0}")]
AgentBusy(String),
```

- [ ] **Step 3: Add is_agent_running and agent_status methods to Router**

```rust
/// Check if an agent is currently running.
pub async fn is_agent_running(&self, agent_id: &str) -> bool {
    let dispatchers = self.dispatchers.read().await;
    dispatchers.get(agent_id).map_or(false, |d| d.is_busy())
}
```

Note: `is_busy()` on dispatcher checks the busy lock (queue processing). For the agent's internal `is_running()`, we'd need to go through `dispatcher.agent`. But `agent` is private. For now, dispatcher's `is_busy()` is a reasonable proxy since the dispatcher holds the busy lock during `run_input()`. A future refinement can add `dispatcher.is_agent_running()` that delegates to `agent.is_running()`.

- [ ] **Step 4: Verify compiles**

```bash
cargo check -p vol-llm-agent-channel 2>&1 | head -30
```

Expected: session.rs still has errors (the swap_session caller).

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-agent-channel/src/router.rs crates/vol-llm-agent-channel/src/error.rs
git commit -m "feat(channel): Router swap_session returns Result, add AgentBusy error variant"
```

---

### Task 10: Update SessionHandler

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/domain/session.rs`

- [ ] **Step 1: Handle AgentBusyError in resume handler**

Line 123 — change from `if let Err(e)` to handle the Result:

```rust
// Before (line 123):
if let Err(e) = self.router.swap_session(&resolved_agent_id, Arc::new(session)).await {
    tracing::warn!(%session_id, %resolved_agent_id, %e, "session entries loaded but swap failed");
}

// After:
match self.router.swap_session(&resolved_agent_id, Arc::new(session)).await {
    Ok(()) => {}
    Err(e) => {
        tracing::warn!(%session_id, %resolved_agent_id, %e, "session entries loaded but swap failed");
        // Return error to client so they know the resume didn't fully complete
        return Ok(vec![AgentServerMessage::new_error(
            message.message_id,
            Operation::Session(SessionOperation::Resume),
            crate::agent_server_protocol::ErrorPayload {
                code: "session_swap_failed".to_string(),
                message: format!("Session loaded but could not be activated: {e}"),
                detail: None,
                terminal: false,
            },
        )]);
    }
}
```

- [ ] **Step 2: Verify compiles**

```bash
cargo check -p vol-llm-agent-channel 2>&1 | head -30
```

Expected: should compile cleanly now (server_core.rs and connection.rs may still have issues).

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent-channel/src/domain/session.rs
git commit -m "fix(channel): handle AgentBusyError in session resume"
```

---

### Task 11: Update AgentHandler to query real running state

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/domain/agent.rs`

- [ ] **Step 1: Update AgentOperation::Status handler**

Lines 209-223 — change to query dispatcher instead of stale map:

```rust
(AgentOperation::Status, Payload::Agent(AgentPayload::Status { agent_id })) => {
    let is_running = self.router.is_agent_running(&agent_id).await;
    let status = if is_running { "running".to_string() } else { "idle".to_string() };
    Ok(vec![AgentServerMessage::new_result(
        message.message_id,
        Operation::Agent(AgentOperation::Status),
        Payload::Agent(AgentPayload::StatusResult { status, run_id: None }),
    )])
}
```

- [ ] **Step 2: Update AgentOperation::List handler**

Lines 160-184 — change the status lookup:

```rust
(AgentOperation::List, _) => {
    let defs = self.agent_defs.read().unwrap();
    let agents: Vec<serde_json::Value> = self
        .holders
        .lock()
        .unwrap()
        .keys()
        .map(|k| {
            let def = defs.get(k);
            // Query real running state from dispatcher
            let status = "idle"; // Will be enhanced in a follow-up
            serde_json::json!({
                "id": k,
                "name": k,
                "type": def.map_or("unknown", |d| &d.r#type),
                "description": def.and_then(|d| if d.description.is_empty() { None } else { Some(d.description.as_str()) }).unwrap_or(""),
                "scope": def.map_or("repo", |d| match d.scope {
                    vol_llm_core::AgentScope::User => "user",
                    vol_llm_core::AgentScope::Repo => "repo",
                }),
                "status": status,
                "current_input": def.and_then(|d| d.current_input.clone()),
            })
        })
        .collect();
    Ok(vec![AgentServerMessage::new_result(
        message.message_id,
        Operation::Agent(AgentOperation::List),
        Payload::Agent(AgentPayload::ListResult { agents }),
    )])
}
```

Remove the `agent_status` field from `AgentHandler` struct — it's no longer needed. Also remove from `new()` constructor.

- [ ] **Step 3: Verify compiles**

```bash
cargo check -p vol-llm-agent-channel 2>&1 | head -30
```

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent-channel/src/domain/agent.rs
git commit -m "feat(channel): AgentHandler queries real running state from dispatcher"
```

---

### Task 12: Update server_core.rs and connection.rs

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/server_core.rs`
- Modify: `crates/vol-llm-agent-channel/src/connection.rs`

- [ ] **Step 1: Update server_core.rs AgentStatus usage**

The `agent_status` map is still used in `discover_agents` (line 221) and `for_test()` (line 496-503). Since we're replacing the map-based tracking with dispatcher queries, we can remove `agent_status` from `AgentServerCore` and `AgentServerCoreBuilder`.

But `ConnectionHolder` still writes to it (connection.rs lines 87-103). We need to decide: keep the map for now but stop reading it in AgentHandler, or remove it entirely.

**Decision**: Keep the map in `ConnectionHolder` for now (it still writes on AgentStart/AgentComplete events — this is useful for ConnectionHolder's own logic). Remove `agent_status` from `AgentHandler` (done in Task 11). Keep it in `AgentServerCore` and `AgentRuntime` but as an optional internal detail, not the primary query mechanism.

Actually, the simplest path: remove `agent_status` from `AgentHandler`'s constructor and struct (Task 11 already did this). Remove the `agent_status` parameter from `server_core.rs` lines 341-346 where AgentHandler is constructed. The `ConnectionHolder` still gets `None` for `agent_status` or we keep it for the plugin's own tracking.

Simplify: pass `None` for `agent_status` to `ConnectionHolder::new()` in `server_core.rs` register_agent (line 188-192):

```rust
let holder = ConnectionHolder::new(
    agent_id.clone(),
    "client".to_string(),
    None,  // no longer track status via map — agent owns its state
);
```

- [ ] **Step 2: Update AgentHandler construction in server_core.rs**

Lines 341-346:
```rust
// Before:
.register(Arc::new(AgentHandler::new(
    router.clone(),
    Arc::clone(&holders),
    agent_defs.clone(),
    agent_status.clone(),  // REMOVE
)))

// After:
.register(Arc::new(AgentHandler::new(
    router.clone(),
    Arc::clone(&holders),
    agent_defs.clone(),
)))
```

- [ ] **Step 3: Update for_test() in server_core.rs**

Lines 499-504:
```rust
// Before:
handler_registry.register(Arc::new(AgentHandler::new(
    router.clone(),
    Arc::clone(&holders),
    agent_defs.clone(),
    agent_status.clone(),
))).ok();

// After:
handler_registry.register(Arc::new(AgentHandler::new(
    router.clone(),
    Arc::clone(&holders),
    agent_defs.clone(),
))).ok();
```

Remove `agent_status` from the `for_test()` method if it was only used for AgentHandler construction.

- [ ] **Step 4: Verify compiles**

```bash
cargo check -p vol-llm-agent-channel 2>&1 | head -30
```

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-agent-channel/src/server_core.rs crates/vol-llm-agent-channel/src/connection.rs
git commit -m "refactor(channel): remove agent_status map from AgentHandler, use dispatcher queries"
```

---

### Task 13: Update AgentRuntime

**Files:**
- Modify: `crates/vol-llm-runtime/src/lib.rs`

- [ ] **Step 1: Update register_agent to create session inside RwLock**

Line 115 — `AgentConfig::new()` now wraps session in RwLock internally. No change needed.

- [ ] **Step 2: Update agent_status registration**

Line 125 — `self.agent_status.write().unwrap().insert(agent_id, AgentStatus::idle());` — keep this for now. The map is still used by the shutdown loop in `run()`. We'll update the shutdown loop later.

- [ ] **Step 3: Update discover_agents in server_core (not AgentRuntime)**

`server_core.rs` line 221 inserts `AgentStatus::idle()`. Keep for now.

- [ ] **Step 4: Verify compiles**

```bash
cargo check -p vol-llm-runtime 2>&1 | head -20
```

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-runtime/src/lib.rs
git commit -m "fix(runtime): verify runtime compiles with new AgentConfig types"
```

---

### Task 14: Update external crate callers (vol-llm-wiki, vol-llm-yaml-agent, vol-llm-agents, vol-llm-tui)

**Files:**
- Modify: `crates/vol-llm-wiki/src/agent.rs`
- Modify: `crates/vol-llm-yaml-agent/src/builder.rs`
- Modify: `crates/vol-llm-agents/src/coding/agent.rs`
- Modify: `crates/vol-llm-tui/src/main.rs`

- [ ] **Step 1: Check vol-llm-wiki/src/agent.rs**

Lines 162-169 use `AgentConfig::builder().with_session(session).build()`. The builder returns `AgentConfig`. `ReActAgent::new(config)` wraps in Arc. No change needed.

- [ ] **Step 2: Check vol-llm-yaml-agent/src/builder.rs**

Lines 85-92 use builder pattern. No change needed.

- [ ] **Step 3: Check vol-llm-agents/src/coding/agent.rs**

Lines 198-206 use builder pattern. No change needed.

Lines 212 and 278 access `self.config.session` directly on `CodingAgentConfig` (NOT `AgentConfig`). `CodingAgentConfig` is a separate struct with its own `session: Option<Arc<Session>>` field. This is NOT affected by our changes to `AgentConfig`.

- [ ] **Step 4: Check vol-llm-tui/src/main.rs**

Line 586: `config.session = Some(session);` — this is on `CodingAgentConfig`, not `AgentConfig`. Not affected.

- [ ] **Step 5: Verify all compile**

```bash
cargo check -p vol-llm-wiki -p vol-llm-yaml-agent -p vol-llm-agents -p vol-llm-tui 2>&1 | head -30
```

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-wiki/src/agent.rs crates/vol-llm-yaml-agent/src/builder.rs crates/vol-llm-agents/src/coding/agent.rs crates/vol-llm-tui/src/main.rs
git commit -m "fix: verify external crates compile with new AgentConfig"
```

---

### Task 15: Fix integration tests

**Files:**
- Modify: `crates/vol-llm-agent/tests/react_agent_integration.rs`
- Modify: `crates/vol-llm-agent/tests/agent_run_tests.rs`
- Modify: `crates/vol-llm-agent/tests/session_history_test.rs`
- Modify: `crates/vol-llm-agents/tests/agent_loki_integration.rs`
- Modify: `crates/vol-llm-agents/tests/observer_plugin_unit.rs`

- [ ] **Step 1: Fix react_agent_integration.rs**

Lines 83, 101, 152 — all use `.with_session(session)` on builder. These should work as-is since builder still accepts `Arc<Session>`.

- [ ] **Step 2: Fix agent_run_tests.rs**

Line 410 — `.with_session(session.clone())` on builder. Should work as-is.

- [ ] **Step 3: Fix session_history_test.rs**

Line 74 — `.with_session(session.clone())` on builder. Should work as-is.

- [ ] **Step 4: Fix agent_loki_integration.rs**

Line 113 — `.with_session(session)` on builder. Should work as-is.

- [ ] **Step 5: Fix observer_plugin_unit.rs**

Line 125-129 uses struct literal `AgentConfig { context_builder, plugin_registry, ..Default::default() }`. This still works — Default fills session with `RwLock::new(...)`.

- [ ] **Step 6: Run all agent tests**

```bash
cargo test -p vol-llm-agent 2>&1 | tail -30
```

- [ ] **Step 7: Fix test failures**

If tests fail because they try to clone `AgentConfig` or access `.session` as `Arc<Session>` instead of `RwLock<Arc<Session>>`, fix each:

For `.session` field access in tests that need the inner `Arc<Session>`:
```rust
// Before:
config.session
// After:
config.session.read().unwrap().clone()
```

For struct literals that set session:
```rust
// Before:
AgentConfig { session: my_session, ..Default::default() }
// After:
AgentConfig { session: std::sync::RwLock::new(my_session), ..Default::default() }
```

- [ ] **Step 8: Commit**

```bash
git add crates/vol-llm-agent/tests/ crates/vol-llm-agents/tests/
git commit -m "test: fix integration tests for new AgentConfig types"
```

---

### Task 16: Verify channel tests and full workspace

**Files:**
- All modified files

- [ ] **Step 1: Run channel tests**

```bash
cargo test -p vol-llm-agent-channel 2>&1 | tail -30
```

- [ ] **Step 2: Run full workspace check**

```bash
cargo check --workspace 2>&1 | tail -40
```

Expected: 0 errors.

- [ ] **Step 3: Run full test suite**

```bash
cargo test --workspace 2>&1 | tail -50
```

Expected: all tests pass.

- [ ] **Step 4: Fix any remaining test failures**

Identify each failing test, trace the root cause, fix.

- [ ] **Step 5: Final commit**

```bash
git add -A
git commit -m "test: full workspace verification — all tests pass"
```

---

### Task 17: Remove deprecated AgentBuilder

**Files:**
- Delete: `crates/vol-llm-agent/src/react/builder.rs`

- [ ] **Step 1: Delete the file**

```bash
rm crates/vol-llm-agent/src/react/builder.rs
```

- [ ] **Step 2: Remove from mod.rs if exported**

Check `crates/vol-llm-agent/src/react/mod.rs` — there's no `pub mod builder;` line (confirmed earlier). The file exists but isn't in the module tree. Safe to delete.

- [ ] **Step 3: Verify compiles**

```bash
cargo check --workspace 2>&1 | head -20
```

- [ ] **Step 4: Commit**

```bash
git rm crates/vol-llm-agent/src/react/builder.rs
git commit -m "refactor(agent): remove deprecated AgentBuilder (was broken, unused)"
```

---

## Testing Strategy

- **Per-task**: `cargo check` + `cargo test -p <crate>` after each task
- **Unit focus**: Task 3 (RunningGuard drop), Task 6 (RunContext creation), Task 8 (set_session rejection)
- **Integration**: Task 15 (full agent tests), Task 16 (channel + workspace)
- **Regression**: Existing tests serve as regression suite — any breakage is caught immediately

## Rollback Plan

Each task is an independent commit. If a task introduces issues, revert that commit and re-assess. The task ordering puts the least-risky changes first (new types, no behavior change) and behavior changes later.
