# CodingAgent Config & State Unification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Delete `CodingAgentState`, flatten its fields onto `CodingAgent`, and build `AgentConfig` on-demand per run.

**Architecture:** `new()` consumes `CodingAgentConfig`, resolves LLM, builds tool registry and context builder, stores them as direct fields on `CodingAgent`. `run()` and `resume()` call a private `build_agent_config()` helper to construct a temporary `AgentConfig`. No public API changes.

**Tech Stack:** Rust, cargo, tokio test

---

### Task 1: Delete `CodingAgentState`, flatten fields onto `CodingAgent`

**Files:**
- Modify: `crates/vol-llm-agents/src/coding/agent.rs`
- Modify: `crates/vol-llm-agents/src/coding/tests.rs` (update `test_agent_with_observer` and `test_agent_with_methods` which reference `config().llm`)

- [ ] **Step 1: Replace struct definition**

In `crates/vol-llm-agents/src/coding/agent.rs`, replace lines 25-38:

```rust
// Delete these lines (25-38):
struct CodingAgentState {
    llm: Arc<dyn vol_llm_core::LLMClient>,
    tool_registry: Arc<ToolRegistry>,
    agent_config: AgentConfig,
}

pub struct CodingAgent {
    config: CodingAgentConfig,
    state: Option<CodingAgentState>,
    observer: Option<Arc<dyn EventObserver>>,
    sandbox: Option<vol_llm_core::SandboxRef>,
}
```

With:

```rust
pub struct CodingAgent {
    config: CodingAgentConfig,
    llm: Arc<dyn vol_llm_core::LLMClient>,
    tool_registry: Arc<ToolRegistry>,
    context_builder: vol_llm_context::ContextBuilder,
    observer: Option<Arc<dyn EventObserver>>,
    sandbox: Option<vol_llm_core::SandboxRef>,
}
```

Add import for `ContextBuilder` at the top of the file if not present:

```rust
use vol_llm_context::ContextBuilder;
```

- [ ] **Step 2: Rewrite `new()` method**

Replace the entire `new()` method body (lines 46-115). Change the `Ok(Self { ... })` block from:

```rust
Ok(Self {
    config,
    state: Some(CodingAgentState {
        llm,
        tool_registry: Arc::new(tool_registry),
        agent_config,
    }),
    observer: None,
    sandbox,
})
```

To:

```rust
Ok(Self {
    config,
    llm,
    tool_registry: Arc::new(tool_registry),
    context_builder,
    observer: None,
    sandbox,
})
```

Also remove the `agent_config` variable construction (lines 87-92) — it will move to the new `build_agent_config()` helper in Task 2. But keep the `context_builder` construction (lines 79-85) since it's needed as a stored field.

Specifically, delete these lines from `new()`:

```rust
        let agent_config = AgentConfig {
            max_iterations: config.max_iterations,
            max_history_messages: 20,
            context_builder,
            ..Default::default()
        };
```

