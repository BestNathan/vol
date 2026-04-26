# TUI Conversation History Persistence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make conversation history persist across multiple `agent.run()` calls within the same TUI REPL loop by creating a shared `FileMessageStore`-backed session at TUI startup.

**Architecture:** TUI creates one `Session` with `FileMessageStore` at startup. The shared session is passed to each `CodingAgent` via `CodingAgentConfig.session`. `CodingAgent::run()` uses `config.session` if provided, otherwise falls back to creating a new `InMemorySession` (backward compatible).

**Tech Stack:** Rust, tokio, vol-session (FileMessageStore, Session), vol-llm-agents (CodingAgent)

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `crates/vol-llm-agents/src/coding/config.rs` | Modify | Add `session` field to `CodingAgentConfig` |
| `crates/vol-llm-agents/src/coding/mod.rs` | Modify | Re-export `Session` from vol-llm-agent |
| `crates/vol-llm-agents/src/coding/agent.rs` | Modify | Use `config.session` in `run()`, fallback to InMemory |
| `crates/vol-llm-tui/src/main.rs` | Modify | Create shared session at startup, pass to config |
| `crates/vol-llm-agents/src/coding/tests.rs` | Modify | Add tests for session field and fallback behavior |

All existing exports (`FileMessageStore`, `InMemorySessionStore`, `Session`) are already public in `vol-session/src/lib.rs` and re-exported from `vol-llm-agent/src/lib.rs`.

---

### Task 1: Add `session` field to CodingAgentConfig + export Session

**Files:**
- Modify: `crates/vol-llm-agents/src/coding/config.rs:1-86` (struct, Default)
- Modify: `crates/vol-llm-agents/src/coding/mod.rs:1-28` (exports)

- [ ] **Step 1: Add session field to struct**

In `crates/vol-llm-agents/src/coding/config.rs`, add after `tool_config` (line 47):

```rust
    /// Shared session for conversation history across runs.
    /// If provided, CodingAgent::run() reuses it instead of creating a new InMemory session.
    pub session: Option<Arc<vol_llm_agent::Session>>,
```

- [ ] **Step 2: Add session to Default impl**

In `Default::default()` (around line 84), add before the closing `}`:

```rust
            session: None,
```

- [ ] **Step 3: Add session to Debug impl**

In `std::fmt::Debug` impl (around line 65), add before `.finish()`:

```rust
            .field("session", &"<Session>")
```

- [ ] **Step 4: Re-export Session from coding module**

In `crates/vol-llm-agents/src/coding/mod.rs`, add after the existing `pub use` lines (around line 21):

```rust
// Re-export Session so TUI can create it
pub use vol_llm_agent::Session;
```

- [ ] **Step 5: Write tests**

In `crates/vol-llm-agents/src/coding/tests.rs`, add after the `test_config_default` test (around line 37):

```rust
#[test]
fn test_config_default_session_is_none() {
    let config = CodingAgentConfig::default();
    assert!(config.session.is_none());
}

#[test]
fn test_config_with_session() {
    use vol_llm_agent::session::{InMemorySessionStore, InMemoryMessageStore};
    let session = Arc::new(vol_llm_agent::Session::new(
        "test-session".to_string(),
        Arc::new(InMemorySessionStore::new()),
        Arc::new(InMemoryMessageStore::new()),
    ));
    let config = CodingAgentConfig {
        session: Some(session.clone()),
        ..Default::default()
    };
    assert!(config.session.is_some());
    assert!(Arc::ptr_eq(config.session.as_ref().unwrap(), &session));
}
```

Add these imports at the top of tests.rs (line 4, after `use crate::coding::*;`):

```rust
use std::sync::Arc;
```

(Arc is already imported at line 4.)

- [ ] **Step 6: Verify compilation**

Run: `cargo check -p vol-llm-agents`
Expected: Compiles successfully

- [ ] **Step 7: Run tests**

Run: `cargo test -p vol-llm-agents --lib -- coding::tests`
Expected: All tests pass including new ones

- [ ] **Step 8: Commit**

```bash
git add crates/vol-llm-agents/src/coding/config.rs crates/vol-llm-agents/src/coding/mod.rs crates/vol-llm-agents/src/coding/tests.rs
git commit -m "feat: add session field to CodingAgentConfig for shared conversation history"
```

---

### Task 2: Update CodingAgent::run() to use config.session

**Files:**
- Modify: `crates/vol-llm-agents/src/coding/agent.rs:187-193` (session creation in run())

- [ ] **Step 1: Replace session creation with conditional logic**

In `crates/vol-llm-agents/src/coding/agent.rs`, replace lines 187-193 (the `use vol_llm_agent::session::...` block through `Session::new(...)`):

**Current code (lines 187-193):**
```rust
        // Create session for this run
        use vol_llm_agent::session::{InMemorySessionStore, InMemoryMessageStore};
        let session = Arc::new(Session::new(
            format!("coding_{}", chrono::Utc::now().format("%Y%m%d_%H%M%S")),
            Arc::new(InMemorySessionStore::new()),
            Arc::new(InMemoryMessageStore::new()),
        ));
```

