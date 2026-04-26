# Agent & AgentConfig Optimization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove `verbose` and `log_base_path` from AgentConfig/CodingAgentConfig, replace with `working_dir`, remove `context_files` and dead code.

**Architecture:** Replace two independent path/config fields (`log_base_path`, `context_files`) and a debug flag (`verbose`) with a single `working_dir` field. All derived paths (logs, sessions) compute from `working_dir` using the convention `{working_dir}/logs/agents/{agent_id}/`. Agent run logic emits events, not logs.

**Tech Stack:** Rust, cargo, tokio test

---

### Task 1: Remove `verbose` from vol-llm-agent crate

**Files:**
- Modify: `crates/vol-llm-agent/src/react/agent.rs` (lines 25, 62, 589-591, 605-607, 625-627, 732, 746)
- Modify: `crates/vol-llm-agent/src/react/builder.rs` (lines 53-55)
- Modify: `crates/vol-llm-agent/src/react/plugin_stream.rs` (lines 107, 117, 128)
- Modify: `crates/vol-llm-agent/src/react/tests.rs` (line 42)

- [ ] **Step 1: Remove `verbose` field from AgentConfig**

In `crates/vol-llm-agent/src/react/agent.rs`, remove line 25 (`pub verbose: bool,`) and line 62 (`verbose: false,`) from the Default impl.

- [ ] **Step 2: Delete verbose-gated tracing blocks**

In the same file, delete these 3 blocks entirely:

```rust
// Lines ~589-591:
if config.verbose {
    tracing::info!("Interceptor task completed gracefully");
}

// Lines ~605-607:
if config.verbose {
    tracing::info!("Listener task completed gracefully");
}

// Lines ~625-627:
if config.verbose {
    tracing::info!("SessionListener task completed gracefully");
}
```

Just remove the `if config.verbose { ... }` wrapper, leaving the match arms empty (no replacement code — the `Ok(Err)` and `Err(_timeout)` arms already handle errors).

- [ ] **Step 3: Remove `with_verbose()` from builder**

In `crates/vol-llm-agent/src/react/builder.rs`, delete lines 53-55:

```rust
pub fn with_verbose(mut self, verbose: bool) -> Self {
    self.config.verbose = verbose;
    self
}
```

- [ ] **Step 4: Remove `verbose` from AgentConfigSnapshot**

In `crates/vol-llm-agent/src/react/plugin_stream.rs`, remove `verbose` field from:
- `AgentConfigSnapshot` struct (line 107)
- `From` impl body (line 117)
- `Default` impl (line 128)

- [ ] **Step 5: Update tests**

In `crates/vol-llm-agent/src/react/tests.rs`, remove `.with_verbose(true)` from the `test_builder_with_methods` chain (line 42).

In `crates/vol-llm-agent/src/react/agent.rs`, update tests at lines 732 and 746: remove `assert_eq!(config.verbose, false)` and `verbose: true,`.

- [ ] **Step 6: Run tests**

Run: `cargo test -p vol-llm-agent -- --test-threads=1`
Expected: All tests pass

- [ ] **Step 7: Commit**

```bash
git add crates/vol-llm-agent/src/react/agent.rs \
          crates/vol-llm-agent/src/react/builder.rs \
          crates/vol-llm-agent/src/react/plugin_stream.rs \
          crates/vol-llm-agent/src/react/tests.rs
git commit -m "refactor: remove verbose field from vol-llm-agent"
```

---

### Task 2: Remove `verbose` from vol-llm-agents crate (coding, advice, ppt)

**Files:**
- Modify: `crates/vol-llm-agents/src/coding/config.rs` (lines 38, 68, 88)
- Modify: `crates/vol-llm-agents/src/coding/tests.rs` (line 34)
- Modify: `crates/vol-llm-agents/src/advice/service.rs` (line 158)
- Modify: `crates/vol-llm-agents/src/ppt/config.rs` (lines 15, 34-37)
- Modify: `crates/vol-llm-agents/src/ppt/agent.rs` (lines 68-70, 76-79, 90-92, 98-100, 105-107, 116-118)

- [ ] **Step 1: Remove verbose from CodingAgentConfig**

In `crates/vol-llm-agents/src/coding/config.rs`:
- Remove `pub verbose: bool,` (line 38)
- Remove `.field("verbose", &self.verbose)` from Debug impl (line 68)
- Remove `verbose: false,` from Default impl (line 88)

- [ ] **Step 2: Remove verbose from CodingAgentConfig test**

In `crates/vol-llm-agents/src/coding/tests.rs`, remove line 34:
```rust
assert!(!config.verbose);
```

- [ ] **Step 3: Remove .with_verbose() from advice service**

In `crates/vol-llm-agents/src/advice/service.rs`, remove line 158:
```rust
.with_verbose(false)
```