And update the `context_builder` variable to be moved into the struct (it's already constructed at line 80-85).

- [ ] **Step 3: Run tests**

Run: `cargo test -p vol-llm-agents -- --test-threads=1`
Expected: All tests pass (some may fail due to removed `state` references — those are fixed in Step 4)

- [ ] **Step 4: Update tests referencing old struct**

In `crates/vol-llm-agents/src/coding/tests.rs`, the test `test_agent_with_observer` (line ~422) accesses `agent.config().llm` which still works since `llm` is on the config. No test should actually reference `.state` — the only structural change is that `config().llm` remains valid (it's on `CodingAgentConfig`).

If the test `test_builder_with_llm` (line ~353) does `assert!(agent.config().llm.is_some())`, this still works — `CodingAgentConfig::llm` is unchanged.

No test file changes needed — the public API (`config()`, `observer()`, etc.) is preserved.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-agents/src/coding/agent.rs
git commit -m "refactor: delete CodingAgentState, flatten fields onto CodingAgent"
```

---

### Task 2: Add `build_agent_config()` helper, simplify `run()` and `resume()`

**Files:**
- Modify: `crates/vol-llm-agents/src/coding/agent.rs` (lines 197-255 for `run()`, lines 258-324 for `resume()`)

- [ ] **Step 1: Add `build_agent_config()` helper**

Add this method to the `impl CodingAgent` block, after `init_context_files()`:

```rust
    /// Build an AgentConfig for a single ReActAgent run.
    fn build_agent_config(&self) -> AgentConfig {
        AgentConfig {
            max_iterations: self.config.max_iterations,
            max_history_messages: 20,
            context_builder: self.context_builder.clone(),
            plugin_registry: self.config.plugin_registry.clone(),
            agent_id: self.config.agent_id.clone(),
            working_dir: self.config.working_dir.clone(),
            unsafe_mode: self.config.unsafe_mode,
            approval_handler: self.config.approval_handler.clone(),
        }
    }
```

- [ ] **Step 2: Simplify `run()` method**

In the `run()` method, replace the `AgentConfig` construction block (lines 214-219):

```rust
// Before:
let agent_config = AgentConfig {
    plugin_registry: self.config.plugin_registry.clone(),
    unsafe_mode: self.config.unsafe_mode,
    approval_handler: self.config.approval_handler.clone(),
    ..state.agent_config.clone()
};
```

With:

```rust
// After:
let agent_config = self.build_agent_config();
```

Also remove the `state` variable extraction since it's no longer needed for `agent_config`. Replace lines 200-201:

```rust
// Before:
let state = self.state.as_ref()
    .ok_or_else(|| CodingAgentError::Config("CodingAgent already consumed".to_string()))?;
```

With no extraction — the `state` variable was only used for `state.agent_config`, `state.llm`, and `state.tool_registry`, which are now direct fields:

```rust
// After: no state extraction needed
```

Update the `ReActAgent::new()` call (lines 221-226) to use `self.llm` and `self.tool_registry` directly:

```rust
// Before:
let mut react_agent = ReActAgent::new(
    state.llm.clone(),
    state.tool_registry.clone(),
    agent_config,
    session,
);

// After:
let mut react_agent = ReActAgent::new(
    self.llm.clone(),
    self.tool_registry.clone(),
    agent_config,
    session,
);
```

- [ ] **Step 3: Simplify `resume()` method**

Apply the same changes to `resume()`:

Remove lines 266-267:
```rust
let state = self.state.as_ref()
    .ok_or_else(|| CodingAgentError::Config("CodingAgent already consumed".to_string()))?;
```

Replace lines 287-292:
```rust
// Before:
let agent_config = AgentConfig {
    plugin_registry: self.config.plugin_registry.clone(),
    unsafe_mode: self.config.unsafe_mode,
    approval_handler: self.config.approval_handler.clone(),
    ..state.agent_config.clone()
};

// After:
let agent_config = self.build_agent_config();
```

Update `ReActAgent::new()` call (lines 294-299):
```rust
// Before:
let mut react_agent = ReActAgent::new(
    state.llm.clone(),
    state.tool_registry.clone(),
    agent_config,
    session,
);

// After:
let mut react_agent = ReActAgent::new(
    self.llm.clone(),
    self.tool_registry.clone(),
    agent_config,
    session,
);
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p vol-llm-agents -- --test-threads=1`
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-agents/src/coding/agent.rs
git commit -m "refactor: add build_agent_config() helper, simplify run/resume"
```

---

### Task 3: Simplify `with_agent_id()` and clean up

**Files:**
- Modify: `crates/vol-llm-agents/src/coding/agent.rs` (lines 164-172)

- [ ] **Step 1: Simplify `with_agent_id()`**

Replace lines 164-172:

```rust
// Before:
pub fn with_agent_id(mut self, agent_id: String) -> Self {
    self.config.agent_id = agent_id;
    // Also update the state's agent_config
    if let Some(ref mut state) = self.state {
        state.agent_config.agent_id = self.config.agent_id.clone();
    }
    self
}
```

With:

```rust
// After:
pub fn with_agent_id(mut self, agent_id: String) -> Self {
    self.config.agent_id = agent_id;
    self
}
```

- [ ] **Step 2: Run full workspace tests**

Run: `cargo test --workspace -- --test-threads=1`
Expected: All tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agents/src/coding/agent.rs
git commit -m "refactor: simplify with_agent_id(), remove state sync"
```