**New code:**
```rust
        // Create session for this run — use shared session from config if available
        let session = match &self.config.session {
            Some(s) => s.clone(),
            None => {
                use vol_llm_agent::session::{InMemorySessionStore, InMemoryMessageStore};
                Arc::new(Session::new(
                    format!("coding_{}", chrono::Utc::now().format("%Y%m%d_%H%M%S")),
                    Arc::new(InMemorySessionStore::new()),
                    Arc::new(InMemoryMessageStore::new()),
                ))
            }
        };
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p vol-llm-agents`
Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agents/src/coding/agent.rs
git commit -m "feat: CodingAgent::run() uses config.session for conversation history persistence"
```

---

### Task 3: Wire shared session into TUI main.rs

**Files:**
- Modify: `crates/vol-llm-tui/src/main.rs:1-163`

- [ ] **Step 1: Add new imports**

At the top of `main.rs`, after the existing `use` block (around line 16), add:

```rust
use vol_session::FileMessageStore;
```

(`Arc` is already imported at line 13. `vol_llm_agents::coding::Session` is accessible via `vol_llm_agents::coding` re-export from Task 1.)

- [ ] **Step 2: Create shared session before REPL loop**

Before the `loop {` that starts the REPL (around line 73), add:

```rust
    // Create persistent session for this TUI run
    let session: Arc<vol_llm_agents::coding::Session> = {
        let session_dir = std::env::current_dir()
            .unwrap_or_default()
            .join(".vol-sessions");
        if let Err(e) = std::fs::create_dir_all(&session_dir) {
            print_colored(Color::Yellow, &format!("Warning: cannot create session dir: {}\n", e));
            print_colored(Color::Yellow, "Using in-memory session (no history persistence)\n");
            use vol_llm_agent::session::{InMemorySessionStore, InMemoryMessageStore};
            Arc::new(vol_llm_agents::coding::Session::new(
                "tui_memory".to_string(),
                Arc::new(InMemorySessionStore::new()),
                Arc::new(InMemoryMessageStore::new()),
            ))
        } else {
            let session_id = format!("tui_{}", chrono::Utc::now().format("%Y%m%d_%H%M%S"));
            let message_store = Arc::new(FileMessageStore::new(&session_dir, &session_id));
            let session_store = Arc::new(vol_session::InMemorySessionStore::new());
            let session = Arc::new(vol_llm_agents::coding::Session::new(
                session_id.clone(),
                session_store,
                message_store,
            ));
            print_colored(Color::Green, &format!("Session: {}\n", session_id));
            session
        }
    };
```

- [ ] **Step 3: Pass session to CodingAgentConfig in REPL loop**

In the REPL loop's `_ => {` branch (around line 125), find the `let config = CodingAgentConfig { ... }` block and add the session field:

**Current code (around line 125-133):**
```rust
                let config = CodingAgentConfig {
                    max_iterations: 10,
                    working_dir: std::env::current_dir()?,
                    hitl_enabled: true,
                    verbose: false,
                    html_report_path: None,
                    tool_config,
                    ..Default::default()
                };
```

**New code:**
```rust
                let config = CodingAgentConfig {
                    max_iterations: 10,
                    working_dir: std::env::current_dir()?,
                    hitl_enabled: true,
                    verbose: false,
                    html_report_path: None,
                    session: Some(session.clone()),
                    tool_config,
                    ..Default::default()
                };
```

- [ ] **Step 4: Add vol-session dependency to TUI Cargo.toml**

In `crates/vol-llm-tui/Cargo.toml`, add under `[dependencies]`:

```toml
vol-session = { path = "../vol-session" }
```

(Verify it's not already present — the current Cargo.toml may already have it from the TUI redesign PR.)

- [ ] **Step 5: Verify compilation**

Run: `cargo check -p vol-llm-tui`
Expected: Compiles successfully

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-tui/src/main.rs crates/vol-llm-tui/Cargo.toml
git commit -m "feat: TUI creates shared FileMessageStore-backed session for conversation history"
```

---

### Task 4: Full workspace verification

**Files:** No changes — just verification

- [ ] **Step 1: Full workspace check**

Run: `cargo check --workspace`
Expected: All crates compile

- [ ] **Step 2: Run all tests**

Run: `cargo test --workspace --lib`
Expected: All tests pass

- [ ] **Step 3: Commit** (if any test fixes needed)

No changes needed if all passes.

---

## Summary of Changes

| File | Change | Lines Changed |
|------|--------|---------------|
| `crates/vol-llm-agents/src/coding/config.rs` | Add `session` field + Debug + Default | +5 |
| `crates/vol-llm-agents/src/coding/mod.rs` | Re-export `Session` | +2 |
| `crates/vol-llm-agents/src/coding/tests.rs` | Add 2 tests for session field | +20 |
| `crates/vol-llm-agents/src/coding/agent.rs` | Conditional session creation in `run()` | +10 net |
| `crates/vol-llm-tui/src/main.rs` | Create shared session + pass to config | +25 |
| `crates/vol-llm-tui/Cargo.toml` | Add vol-session dependency (if missing) | +1 |

**Key behavioral changes:**
1. TUI creates one `FileMessageStore`-backed session at startup (stored in `.vol-sessions/tui_<timestamp>.jsonl`)
2. Each `agent.run()` call shares the same session → `init_messages()` loads prior Q&As
3. `SessionListener` appends new messages to the JSONL file → history accumulates
4. `max_history_messages` (20) caps how many messages are loaded per run
5. Backward compatible: if `config.session` is `None`, `CodingAgent::run()` creates an ephemeral `InMemorySession` as before