- [ ] **Step 4: Remove verbose from PptAgentConfig**

In `crates/vol-llm-agents/src/ppt/config.rs`:
- Remove `pub verbose: bool,` (line 15)
- Remove `with_verbose()` method (lines 34-37):

```rust
pub fn with_verbose(mut self, verbose: bool) -> Self {
    self.verbose = verbose;
    self
}
```

- [ ] **Step 5: Remove println! blocks from PptAgent**

In `crates/vol-llm-agents/src/ppt/agent.rs`, delete these 6 blocks entirely:

```rust
// Line 68-70:
if self.config.verbose {
    println!("Generating PPT for: {}", description);
}

// Line 76-79:
if self.config.verbose {
    println!("Analyzed requirements: topic={}, audience={:?}, style={:?}",
        requirements.topic, requirements.audience, requirements.style);
}

// Line 90-92:
if self.config.verbose {
    println!("Generated outline with {} slides", outline.slides.len());
}

// Line 98-100:
if self.config.verbose {
    println!("Expanded content for all slides");
}

// Line 105-107:
if self.config.verbose {
    println!("Using template: {} ({})", template.name, template.id);
}

// Line 116-118:
if self.config.verbose {
    println!("Saved PPTX to: {:?}", output_path);
}
```

- [ ] **Step 6: Run tests**

Run: `cargo test -p vol-llm-agents -- --test-threads=1`
Expected: All tests pass

- [ ] **Step 7: Commit**

```bash
git add crates/vol-llm-agents/src/coding/config.rs \
          crates/vol-llm-agents/src/coding/tests.rs \
          crates/vol-llm-agents/src/advice/service.rs \
          crates/vol-llm-agents/src/ppt/config.rs \
          crates/vol-llm-agents/src/ppt/agent.rs
git commit -m "refactor: remove verbose from vol-llm-agents (coding, advice, ppt)"
```

---

### Task 3: Replace `log_base_path` with `working_dir` in AgentConfig

**Files:**
- Modify: `crates/vol-llm-agent/src/react/agent.rs` (replace `log_base_path` → `working_dir`, update run() method)
- Modify: `crates/vol-llm-agent/src/react/builder.rs` (replace `with_log_base_path` → `with_working_dir`)

- [ ] **Step 1: Update AgentConfig struct**

In `crates/vol-llm-agent/src/react/agent.rs`, replace the AgentConfig struct fields. Current:

```rust
pub struct AgentConfig {
    pub max_iterations: u32,
    pub max_history_messages: usize,
    pub context_builder: ContextBuilder,
    pub verbose: bool,          // already removed in Task 1
    pub plugin_registry: PluginRegistry,
    pub agent_id: String,
    pub log_base_path: PathBuf,
    pub unsafe_mode: bool,
    pub approval_handler: Option<super::BoxedApprovalHandler>,
    pub context_files: Vec<String>,
}
```

Replace with:

```rust
pub struct AgentConfig {
    pub max_iterations: u32,
    pub max_history_messages: usize,
    pub context_builder: ContextBuilder,
    pub plugin_registry: PluginRegistry,
    pub agent_id: String,
    pub working_dir: PathBuf,
    pub unsafe_mode: bool,
    pub approval_handler: Option<super::BoxedApprovalHandler>,
}
```

Note: `context_files` is also removed (spec §3).

- [ ] **Step 2: Update Default impl**

Replace:

```rust
Self {
    max_iterations: 5,
    max_history_messages: 20,
    context_builder,
    verbose: false,
    plugin_registry: PluginRegistry::new(),
    agent_id: generate_agent_id(),
    log_base_path: PathBuf::from("logs/agents"),
    unsafe_mode: false,
    approval_handler: None,
    context_files: Vec::new(),
}
```

With:

```rust
Self {
    max_iterations: 5,
    max_history_messages: 20,
    context_builder,
    plugin_registry: PluginRegistry::new(),
    agent_id: generate_agent_id(),
    working_dir: PathBuf::from("."),
    unsafe_mode: false,
    approval_handler: None,
}
```

- [ ] **Step 3: Update run() method — log path derivation**

In `run()`, replace:

```rust
let log_base_path = self.config.log_base_path.clone();
let agent_id = self.config.agent_id.clone();
tokio::spawn(async move {
    let agent_path = log_base_path.join(&agent_id);
    ...
});
```

With:

```rust
let log_base_path = self.config.working_dir.join("logs/agents");
let agent_id = self.config.agent_id.clone();
tokio::spawn(async move {
    let agent_path = log_base_path.join(&agent_id);
    ...
});
```

Also replace the FileSessionEntryStore path at line ~193:

```rust
// Before:
config.log_base_path.join(&config.agent_id),

// After:
config.working_dir.join("logs/agents").join(&config.agent_id),
```

