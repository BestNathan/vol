# ToolContext Simplification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Simplify `ToolContext` struct to only contain `messages` field, removing `alert`, `instrument`, and `metadata` fields that are not related to LLM Agent core functionality.

**Architecture:** Modify `ToolContext` in `vol-llm-tool/src/tool.rs` to only contain `messages: Vec<Message>`, remove `vol-core` dependency from `vol-llm-tool`, and update all usages in `vol-llm-bridge` and test files.

**Tech Stack:** Rust, tokio, existing vol-llm-core Message type

---

## File Structure

**Files to Modify:**
- `crates/vol-llm-tool/src/tool.rs` - Simplify ToolContext struct
- `crates/vol-llm-tool/Cargo.toml` - Remove vol-core dependency
- `crates/vol-llm-bridge/src/service.rs` - Update ToolContext creation
- `crates/vol-llm-agent/tests/code_agent_simulation.rs` - Update test context creation
- `crates/vol-llm-agent/tests/react_agent_integration.rs` - Update test context creation

**Files to Verify (no changes needed if using default):**
- `crates/vol-llm-agent/tests/agent_alert_scenario.rs`
- `crates/vol-llm-agent/tests/agent_llm_integration.rs`
- `crates/vol-llm-agent/tests/debug_agent_output.rs`
- `crates/vol-llm-agent/tests/react_mock_test.rs`

---

### Task 1: Simplify ToolContext Struct in vol-llm-tool

**Files:**
- Modify: `crates/vol-llm-tool/src/tool.rs`

- [ ] **Step 1: Read current tool.rs to understand structure**

Already done - current structure:
```rust
#[derive(Debug, Clone, Default)]
pub struct ToolContext {
    pub alert: Option<Alert>,
    pub instrument: String,
    pub messages: Vec<Message>,
    pub metadata: std::collections::HashMap<String, String>,
}
```

- [ ] **Step 2: Replace ToolContext with simplified version**

Replace lines 41-48 in `crates/vol-llm-tool/src/tool.rs`:

```rust
/// Tool execution context
#[derive(Debug, Clone, Default)]
pub struct ToolContext {
    pub messages: Vec<Message>,
}
```

- [ ] **Step 3: Remove Alert import from top of file**

In `crates/vol-llm-tool/src/tool.rs`, remove line 5:
```rust
use vol_core::Alert;
```

The imports should now be:
```rust
//! Tool trait and types.

use async_trait::async_trait;
use vol_llm_core::{ToolDefinition, Message};
use std::error::Error;
```

- [ ] **Step 4: Run cargo check to verify compilation**

Run: `cd crates/vol-llm-tool && cargo check`

Expected: Compilation errors in dependent crates (expected - will fix in subsequent tasks)

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-tool/src/tool.rs
git commit -m "refactor: simplify ToolContext to only contain messages field"
```

---

### Task 2: Remove vol-core Dependency from vol-llm-tool

**Files:**
- Modify: `crates/vol-llm-tool/Cargo.toml`

- [ ] **Step 1: Remove vol-core from Cargo.toml**

In `crates/vol-llm-tool/Cargo.toml`, remove line:
```toml
vol-core = { workspace = true }
```

The file should now be:
```toml
[package]
name = "vol-llm-tool"
version.workspace = true
edition.workspace = true

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true }
async-trait = { workspace = true }
tracing = { workspace = true }
vol-llm-core = { path = "../vol-llm-core" }
```

- [ ] **Step 2: Run cargo check to verify vol-llm-tool compiles**

Run: `cd crates/vol-llm-tool && cargo check`

Expected: Compiles without errors

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-tool/Cargo.toml
git commit -m "chore: remove vol-core dependency from vol-llm-tool"
```

---

### Task 3: Update vol-llm-bridge Service to Use Simplified Context

**Files:**
- Modify: `crates/vol-llm-bridge/src/service.rs`

- [ ] **Step 1: Update ToolContext creation in generate_advice method**

In `crates/vol-llm-bridge/src/service.rs`, replace lines 166-172:

```rust
// Old code
let context = ToolContext {
    alert: Some(alert.clone()),
    instrument: alert.symbol.clone(),
    messages: Vec::new(),
    metadata: std::collections::HashMap::new(),
};

// New code
let context = ToolContext {
    messages: Vec::new(),
};
```

- [ ] **Step 2: Run cargo check on vol-llm-bridge**

Run: `cd crates/vol-llm-bridge && cargo check`

Expected: Compiles without errors

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-bridge/src/service.rs
git commit -m "refactor: update ToolContext creation in AgentAdviceService"
```

---

### Task 4: Update vol-llm-agent Test Files

**Files:**
- Modify: `crates/vol-llm-agent/tests/code_agent_simulation.rs`
- Modify: `crates/vol-llm-agent/tests/react_agent_integration.rs`

- [ ] **Step 1: Update code_agent_simulation.rs line 321-324**

In `crates/vol-llm-agent/tests/code_agent_simulation.rs`, replace:

```rust
// Old code
let context = ToolContext {
    instrument: "eth_usd".to_string(),
    ..Default::default()
};

// New code
let context = ToolContext::default();
```

- [ ] **Step 2: Update react_agent_integration.rs lines 54-57**

In `crates/vol-llm-agent/tests/react_agent_integration.rs`, replace:

```rust
// Old code
let context = ToolContext {
    instrument: "btc_usd".to_string(),
    ..Default::default()
};

// New code
let context = ToolContext::default();
```

- [ ] **Step 3: Check for other instrument field usages in react_agent_integration.rs**

Run: `grep -n "instrument:" crates/vol-llm-agent/tests/react_agent_integration.rs`

If found, update similar patterns. Expected locations:
- Line 115: Same pattern, update to `ToolContext::default()`

- [ ] **Step 4: Run cargo test on vol-llm-agent**

Run: `cd crates/vol-llm-agent && cargo test --test code_agent_simulation --test react_agent_integration -- --nocapture`

Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-agent/tests/code_agent_simulation.rs crates/vol-llm-agent/tests/react_agent_integration.rs
git commit -m "test: update ToolContext creation in agent tests"
```

---

### Task 5: Verify All Tests Pass

**Files:**
- Test: All workspace tests

- [ ] **Step 1: Run full workspace test suite**

Run: `cargo test --workspace 2>&1 | grep -E "(test result|FAILED)"`

Expected output pattern:
```
test result: ok. X passed; 0 failed; Y ignored
```

All crates should show `ok` status.

- [ ] **Step 2: Run vol-monitor build to verify integration**

Run: `cargo build -p vol-monitor --release`

Expected: Compiles without errors (warnings are acceptable)

- [ ] **Step 3: Mark task complete**

All tests passing confirms the refactoring is complete.

---

## Self-Review Checklist

After completing all tasks:

**1. Spec Coverage:**
- [x] ToolContext simplified to only `messages` - Task 1
- [x] vol-core dependency removed - Task 2
- [x] vol-llm-bridge updated - Task 3
- [x] Test files updated - Task 4
- [x] All tests pass - Task 5

**2. No Placeholders:**
- All steps contain actual code snippets
- All commands have expected output
- All file paths are exact

**3. Type Consistency:**
- `ToolContext` uses `Vec<Message>` from `vol_llm_core`
- `ToolContext::default()` creates empty messages vector
- All usages updated consistently

---

Plan complete and saved to `docs/superpowers/plans/2026-04-07-tool-context-simplification.md`. Two execution options:

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

**Which approach?**