- [ ] **Step 4: Update builder**

In `crates/vol-llm-agent/src/react/builder.rs`, replace `with_log_base_path`:

```rust
// Before:
pub fn with_log_base_path(mut self, path: std::path::PathBuf) -> Self {
    self.config.log_base_path = path;
    self
}
```

With:

```rust
pub fn with_working_dir(mut self, path: std::path::PathBuf) -> Self {
    self.config.working_dir = path;
    self
}
```

Also update `with_observability_plugin()` (line ~93-99) to derive log path:

```rust
// Before:
let plugin = crate::observability::ObservabilityPlugin::new(
    self.config.agent_id.clone(),
    self.config.log_base_path.clone(),
);

// After:
let log_base_path = self.config.working_dir.join("logs/agents");
let plugin = crate::observability::ObservabilityPlugin::new(
    self.config.agent_id.clone(),
    log_base_path,
);
```

- [ ] **Step 5: Update AgentConfig tests**

In `crates/vol-llm-agent/src/react/agent.rs`, update the test at lines ~738-756. Replace:

```rust
let config = AgentConfig {
    max_iterations: 10,
    max_history_messages: 50,
    context_builder,
    verbose: true,
    plugin_registry: PluginRegistry::new(),
    agent_id: "custom_agent".to_string(),
    log_base_path: PathBuf::from("custom/logs"),
    unsafe_mode: false,
    approval_handler: None,
    context_files: Vec::new(),
};
```

With:

```rust
let config = AgentConfig {
    max_iterations: 10,
    max_history_messages: 50,
    context_builder,
    plugin_registry: PluginRegistry::new(),
    agent_id: "custom_agent".to_string(),
    working_dir: PathBuf::from("/custom/project"),
    unsafe_mode: false,
    approval_handler: None,
};
```

And update the assertions at lines 755-756:

```rust
assert_eq!(config.agent_id, "custom_agent");
assert_eq!(config.working_dir, PathBuf::from("/custom/project"));
```

Also update `test_agent_config_with_observability` test — remove `log_base_path` from the struct literal.

- [ ] **Step 6: Run tests**

Run: `cargo test -p vol-llm-agent -- --test-threads=1`
Expected: All tests pass

- [ ] **Step 7: Commit**

```bash
git add crates/vol-llm-agent/src/react/agent.rs \
          crates/vol-llm-agent/src/react/builder.rs
git commit -m "refactor: replace log_base_path with working_dir in AgentConfig"
```

---

### Task 4: Remove `log_base_path` from CodingAgentConfig and update all downstream callers

**Files:**
- Modify: `crates/vol-llm-agents/src/coding/config.rs` (lines 29, 65, 85)
- Modify: `crates/vol-llm-agents/src/coding/agent.rs` (lines 175-179, 193)
- Modify: `crates/vol-llm-agents/src/coding/tests.rs` (lines 32, 439)
- Modify: `crates/vol-llm-agents/tests/observer_plugin_unit.rs` (line 139)
- Modify: `crates/vol-llm-agents/tests/session_recording_test.rs` (line 74)
- Modify: `crates/vol-llm-agents/tests/agent_run_tests.rs` (line 232)
- Modify: `crates/vol-llm-agent/tests/agent_run_tests.rs` (various lines)
- Modify: `crates/vol-llm-agent/tests/session_recording_test.rs` (if exists)
- Modify: `crates/vol-llm-agent/src/react/tests.rs` (line 46)
- Modify: `crates/vol-llm-agent/examples/agent_cli_approval.rs` (line 282)
- Modify: `crates/vol-llm-agent/examples/agent_observability_test.rs` (lines 78, 88, 149)
- Modify: `crates/vol-llm-agents/examples/coding_agent_basic.rs` (lines 40, 43, 69, 97)

- [ ] **Step 1: Remove `log_base_path` from CodingAgentConfig**

In `crates/vol-llm-agents/src/coding/config.rs`:
- Remove `pub log_base_path: PathBuf,` (line 29)
- Remove `.field("log_base_path", &self.log_base_path)` (line 65)
- Remove `log_base_path: PathBuf::from("logs"),` from Default (line 85)

- [ ] **Step 2: Update CodingAgent agent.rs**

In `crates/vol-llm-agents/src/coding/agent.rs`:
- Remove `with_log_base_path()` method (lines 175-180):

```rust
pub fn with_log_base_path(mut self, log_base_path: PathBuf) -> Self {
    self.config.log_base_path = log_base_path;
    if let Some(ref mut state) = self.state {
        state.agent_config.log_base_path = self.config.log_base_path.clone();
    }
    self
}
```

- In `run()` method, replace any `config.log_base_path` usage with `config.working_dir.join("logs/agents")`. Specifically, the `cleanup_old_logs` spawn at lines ~152-157 and `FileSessionEntryStore` at line ~193.

- [ ] **Step 3: Update CodingAgentConfig test**

In `crates/vol-llm-agents/src/coding/tests.rs`:
- Remove `assert_eq!(config.log_base_path, std::path::PathBuf::from("logs"));` (line 32)
- Update `test_agent_with_methods` (line ~439) — replace `.with_log_base_path(tmp_dir.path().join("logs"))` with `.working_dir(tmp_dir.path().to_path_buf())` on the builder, or remove the call entirely since working_dir already has a default.

- [ ] **Step 4: Update downstream test files**

In `crates/vol-llm-agents/tests/observer_plugin_unit.rs`, line ~139:
Remove `log_base_path: std::path::PathBuf::from("logs/test"),` from the AgentConfig struct literal. The working_dir default will suffice for tests.

In `crates/vol-llm-agents/tests/session_recording_test.rs`, line ~74:
Replace `.with_log_base_path(tmp_dir.path().to_path_buf())` with `.with_working_dir(tmp_dir.path().to_path_buf())`.

In `crates/vol-llm-agents/tests/agent_run_tests.rs`, line ~232:
Replace `.with_log_base_path(tmp_dir.path().to_path_buf())` with `.with_working_dir(tmp_dir.path().to_path_buf())`.

In `crates/vol-llm-agent/tests/agent_run_tests.rs`, find and replace all `.with_log_base_path(...)` with `.with_working_dir(...)`.

In `crates/vol-llm-agent/src/react/tests.rs`, line ~46:
Replace `.with_log_base_path(tmp_dir.path().to_path_buf())` with `.with_working_dir(tmp_dir.path().to_path_buf())`.

- [ ] **Step 5: Update examples**

In `crates/vol-llm-agent/examples/agent_cli_approval.rs`, line ~282:
Replace `.with_log_base_path(log_path)` with `.with_working_dir(workdir)` where workdir is the parent directory.

In `crates/vol-llm-agent/examples/agent_observability_test.rs`:
- Line 78: Replace `let log_base_path = PathBuf::from("logs/agents");` with `let working_dir = PathBuf::from(".");`
- Line 88: Replace `.with_log_base_path(log_base_path.clone())` with `.with_working_dir(working_dir.clone())`
- Line 149: Replace `let agent_path = log_base_path.join(&agent_id);` with `let agent_path = working_dir.join("logs/agents").join(&agent_id);`

In `crates/vol-llm-agents/examples/coding_agent_basic.rs`:
- Line 40: Replace `let log_base_path = PathBuf::from("logs/agents");` with `let working_dir = PathBuf::from(".");`
- Line 43: Remove `println!("Log base path: {:?}", log_base_path);`
- Line 69: Replace `log_base_path: log_base_path.clone(),` with `working_dir: working_dir.clone(),`
- Line 97: Replace `let session_log_dir = log_base_path.join(&agent_id);` with `let session_log_dir = working_dir.join("logs/agents").join(&agent_id);`

- [ ] **Step 6: Run full test suite**

Run: `cargo test --workspace -- --test-threads=1`
Expected: All tests pass

- [ ] **Step 7: Commit**

```bash
git add crates/vol-llm-agents/src/coding/config.rs \
          crates/vol-llm-agents/src/coding/agent.rs \
          crates/vol-llm-agents/src/coding/tests.rs \
          crates/vol-llm-agents/tests/ \
          crates/vol-llm-agent/tests/ \
          crates/vol-llm-agent/examples/ \
          crates/vol-llm-agents/examples/
git commit -m "refactor: remove log_base_path from CodingAgentConfig, update all callers"
```

---

### Task 5: Remove dead code

**Files:**
- Modify: `crates/vol-llm-tui/src/approval.rs` (line 43)
- Modify: `crates/vol-llm-agents/src/coding/agent.rs` (line 419)

- [ ] **Step 1: Remove `ApprovalState::is_pending()`**

In `crates/vol-llm-tui/src/approval.rs`, delete lines 43-45:

```rust
pub async fn is_pending(&self) -> bool {
    self.tool_name.lock().await.is_some()
}
```

- [ ] **Step 2: Remove `generate_agent_id()` in vol-llm-agents**

In `crates/vol-llm-agents/src/coding/agent.rs`, delete the function at line ~419:

```rust
fn generate_agent_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    format!("coding_{:x}", timestamp % 0xFFFFFF)
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test --workspace -- --test-threads=1`
Expected: All tests pass, no dead_code warnings for these items

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-tui/src/approval.rs \
          crates/vol-llm-agents/src/coding/agent.rs
git commit -m "refactor: remove dead code (is_pending, generate_agent_id)"
```
