# vol-llm-ui Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a cross-platform UI crate (`vol-llm-ui`) with shared core logic, ratatui TUI frontend, Dioxus Web WASM frontend, and JSON-RPC WebSocket remote agent service.

**Architecture:** Three-tier separation — shared core library (state/events/connections) + two rendering frontends (ratatui TUI / Dioxus Web WASM) + remote JSON-RPC WebSocket service built on vol-llm-agent-channel.

**Tech Stack:** Rust, Dioxus 0.6 (Web/WASM), ratatui 0.30 (TUI), jsonrpsee 0.26 (JSON-RPC 2.0 over WebSocket), tokio, axum 0.7

---

### Task 1: Create Crate Structure and Workspace Config

**Files:**
- Create: `crates/vol-llm-ui/Cargo.toml`
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Create `crates/vol-llm-ui/Cargo.toml`**

```toml
[package]
name = "vol-llm-ui"
version.workspace = true
edition.workspace = true

[lib]
path = "src/lib.rs"

[[bin]]
name = "vol-llm-tui"
path = "src/tui/bin/tui.rs"
required-features = ["tui"]

[[bin]]
name = "vol-llm-ui-web"
path = "src/web/bin/web.rs"
required-features = ["web"]

[dependencies]
vol-llm-agent = { path = "../vol-llm-agent" }
vol-llm-agent-channel = { path = "../vol-llm-agent-channel" }
vol-llm-core = { path = "../vol-llm-core" }
vol-llm-provider = { path = "../vol-llm-provider" }
vol-llm-tool = { path = "../vol-llm-tool" }
vol-llm-agents = { path = "../vol-llm-agents" }
vol-llm-skill = { path = "../vol-llm-skill" }
vol-llm-observability = { path = "../vol-llm-observability" }
vol-session = { path = "../vol-session" }
tokio = { workspace = true }
async-trait = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tracing = { workspace = true }
anyhow = "1.0"

# TUI (optional, default feature)
ratatui = { version = "0.30", default-features = false, features = ["crossterm_0_29"], optional = true }
crossterm = { version = "0.29", features = ["event-stream"], optional = true }

# Web (optional)
dioxus = { version = "0.6", features = ["web"], optional = true }

# Remote connection (JSON-RPC over WS)
jsonrpsee = { version = "0.26", features = ["client", "wasm-client", "ws-client"] }

[features]
default = ["tui"]
tui = ["dep:ratatui", "dep:crossterm"]
web = ["dep:dioxus"]
```

- [ ] **Step 2: Add vol-llm-ui to workspace members in root `Cargo.toml`**

Add `"crates/vol-llm-ui"` to the `members` array in `/root/nq-deribit/Cargo.toml`, line after `"crates/vol-llm-agent-channel"`.

```toml
# Find this line:
    "crates/vol-llm-agent-channel",
# Add after it:
    "crates/vol-llm-ui",
```

- [ ] **Step 3: Create empty source files**

```bash
mkdir -p crates/vol-llm-ui/src/{state,connection,hooks,tui/bin,web/bin,web/components}
touch crates/vol-llm-ui/src/lib.rs
```

- [ ] **Step 4: Verify workspace resolves**

Run: `cargo metadata -p vol-llm-ui --format-version 1 > /dev/null`
Expected: succeeds, no errors

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/vol-llm-ui/Cargo.toml crates/vol-llm-ui/src/lib.rs
git commit -m "feat: add vol-llm-ui crate with workspace config"
```

---

### Task 2: Shared State Model (`UiState` + `UiEvent`)

**Files:**
- Create: `crates/vol-llm-ui/src/state/mod.rs`
- Modify: `crates/vol-llm-ui/src/lib.rs`

- [ ] **Step 1: Write the test for UiEvent serialization**

```rust
// crates/vol-llm-ui/src/state/mod.rs (test section at bottom)

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ui_event_agent_start_serializes() {
        let event = UiEvent::AgentStart { input: "hello".into() };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""type":"agent_start""#));
        assert!(json.contains(r#""input":"hello""#));
    }

    #[test]
    fn test_ui_event_tool_call_begin_serializes() {
        let event = UiEvent::ToolCallBegin {
            tool_name: "bash".into(),
            arguments: r#"{"cmd":"ls"}"#.into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""type":"tool_call_begin""#));
        assert!(json.contains(r#""tool_name":"bash""#));
    }

    #[test]
    fn test_ui_event_deserializes_from_remote() {
        let json = r#"{"type":"content_complete","content":"The answer is 42."}"#;
        let event: UiEvent = serde_json::from_str(json).unwrap();
        match event {
            UiEvent::ContentComplete { content } => assert_eq!(content, "The answer is 42."),
            _ => panic!("Expected ContentComplete"),
        }
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vol-llm-ui --lib state::tests -- 2>&1 | tail -5`
Expected: FAIL with "cannot find type `UiEvent` in this scope"

- [ ] **Step 3: Implement `UiEvent` and state types**

```rust
// crates/vol-llm-ui/src/state/mod.rs

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::Instant;

// === Unified Event Type ======================================================

/// All agent and UI events flow through this type.
/// Local mode: AgentStreamEvent → UiEvent (via EventBuffer).
/// Remote mode: JSON-RPC notification → UiEvent (deserialized from WS).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UiEvent {
    // Agent lifecycle
    AgentStart { input: String },
    AgentComplete { response: String },
    AgentAborted { reason: String },
    AgentError { message: String },

    // Thinking
    ThinkingStart,
    ThinkingDelta { delta: String },
    ThinkingComplete,

    // Content
    ContentStart,
    ContentDelta { delta: String },
    ContentComplete { content: String },

    // Tools
    ToolCallBegin { tool_name: String, arguments: String },
    ToolCallArgumentDelta { delta: String },
    ToolCallComplete { tool_name: String, result: String, duration_ms: Option<u64> },
    ToolCallError { tool_name: String, error: String, duration_ms: Option<u64> },
    ToolCallSkipped { tool_name: String, reason: String, duration_ms: Option<u64> },

    // Iteration
    MaxIterationsReached { current: u32, max: u32 },
    IterationContinued { from_iteration: u32 },
    IterationComplete { iteration: u32, final_answer: Option<String> },

    // HITL
    ApprovalRequest { tool_name: String, reason: String, arguments: String },
    ApprovalResolved { approved: bool },
}

// === Display Types ===========================================================

#[derive(Debug, Clone)]
pub enum ToolCallStatus { Running, Success, Error, Skipped }

#[derive(Debug, Clone)]
pub struct ToolCallEntry {
    pub sequence: u32,
    pub tool_name: String,
    pub arg_preview: String,
    pub status: ToolCallStatus,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone)]
pub enum ConversationEntry {
    UserInput { text: String },
    Thinking { content: String },
    ContentStreaming { content: String },
    ToolCall { tool_name: String, arg_preview: String },
    ToolResult { tool_name: String, preview: String, success: bool },
    AgentAnswer { text: String },
    RunSummary { iterations: u32, tool_calls: u32, elapsed_ms: u128 },
    Error { message: String },
}

#[derive(Debug, Clone)]
pub struct WorkspaceTree {
    pub root: String,
    pub entries: Vec<WorkspaceEntry>,
}

#[derive(Debug, Clone)]
pub struct WorkspaceEntry {
    pub path: String,
    pub is_dir: bool,
    pub modified: bool,
    pub indent: usize,
}

#[derive(Debug, Clone)]
pub struct SkillDisplayEntry {
    pub name: String,
    pub version: String,
    pub scope: String,
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ActiveTab { Conversation, Workspace, Skills, Logs }

impl ActiveTab {
    pub fn toggle(self) -> Self {
        match self {
            ActiveTab::Conversation => ActiveTab::Workspace,
            ActiveTab::Workspace => ActiveTab::Skills,
            ActiveTab::Skills => ActiveTab::Logs,
            ActiveTab::Logs => ActiveTab::Conversation,
        }
    }
}

pub struct SessionDialogEntry {
    pub session_id: String,
    pub entry_count: usize,
    pub age_label: String,
}

// === ApprovalState (sync, no Arc/Mutex — UI layer handles concurrency) ========

pub struct ApprovalState {
    pub tool_name: Option<String>,
    pub reason: Option<String>,
    pub arguments: Option<String>,
    pub response: Option<(bool, Option<String>)>,
}

impl ApprovalState {
    pub fn new() -> Self {
        Self {
            tool_name: None,
            reason: None,
            arguments: None,
            response: None,
        }
    }

    pub fn has_pending(&self) -> bool {
        self.tool_name.is_some()
    }

    pub fn clear(&mut self) {
        self.tool_name = None;
        self.reason = None;
        self.arguments = None;
        self.response = None;
    }
}

// === UiState =================================================================

pub struct UiState {
    pub session_id: String,
    pub run_count: u32,
    pub iteration: u32,
    pub tool_call_count: u32,
    pub run_start: Option<Instant>,
    pub run_elapsed: std::time::Duration,
    pub is_running: bool,
    pub exiting: bool,
    pub conversation: Vec<ConversationEntry>,
    pub tool_calls: Vec<ToolCallEntry>,
    pub workspace: WorkspaceTree,
    pub modified_files: HashSet<String>,
    pub active_tab: ActiveTab,
    pub conversation_scroll: u16,
    pub workspace_scroll: u16,
    pub tools_scroll: u16,
    pub conversation_auto_scroll: bool,
    pub approval_state: ApprovalState,
    pub session_dialog_open: bool,
    pub session_dialog_sessions: Vec<SessionDialogEntry>,
    pub session_dialog_selected: usize,
    pub log_viewer_selected_run: Option<String>,
    pub log_viewer_entries: Vec<LogLine>,
    pub log_viewer_scroll: u16,
    pub log_viewer_auto_scroll: bool,
    pub log_viewer_run_logs: Vec<LogRunSummary>,
    pub skills: Vec<SkillDisplayEntry>,
    pub unsafe_mode: bool,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LogLine {
    pub event_type: String,
    pub summary: String,
    pub timestamp: String,
}

#[derive(Debug, Clone)]
pub struct LogRunSummary {
    pub run_id: String,
    pub event_count: usize,
    pub last_event: String,
    pub last_event_time: String,
}

impl UiState {
    pub fn new(session_id: String, working_dir: &str) -> Self {
        Self {
            session_id,
            run_count: 0,
            iteration: 0,
            tool_call_count: 0,
            run_start: None,
            run_elapsed: std::time::Duration::ZERO,
            is_running: false,
            exiting: false,
            conversation: Vec::new(),
            tool_calls: Vec::new(),
            workspace: scan_workspace(working_dir),
            modified_files: HashSet::new(),
            active_tab: ActiveTab::Conversation,
            conversation_scroll: 0,
            workspace_scroll: 0,
            tools_scroll: 0,
            conversation_auto_scroll: true,
            approval_state: ApprovalState::new(),
            session_dialog_open: false,
            session_dialog_sessions: Vec::new(),
            session_dialog_selected: 0,
            log_viewer_selected_run: None,
            log_viewer_entries: Vec::new(),
            log_viewer_scroll: 0,
            log_viewer_auto_scroll: true,
            log_viewer_run_logs: Vec::new(),
            skills: Vec::new(),
            unsafe_mode: false,
            last_error: None,
        }
    }

    pub fn reset_for_run(&mut self) {
        self.iteration = 0;
        self.tool_call_count = 0;
        self.run_start = Some(Instant::now());
        self.run_elapsed = std::time::Duration::ZERO;
        self.tool_calls.clear();
        self.modified_files.clear();
        self.tools_scroll = 0;
        self.run_count += 1;
    }

    /// Apply a UiEvent to mutate state.
    pub fn apply(&mut self, event: UiEvent) {
        match event {
            UiEvent::AgentStart { input } => {
                self.reset_for_run();
                self.conversation.push(ConversationEntry::UserInput { text: input });
            }
            UiEvent::AgentComplete { response } => {
                self.flush_pending_content();
                let elapsed = self.run_start.map(|s| s.elapsed()).unwrap_or_default();
                self.run_elapsed = elapsed;
                self.conversation.push(ConversationEntry::RunSummary {
                    iterations: self.iteration,
                    tool_calls: self.tool_call_count,
                    elapsed_ms: elapsed.as_millis(),
                });
                if !response.is_empty() {
                    self.conversation.push(ConversationEntry::AgentAnswer { text: response });
                }
                self.is_running = false;
            }
            UiEvent::AgentAborted { reason } => {
                self.flush_pending_content();
                let elapsed = self.run_start.map(|s| s.elapsed()).unwrap_or_default();
                self.run_elapsed = elapsed;
                self.conversation.push(ConversationEntry::Error { message: reason });
                self.is_running = false;
            }
            UiEvent::AgentError { message } => {
                self.conversation.push(ConversationEntry::Error { message });
                self.is_running = false;
            }
            UiEvent::ThinkingStart => {
                self.conversation.push(ConversationEntry::Thinking { content: String::new() });
            }
            UiEvent::ThinkingDelta { delta } => {
                if let Some(ConversationEntry::Thinking { content }) = self.conversation.last_mut() {
                    content.push_str(&delta);
                }
            }
            UiEvent::ThinkingComplete => {
                // No-op — thinking content already streamed via deltas
            }
            UiEvent::ContentStart => {
                self.conversation.push(ConversationEntry::ContentStreaming { content: String::new() });
            }
            UiEvent::ContentDelta { delta } => {
                if let Some(ConversationEntry::ContentStreaming { content }) = self.conversation.last_mut() {
                    content.push_str(&delta);
                }
            }
            UiEvent::ContentComplete { content } => {
                if let Some(ConversationEntry::ContentStreaming { .. }) = self.conversation.last() {
                    let entry = self.conversation.last_mut().unwrap();
                    *entry = ConversationEntry::AgentAnswer { text: content };
                } else if !content.is_empty() {
                    self.conversation.push(ConversationEntry::AgentAnswer { text: content });
                }
            }
            UiEvent::ToolCallBegin { tool_name, arguments } => {
                let seq = self.tool_call_count + 1;
                self.tool_call_count = seq;
                let preview = extract_arg_preview(&arguments);
                self.tool_calls.push(ToolCallEntry {
                    sequence: seq,
                    tool_name: tool_name.clone(),
                    arg_preview: preview.clone(),
                    status: ToolCallStatus::Running,
                    duration_ms: None,
                });
                self.conversation.push(ConversationEntry::ToolCall {
                    tool_name,
                    arg_preview: preview,
                });
            }
            UiEvent::ToolCallArgumentDelta { delta: _ } => {
                // Invisible in UI
            }
            UiEvent::ToolCallComplete { tool_name, result, duration_ms } => {
                self.update_tool_call_status(&tool_name, ToolCallStatus::Success, duration_ms);
                let preview = truncate_preview(&result, 200);
                self.conversation.push(ConversationEntry::ToolResult {
                    tool_name,
                    preview,
                    success: true,
                });
            }
            UiEvent::ToolCallError { tool_name, error, duration_ms } => {
                self.update_tool_call_status(&tool_name, ToolCallStatus::Error, duration_ms);
                self.conversation.push(ConversationEntry::ToolResult {
                    tool_name,
                    preview: error,
                    success: false,
                });
            }
            UiEvent::ToolCallSkipped { tool_name, reason, duration_ms } => {
                self.update_tool_call_status(&tool_name, ToolCallStatus::Skipped, duration_ms);
                self.conversation.push(ConversationEntry::ToolResult {
                    tool_name,
                    preview: reason,
                    success: false,
                });
            }
            UiEvent::MaxIterationsReached { current, max } => {
                self.conversation.push(ConversationEntry::Error {
                    message: format!("Max iterations reached ({}/{}) — waiting for user decision...", current, max),
                });
            }
            UiEvent::IterationContinued { from_iteration } => {
                self.conversation.push(ConversationEntry::AgentAnswer {
                    text: format!("Continuing from iteration {} (counter reset to 0)", from_iteration),
                });
            }
            UiEvent::IterationComplete { iteration, final_answer } => {
                self.iteration = iteration;
                if let Some(answer) = final_answer {
                    self.conversation.push(ConversationEntry::AgentAnswer { text: answer });
                }
            }
            UiEvent::ApprovalRequest { tool_name, reason, arguments } => {
                self.approval_state.tool_name = Some(tool_name);
                self.approval_state.reason = Some(reason);
                self.approval_state.arguments = Some(arguments);
            }
            UiEvent::ApprovalResolved { approved: _ } => {
                self.approval_state.clear();
            }
        }

        // Auto-scroll
        if self.conversation_auto_scroll {
            self.conversation_scroll = 0;
        }
        self.tools_scroll = self.tool_calls.len() as u16;
    }

    fn flush_pending_content(&mut self) {
        // Convert any streaming entry to answered if still streaming
        if let Some(ConversationEntry::ContentStreaming { content }) = self.conversation.last() {
            let text = content.clone();
            if !text.is_empty() {
                let entry = self.conversation.last_mut().unwrap();
                *entry = ConversationEntry::AgentAnswer { text };
            }
        }
    }

    fn update_tool_call_status(&mut self, tool_name: &str, status: ToolCallStatus, duration_ms: Option<u64>) {
        for entry in self.tool_calls.iter_mut().rev() {
            if entry.tool_name == tool_name && matches!(entry.status, ToolCallStatus::Running) {
                entry.status = status;
                entry.duration_ms = duration_ms;
                break;
            }
        }
    }
}

// === Helpers =================================================================

fn extract_arg_preview(arguments: &str) -> String {
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(arguments) {
        if let Some(cmd) = parsed.get("command").and_then(|v| v.as_str()) {
            if cmd.chars().count() > 80 {
                return format!("Command: {}...", cmd.chars().take(77).collect::<String>());
            }
            return format!("Command: {}", cmd);
        }
        if let Some(path) = parsed.get("path").and_then(|v| v.as_str()) {
            return format!("Path: {}", path);
        }
        if let Some(file_path) = parsed.get("file_path").and_then(|v| v.as_str()) {
            return format!("File: {}", file_path);
        }
        if arguments.chars().count() > 80 {
            return format!("Args: {}...", arguments.chars().take(77).collect::<String>());
        }
        return format!("Args: {}", arguments);
    }
    String::new()
}

fn truncate_preview(s: &str, max_chars: usize) -> String {
    let total_chars = s.chars().count();
    if total_chars <= max_chars {
        return s.to_string();
    }
    let truncated: String = s.chars().take(max_chars).collect();
    format!("{}...", truncated)
}

#[cfg(test)]
mod tests {
    // (Tests from Step 1 go here)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vol-llm-ui --lib state::tests -- 2>&1 | tail -5`
Expected: PASS, 3 tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-ui/src/state/mod.rs crates/vol-llm-ui/src/lib.rs
git commit -m "feat: add UiState and UiEvent types with serialization tests"
```

---

### Task 3: EventBuffer (AgentStreamEvent → UiEvent) and Workspace Scanner

**Files:**
- Create: `crates/vol-llm-ui/src/state/event_buffer.rs`
- Create: `crates/vol-llm-ui/src/state/workspace.rs`
- Modify: `crates/vol-llm-ui/src/state/mod.rs` (add `mod event_buffer`, `mod workspace`, re-exports)
- Modify: `crates/vol-llm-ui/src/lib.rs` (re-export new modules)

- [ ] **Step 1: Write test for EventBuffer conversion**

```rust
// crates/vol-llm-ui/src/state/event_buffer.rs (test section at bottom)

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{UiState, ConversationEntry};

    #[test]
    fn test_agent_start_converts_to_ui_event() {
        let mut buf = EventBuffer::new();
        let mut state = UiState::new("test".into(), ".");
        let event = vol_llm_core::AgentStreamEvent::agent_start("hello".into());
        buf.apply_stream(&event, &mut state);
        assert_eq!(state.conversation.len(), 1);
        match &state.conversation[0] {
            ConversationEntry::UserInput { text } => assert_eq!(text, "hello"),
            _ => panic!("Expected UserInput"),
        }
    }

    #[test]
    fn test_tool_call_events_convert() {
        let mut buf = EventBuffer::new();
        let mut state = UiState::new("test".into(), ".");
        let event = vol_llm_core::AgentStreamEvent::tool_call_begin(
            "call_1".into(), "bash".into(), r#"{"cmd":"ls"}"#.into(),
        );
        buf.apply_stream(&event, &mut state);
        assert_eq!(state.tool_calls.len(), 1);
        assert_eq!(state.tool_calls[0].tool_name, "bash");
        assert_eq!(state.tool_calls[0].status, crate::state::ToolCallStatus::Running);
    }

    #[test]
    fn test_thinking_events_stream_correctly() {
        let mut buf = EventBuffer::new();
        let mut state = UiState::new("test".into(), ".");
        buf.apply_stream(&vol_llm_core::AgentStreamEvent::thinking_start(), &mut state);
        buf.apply_stream(&vol_llm_core::AgentStreamEvent::thinking_delta("thinking...".into()), &mut state);
        buf.apply_stream(&vol_llm_core::AgentStreamEvent::thinking_complete(), &mut state);
        assert_eq!(state.conversation.len(), 1);
        match &state.conversation[0] {
            ConversationEntry::Thinking { content } => assert_eq!(content, "thinking..."),
            _ => panic!("Expected Thinking"),
        }
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vol-llm-ui --lib state::event_buffer::tests -- 2>&1 | tail -5`
Expected: FAIL — `EventBuffer` not found

- [ ] **Step 3: Implement EventBuffer**

```rust
// crates/vol-llm-ui/src/state/event_buffer.rs

use vol_llm_core::AgentStreamEvent;
use crate::state::UiState;

/// Buffers streaming agent events and applies them to UiState.
///
/// Handles the think/content streaming state machine:
/// Start → Delta×N → Complete
pub struct EventBuffer {
    thinking_active: bool,
    content_active: bool,
}

impl EventBuffer {
    pub fn new() -> Self {
        Self {
            thinking_active: false,
            content_active: false,
        }
    }

    /// Convert an AgentStreamEvent and apply to UiState.
    pub fn apply_stream(&mut self, event: &AgentStreamEvent, state: &mut UiState) {
        use AgentStreamEvent as E;
        match event {
            E::AgentStart { input, .. } => {
                state.apply(crate::state::UiEvent::AgentStart { input: input.clone() });
            }
            E::AgentComplete { response, .. } => {
                let response = response
                    .as_ref()
                    .and_then(|v| v.as_str().map(|s| s.to_string()))
                    .unwrap_or_default();
                state.apply(crate::state::UiEvent::AgentComplete { response });
            }
            E::AgentAborted { reason, .. } => {
                state.apply(crate::state::UiEvent::AgentAborted { reason: reason.clone() });
            }
            E::MaxIterationsReached { current_iteration, max_iterations, .. } => {
                state.apply(crate::state::UiEvent::MaxIterationsReached {
                    current: *current_iteration,
                    max: *max_iterations,
                });
            }
            E::IterationContinued { from_iteration, .. } => {
                state.apply(crate::state::UiEvent::IterationContinued { from_iteration: *from_iteration });
            }
            E::ThinkingStart { .. } => {
                state.apply(crate::state::UiEvent::ThinkingStart);
                self.thinking_active = true;
            }
            E::ThinkingDelta { delta, .. } => {
                state.apply(crate::state::UiEvent::ThinkingDelta { delta: delta.clone() });
            }
            E::ThinkingComplete { .. } => {
                self.thinking_active = false;
                state.apply(crate::state::UiEvent::ThinkingComplete);
            }
            E::ContentStart { .. } => {
                state.apply(crate::state::UiEvent::ContentStart);
                self.content_active = true;
            }
            E::ContentDelta { delta, .. } => {
                state.apply(crate::state::UiEvent::ContentDelta { delta: delta.clone() });
            }
            E::ContentComplete { content, .. } => {
                self.content_active = false;
                state.apply(crate::state::UiEvent::ContentComplete { content: content.clone() });
            }
            E::ToolCallBegin { tool_name, arguments, .. } => {
                state.apply(crate::state::UiEvent::ToolCallBegin {
                    tool_name: tool_name.clone(),
                    arguments: arguments.clone(),
                });
            }
            E::ToolCallArgumentDelta { .. } => {
                // Invisible in UI
            }
            E::ToolCallComplete { tool_name, result, duration_ms, .. } => {
                state.apply(crate::state::UiEvent::ToolCallComplete {
                    tool_name: tool_name.clone(),
                    result: result.clone(),
                    duration_ms: *duration_ms,
                });
            }
            E::ToolCallError { tool_name, error, duration_ms, .. } => {
                state.apply(crate::state::UiEvent::ToolCallError {
                    tool_name: tool_name.clone(),
                    error: error.clone(),
                    duration_ms: *duration_ms,
                });
            }
            E::ToolCallSkipped { tool_name, reason, duration_ms, .. } => {
                state.apply(crate::state::UiEvent::ToolCallSkipped {
                    tool_name: tool_name.clone(),
                    reason: reason.clone(),
                    duration_ms: *duration_ms,
                });
            }
            E::IterationComplete { iteration, final_answer, .. } => {
                state.apply(crate::state::UiEvent::IterationComplete {
                    iteration: *iteration,
                    final_answer: final_answer.clone(),
                });
            }
            // LLM calls and plugin events are meta — not displayed
            E::LLMCallStart { .. }
            | E::LLMCallComplete { .. }
            | E::LLMCallError { .. }
            | E::PluginEvent { .. } => {}
        }
    }
}
```

- [ ] **Step 4: Write test for workspace scanner**

```rust
// crates/vol-llm-ui/src/state/workspace.rs (test section at bottom)

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_workspace_returns_entries() {
        let tree = scan_workspace(".");
        // Should at least find the Cargo.toml in root
        assert!(!tree.entries.is_empty() || tree.root == ".");
    }

    #[test]
    fn test_scan_workspace_skips_target_and_git() {
        let tree = scan_workspace(".");
        for entry in &tree.entries {
            assert!(!entry.path.contains("target/"));
            assert!(!entry.path.contains(".git/"));
        }
    }
}
```

- [ ] **Step 5: Implement workspace scanner**

```rust
// crates/vol-llm-ui/src/state/workspace.rs

use crate::state::{WorkspaceEntry, WorkspaceTree};

/// Scan the working directory for files, skipping ignored directories.
pub fn scan_workspace(root: &str) -> WorkspaceTree {
    let skip_dirs = &[".git", "target", "node_modules"];
    let mut entries = Vec::new();

    fn walk(
        dir: &std::path::Path,
        root: &str,
        entries: &mut Vec<WorkspaceEntry>,
        skip_dirs: &[&str],
        indent: usize,
    ) {
        let Ok(read_dir) = std::fs::read_dir(dir) else { return };
        let mut paths: Vec<_> = read_dir
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .collect();
        paths.sort();

        for path in paths {
            let file_name = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            if path.is_dir() {
                if skip_dirs.contains(&file_name) {
                    continue;
                }
                let rel = path.strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();
                entries.push(WorkspaceEntry {
                    path: rel.clone(),
                    is_dir: true,
                    modified: false,
                    indent,
                });
                walk(&path, root, entries, skip_dirs, indent + 1);
            } else {
                let rel = path.strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();
                entries.push(WorkspaceEntry {
                    path: rel,
                    is_dir: false,
                    modified: false,
                    indent,
                });
            }
        }
    }

    let root_path = std::path::Path::new(root);
    if root_path.is_dir() {
        walk(root_path, root, &mut entries, skip_dirs, 0);
    }

    WorkspaceTree {
        root: root.to_string(),
        entries,
    }
}

#[cfg(test)]
mod tests {
    // (Tests from Step 4 go here)
}
```

- [ ] **Step 6: Wire modules in state/mod.rs and lib.rs**

Add to `crates/vol-llm-ui/src/state/mod.rs` top:

```rust
mod event_buffer;
mod workspace;

pub use event_buffer::EventBuffer;
pub use workspace::scan_workspace;
```

Update `crates/vol-llm-ui/src/lib.rs`:

```rust
//! vol-llm-ui: Cross-platform UI for agent interaction.
//!
//! Shared core: UiState, UiEvent, EventBuffer, AgentConnection trait.

pub mod connection;
pub mod state;

pub use state::{UiState, UiEvent, EventBuffer};
```

- [ ] **Step 7: Run all tests**

Run: `cargo test -p vol-llm-ui --lib 2>&1 | tail -10`
Expected: All tests pass (6 total)

- [ ] **Step 8: Commit**

```bash
git add crates/vol-llm-ui/src/state/event_buffer.rs crates/vol-llm-ui/src/state/workspace.rs crates/vol-llm-ui/src/state/mod.rs crates/vol-llm-ui/src/lib.rs
git commit -m "feat: add EventBuffer conversion and workspace scanner"
```

---

### Task 4: Connection Abstraction (`AgentConnection` trait)

**Files:**
- Create: `crates/vol-llm-ui/src/connection/mod.rs`
- Modify: `crates/vol-llm-ui/src/lib.rs`

- [ ] **Step 1: Write test for FileEntry serialization**

```rust
// crates/vol-llm-ui/src/connection/mod.rs (test section at bottom)

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_entry_serializes() {
        let entry = FileEntry {
            name: "src/main.rs".into(),
            is_dir: false,
            size: 1234,
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains(r#""name":"src/main.rs""#));
        assert!(json.contains(r#""is_dir":false"#));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vol-llm-ui --lib connection::tests 2>&1 | tail -5`
Expected: FAIL — `FileEntry` not found

- [ ] **Step 3: Implement AgentConnection trait**

```rust
// crates/vol-llm-ui/src/connection/mod.rs

use crate::state::UiEvent;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogRunInfo {
    pub id: String,
    pub timestamp: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub entry_count: usize,
    pub created_at: i64,
}

/// Unified connection interface for agent interaction.
///
/// Implemented by LocalConnection (in-process) and RemoteConnection (JSON-RPC WS).
#[async_trait]
pub trait AgentConnection: Send + Sync {
    /// Submit user input. Returns a receiver for UiEvents.
    async fn submit(&self, input: String) -> anyhow::Result<mpsc::Receiver<UiEvent>>;

    /// Approve/reject a tool execution.
    async fn approve_tool(&self, approved: bool, reason: Option<String>) -> anyhow::Result<()>;

    /// Cancel the current agent run.
    async fn cancel(&self) -> anyhow::Result<()>;

    /// Whether the connection is active.
    fn is_connected(&self) -> bool;

    /// List files at the given path.
    async fn list_files(&self, path: &str) -> anyhow::Result<Vec<FileEntry>>;

    /// Read file content.
    async fn read_file(&self, path: &str) -> anyhow::Result<String>;

    /// List available log runs.
    async fn list_logs(&self) -> anyhow::Result<Vec<LogRunInfo>>;

    /// Read a specific log run.
    async fn read_log(&self, run_id: &str) -> anyhow::Result<String>;

    /// List saved sessions.
    async fn list_sessions(&self) -> anyhow::Result<Vec<SessionInfo>>;

    /// Resume a saved session.
    async fn resume_session(&self, session_id: &str) -> anyhow::Result<()>;
}

#[cfg(test)]
mod tests {
    // (Tests from Step 1 go here)
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p vol-llm-ui --lib connection::tests 2>&1 | tail -5`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-ui/src/connection/mod.rs crates/vol-llm-ui/src/lib.rs
git commit -m "feat: add AgentConnection trait with file/log/session methods"
```

---

### Task 5: LocalConnection Implementation

**Files:**
- Create: `crates/vol-llm-ui/src/connection/local.rs`
- Modify: `crates/vol-llm-ui/src/connection/mod.rs` (add `mod local;` and re-export)

- [ ] **Step 1: Implement LocalConnection**

```rust
// crates/vol-llm-ui/src/connection/local.rs

use super::{AgentConnection, FileEntry, LogRunInfo, SessionInfo};
use crate::state::UiEvent;
use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use vol_session::{FileSessionEntryStore, Session, SessionEntryStore};

/// Local in-process connection. Runs a ReActAgent directly.
pub struct LocalConnection {
    pub working_dir: PathBuf,
    pub store_dir: PathBuf,
}

impl LocalConnection {
    pub fn new(working_dir: PathBuf, store_dir: PathBuf) -> Self {
        Self { working_dir, store_dir }
    }
}

#[async_trait]
impl AgentConnection for LocalConnection {
    async fn submit(&self, _input: String) -> anyhow::Result<mpsc::Receiver<UiEvent>> {
        // TODO: Create agent, run with observer pattern.
        // This is a placeholder — full implementation in Task 8 (TUI integration).
        let (tx, rx) = mpsc::channel(1);
        Ok(rx)
    }

    async fn approve_tool(&self, _approved: bool, _reason: Option<String>) -> anyhow::Result<()> {
        anyhow::bail!("approve_tool not supported in local connection yet")
    }

    async fn cancel(&self) -> anyhow::Result<()> {
        anyhow::bail!("cancel not supported in local connection yet")
    }

    fn is_connected(&self) -> bool {
        true
    }

    async fn list_files(&self, path: &str) -> anyhow::Result<Vec<FileEntry>> {
        let full_path = self.working_dir.join(path);
        let entries = std::fs::read_dir(&full_path)?;
        let mut result = Vec::new();
        for entry in entries {
            let entry = entry?;
            let metadata = entry.metadata()?;
            result.push(FileEntry {
                name: entry.file_name().to_string_lossy().to_string(),
                is_dir: metadata.is_dir(),
                size: metadata.len(),
            });
        }
        Ok(result)
    }

    async fn read_file(&self, path: &str) -> anyhow::Result<String> {
        let full_path = self.working_dir.join(path);
        Ok(std::fs::read_to_string(&full_path)?)
    }

    async fn list_logs(&self) -> anyhow::Result<Vec<LogRunInfo>> {
        // Placeholder — implemented when log viewer is wired up
        Ok(Vec::new())
    }

    async fn read_log(&self, _run_id: &str) -> anyhow::Result<String> {
        Ok(String::new())
    }

    async fn list_sessions(&self) -> anyhow::Result<Vec<SessionInfo>> {
        let store = FileSessionEntryStore::new(&self.store_dir);
        let summaries = store.list_sessions()?;
        Ok(summaries.into_iter().map(|s| SessionInfo {
            id: s.session_id,
            entry_count: s.entry_count,
            created_at: s.created_at,
        }).collect())
    }

    async fn resume_session(&self, _session_id: &str) -> anyhow::Result<()> {
        // Placeholder — session resume logic goes here
        Ok(())
    }
}
```

- [ ] **Step 2: Wire local module in connection/mod.rs**

Add to `crates/vol-llm-ui/src/connection/mod.rs` top:

```rust
mod local;
pub use local::LocalConnection;
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p vol-llm-ui --lib 2>&1 | tail -5`
Expected: compiles cleanly (warnings OK)

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-ui/src/connection/local.rs crates/vol-llm-ui/src/connection/mod.rs
git commit -m "feat: implement LocalConnection with file/session operations"
```

---

### Task 6: RemoteConnection (JSON-RPC WebSocket)

**Files:**
- Create: `crates/vol-llm-ui/src/connection/remote.rs`
- Modify: `crates/vol-llm-ui/src/connection/mod.rs` (add `mod remote;` and re-export)

- [ ] **Step 1: Implement RemoteConnection**

```rust
// crates/vol-llm-ui/src/connection/remote.rs

use super::{AgentConnection, FileEntry, LogRunInfo, SessionInfo};
use crate::state::UiEvent;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::mpsc;

/// JSON-RPC request sent over WebSocket.
#[derive(Debug, Serialize)]
struct JsonRpcRequest {
    jsonrpc: &'static str,
    method: String,
    params: serde_json::Value,
    id: Option<u64>,
}

/// JSON-RPC response received from server.
#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    result: Option<serde_json::Value>,
    error: Option<serde_json::Value>,
    id: Option<u64>,
}

/// Remote connection via JSON-RPC over WebSocket.
pub struct RemoteConnection {
    url: String,
    // In WASM, we use jsonrpsee's WsClientBuilder which provides async methods.
    // For native, we could use tokio-tungstenite directly.
    #[cfg(not(target_arch = "wasm32"))]
    client: Option<jsonrpsee::ws_client::WsClient>,
    #[cfg(target_arch = "wasm32")]
    _marker: std::marker::PhantomData<()>,
}

impl RemoteConnection {
    pub fn new(url: String) -> Self {
        Self {
            url,
            #[cfg(not(target_arch = "wasm32"))]
            client: None,
            #[cfg(target_arch = "wasm32")]
            _marker: std::marker::PhantomData,
        }
    }

    /// Call a JSON-RPC method and deserialize the result.
    #[cfg(not(target_arch = "wasm32"))]
    async fn call<T: serde::de::DeserializeOwned>(&self, method: &str, params: serde_json::Value) -> anyhow::Result<T> {
        let client = self.client.as_ref()
            .ok_or_else(|| anyhow::anyhow!("not connected"))?;
        let result: T = client.request(method, params).await?;
        Ok(result)
    }
}

#[async_trait]
impl AgentConnection for RemoteConnection {
    async fn submit(&self, input: String) -> anyhow::Result<mpsc::Receiver<UiEvent>> {
        let (tx, rx) = mpsc::channel(256);

        // In native mode, send agent.submit via WS.
        // The server will push ui.event notifications back.
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(client) = &self.client {
                // Subscribe to events via notification stream
                // This is a simplified implementation — full version uses
                // jsonrpsee's SubscriptionClient for proper event streams.
                let _ = client.request::<String>("agent.submit", json!({"input": input})).await;
            }
        }

        Ok(rx)
    }

    async fn approve_tool(&self, approved: bool, reason: Option<String>) -> anyhow::Result<()> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.call::<serde_json::Value>("agent.approve", json!({
                "approved": approved,
                "reason": reason,
            })).await?;
        }
        Ok(())
    }

    async fn cancel(&self) -> anyhow::Result<()> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.call::<serde_json::Value>("agent.cancel", json!({})).await?;
        }
        Ok(())
    }

    fn is_connected(&self) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        return self.client.is_some();
        #[cfg(target_arch = "wasm32")]
        return false; // WASM: connection check deferred
    }

    async fn list_files(&self, path: &str) -> anyhow::Result<Vec<FileEntry>> {
        #[cfg(not(target_arch = "wasm32"))]
        return self.call("file.list", json!({"path": path})).await;
        #[cfg(target_arch = "wasm32")]
        return Ok(Vec::new());
    }

    async fn read_file(&self, path: &str) -> anyhow::Result<String> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let resp: serde_json::Value = self.call("file.read", json!({"path": path})).await?;
            Ok(resp.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string())
        }
        #[cfg(target_arch = "wasm32")]
        return Ok(String::new());
    }

    async fn list_logs(&self) -> anyhow::Result<Vec<LogRunInfo>> {
        #[cfg(not(target_arch = "wasm32"))]
        return self.call("log.list", json!({})).await;
        #[cfg(target_arch = "wasm32")]
        return Ok(Vec::new());
    }

    async fn read_log(&self, run_id: &str) -> anyhow::Result<String> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let resp: serde_json::Value = self.call("log.read", json!({"run_id": run_id})).await?;
            Ok(resp.get("entries").and_then(|v| v.as_str()).unwrap_or("").to_string())
        }
        #[cfg(target_arch = "wasm32")]
        return Ok(String::new());
    }

    async fn list_sessions(&self) -> anyhow::Result<Vec<SessionInfo>> {
        #[cfg(not(target_arch = "wasm32"))]
        return self.call("session.list", json!({})).await;
        #[cfg(target_arch = "wasm32")]
        return Ok(Vec::new());
    }

    async fn resume_session(&self, session_id: &str) -> anyhow::Result<()> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.call::<serde_json::Value>("session.resume", json!({"session_id": session_id})).await?;
        }
        Ok(())
    }
}
```

- [ ] **Step 2: Wire remote module in connection/mod.rs**

Add to `crates/vol-llm-ui/src/connection/mod.rs`:

```rust
mod remote;
pub use remote::RemoteConnection;
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p vol-llm-ui --lib 2>&1 | tail -5`
Expected: compiles cleanly (warnings about unused imports OK)

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-ui/src/connection/remote.rs crates/vol-llm-ui/src/connection/mod.rs
git commit -m "feat: implement RemoteConnection with JSON-RPC WebSocket"
```

---

### Task 7: TUI Frontend (ratatui)

**Files:**
- Create: `crates/vol-llm-ui/src/tui/bin/tui.rs`
- Create: `crates/vol-llm-ui/src/tui/render.rs`
- Create: `crates/vol-llm-ui/src/tui/input.rs`

- [ ] **Step 1: Create render.rs — UI layout and all panel renderers**

This file migrates the 9 render functions from `vol-llm-tui/src/ui/` (conversation.rs, status_bar.rs, tools_panel.rs, input_area.rs, workspace_panel.rs, log_viewer.rs, skills_panel.rs, session_dialog.rs, mod.rs), adapting from `AppState` to `UiState`.

```rust
// crates/vol-llm-ui/src/tui/render.rs
//
// NOTE: This file is ~400 lines. Each render function mirrors the equivalent
// in vol-llm-tui/src/ui/*.rs, adapted for UiState instead of AppState.
// Key differences:
// - state.conversation is UiState.conversation (same ConversationEntry enum)
// - state.tool_calls uses ToolCallEntry from crate::state
// - state.approval_state is UiState.approval_state (sync, no Arc<Mutex>)
// - state.session_dialog is split into 3 fields (open, sessions, selected)
// - state.log_viewer.* is split into individual UiState fields

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};

use crate::state::{UiState, ConversationEntry, ToolCallStatus, ActiveTab};

/// Render the full UI to the frame.
pub fn render_ui(frame: &mut Frame, state: &UiState) {
    let area = frame.area();

    // Status bar: 1 row at top
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .split(area);

    render_status_bar(frame, chunks[0], state);

    // Split remaining area: tools panel (30%) | content panel (70%)
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(70),
        ])
        .split(chunks[1]);

    render_tools_panel(frame, main_chunks[0], state);
    render_right_panel(frame, main_chunks[1], state);
    render_session_dialog(frame, area, state);
}

fn render_right_panel(frame: &mut Frame, area: Rect, state: &UiState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),   // tab bar
            Constraint::Min(3),      // tab content
            Constraint::Length(5),   // input area
        ])
        .split(area);

    render_tab_bar(frame, chunks[0], state);

    match state.active_tab {
        ActiveTab::Conversation => render_conversation(frame, chunks[1], state),
        ActiveTab::Workspace => render_workspace(frame, chunks[1], state),
        ActiveTab::Logs => render_log_viewer(frame, chunks[1], state),
        ActiveTab::Skills => render_skills(frame, chunks[1], state),
    }

    render_input_area(frame, chunks[2], state);
}

// === Status Bar =============================================================

fn render_status_bar(frame: &mut Frame, area: Rect, state: &UiState) {
    let elapsed = if state.is_running {
        state.run_start.map(|s| s.elapsed()).unwrap_or_default()
    } else {
        state.run_elapsed
    };
    let elapsed_secs = elapsed.as_secs();
    let time_str = format!("{:02}:{:02}", elapsed_secs / 60, elapsed_secs % 60);

    let status = if state.is_running { "Running" } else { "Idle" };
    let unsafe_prefix = if state.unsafe_mode { "!! " } else { "" };
    let prefix = if state.exiting { "QUITTING · " } else { "" };

    let text = format!(
        " {}Session: {}{} │ Run: {} │ Iter: {} │ Tools: {} │ Time: {} │ {}",
        unsafe_prefix, prefix, state.session_id, state.run_count,
        state.iteration, state.tool_call_count, time_str, status,
    );

    let paragraph = Paragraph::new(text)
        .style(Style::default().fg(Color::White).bg(Color::DarkGray));
    frame.render_widget(paragraph, area);
}

// === Tab Bar =================================================================

fn render_tab_bar(frame: &mut Frame, area: Rect, state: &UiState) {
    let active = &state.active_tab;

    let style = |tab: ActiveTab| {
        if *tab == *active {
            Style::default().fg(Color::Black).bg(Color::White).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        }
    };

    let tabs = Line::from(vec![
        Span::raw(" "),
        Span::styled(" Conversation ", style(ActiveTab::Conversation)),
        Span::raw(" "),
        Span::styled(" Workspace ", style(ActiveTab::Workspace)),
        Span::raw(" "),
        Span::styled(" Skills ", style(ActiveTab::Skills)),
        Span::raw(" "),
        Span::styled(" Logs ", style(ActiveTab::Logs)),
        Span::raw(" "),
    ]);

    let paragraph = Paragraph::new(tabs)
        .block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(paragraph, area);
}

// === Conversation ============================================================

fn render_conversation(frame: &mut Frame, area: Rect, state: &UiState) {
    let block = Block::default().borders(Borders::ALL).title(" Conversation ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.conversation.is_empty() {
        let empty = Paragraph::new("No messages yet. Type a query and press Enter.")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(empty, inner);
        return;
    }

    let lines = build_conversation_lines(state, inner.width as usize);
    let total = lines.len();
    let visible = inner.height as usize;
    let scroll = if state.conversation_auto_scroll {
        total.saturating_sub(visible)
    } else {
        (state.conversation_scroll as usize).min(total.saturating_sub(1))
    };

    let visible_lines: Vec<Line> = lines.into_iter().skip(scroll).take(visible).collect();
    let paragraph = Paragraph::new(Text::from(visible_lines));
    frame.render_widget(paragraph, inner);
}

fn wrap_line(text: &str, max_chars: usize) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max_chars || max_chars == 0 {
        return vec![text.to_string()];
    }
    let mut lines = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        let end = start + max_chars;
        if end >= chars.len() {
            lines.push(chars[start..].iter().collect());
            break;
        }
        let mut split = end;
        for i in (start..end).rev() {
            if chars[i] == ' ' { split = i; break; }
        }
        if split == start || chars[start..end].iter().all(|&c| c != ' ') {
            lines.push(chars[start..end].iter().collect());
            start = end;
        } else {
            lines.push(chars[start..split].iter().collect());
            start = split + 1;
        }
    }
    lines
}

fn build_conversation_lines(state: &UiState, max_width: usize) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for entry in &state.conversation {
        match entry {
            ConversationEntry::UserInput { text } => {
                let wrap = max_width.saturating_sub(4);
                lines.push(Line::from(vec![
                    Span::styled(">>> ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                ]));
                for line in text.lines() {
                    for w in wrap_line(line, wrap) {
                        lines.push(Line::from(vec![Span::styled(w, Style::default().fg(Color::White))]));
                    }
                }
                lines.push(Line::raw(""));
            }
            ConversationEntry::Thinking { content } => {
                lines.push(Line::from(vec![
                    Span::styled("Thinking", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                ]));
                let wrap = max_width.saturating_sub(2);
                for line in content.lines() {
                    for w in wrap_line(line, wrap) {
                        lines.push(Line::from(vec![
                            Span::styled(format!("  {}", w), Style::default().fg(Color::DarkGray)),
                        ]));
                    }
                }
                lines.push(Line::raw(""));
            }
            ConversationEntry::ContentStreaming { content } => {
                if content.is_empty() {
                    lines.push(Line::from(vec![
                        Span::styled("Generating...", Style::default().fg(Color::DarkGray)),
                    ]));
                } else {
                    for line in content.lines() {
                        for w in wrap_line(line, max_width) {
                            lines.push(Line::from(vec![Span::styled(w, Style::default().fg(Color::White))]));
                        }
                    }
                }
            }
            ConversationEntry::ToolCall { tool_name, arg_preview } => {
                lines.push(Line::from(vec![
                    Span::styled(format!("[{}]", tool_name), Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
                ]));
                if !arg_preview.is_empty() {
                    for w in wrap_line(arg_preview, max_width.saturating_sub(2)) {
                        lines.push(Line::from(vec![
                            Span::styled(format!("  {}", w), Style::default().fg(Color::DarkGray)),
                        ]));
                    }
                }
            }
            ConversationEntry::ToolResult { tool_name, preview, success } => {
                let status = if *success { "OK" } else { "ERR" };
                let color = if *success { Color::Green } else { Color::Red };
                lines.push(Line::from(vec![
                    Span::styled(format!("  {} {} ", status, tool_name), Style::default().fg(color)),
                ]));
                let wrap = max_width.saturating_sub(4);
                for line in preview.lines().take(6) {
                    for w in wrap_line(line, wrap) {
                        lines.push(Line::from(vec![
                            Span::styled(format!("    {}", w), Style::default().fg(Color::DarkGray)),
                        ]));
                    }
                }
                lines.push(Line::raw(""));
            }
            ConversationEntry::AgentAnswer { text } => {
                lines.push(Line::raw(""));
                for line in text.lines() {
                    for w in wrap_line(line, max_width) {
                        lines.push(Line::from(vec![Span::styled(w, Style::default().fg(Color::White))]));
                    }
                }
                lines.push(Line::raw(""));
            }
            ConversationEntry::RunSummary { iterations, tool_calls, elapsed_ms } => {
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("Done · {} iteration{} · {} tool call{} · {}ms",
                            iterations, if *iterations == 1 { "" } else { "s" },
                            tool_calls, if *tool_calls == 1 { "" } else { "s" },
                            elapsed_ms),
                        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                    ),
                ]));
            }
            ConversationEntry::Error { message } => {
                lines.push(Line::from(vec![
                    Span::styled(format!("Error: {}", message),
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                ]));
            }
        }
    }
    lines
}

// === Tools Panel ============================================================

fn render_tools_panel(frame: &mut Frame, area: Rect, state: &UiState) {
    let title = format!(" Tools Called ({}) ", state.tool_calls.len());
    let block = Block::default().borders(Borders::ALL).title(title)
        .style(Style::default().fg(Color::Blue));

    if state.tool_calls.is_empty() {
        let empty = Paragraph::new("No tool calls yet")
            .style(Style::default().fg(Color::DarkGray)).block(block.clone());
        frame.render_widget(empty, area);
        return;
    }

    let items: Vec<ListItem> = state.tool_calls.iter().map(|entry| {
        let (status_str, status_color) = match entry.status {
            ToolCallStatus::Running => ("…", Color::Yellow),
            ToolCallStatus::Success => ("✓", Color::Green),
            ToolCallStatus::Error => ("ERR", Color::Red),
            ToolCallStatus::Skipped => ("SKIP", Color::DarkGray),
        };
        ListItem::new(vec![
            Line::from(vec![
                Span::styled(
                    format!("{}. [{}]  {}", entry.sequence, entry.tool_name, status_str),
                    Style::default().fg(status_color).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled(&entry.arg_preview, Style::default().fg(Color::DarkGray)),
            ]),
        ])
    }).collect();

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

// === Input Area + Approval ==================================================

fn render_input_area(frame: &mut Frame, area: Rect, state: &UiState) {
    if area.height < 3 { return; }

    if state.approval_state.has_pending() {
        render_approval_panel(frame, area, state);
    } else {
        render_textarea_hints(frame, area, state);
    }
}

fn render_textarea_hints(frame: &mut Frame, area: Rect, state: &UiState) {
    let hint_area = Rect { x: area.x, y: area.y + area.height - 1, width: area.width, height: 1 };
    let text_area = Rect { x: area.x, y: area.y, width: area.width, height: area.height - 1 };

    let block = Block::default().borders(Borders::ALL).title(" Input ");
    let inner = block.inner(text_area);
    frame.render_widget(block, text_area);

    // Placeholder: "Type here" since we don't have ratatui-textarea in vol-llm-ui yet
    // The TUI bin will use ratatui-textarea for actual input
    let placeholder = Paragraph::new("Type here (input handled by TUI event loop)")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(placeholder, inner);

    let hint = if state.is_running {
        Line::from(vec![Span::styled(" Running... (input disabled) ", Style::default().fg(Color::Yellow))])
    } else {
        Line::from(vec![
            Span::styled(" Enter ", Style::default().fg(Color::Blue)),
            Span::styled("Send  ", Style::default().fg(Color::DarkGray)),
            Span::styled(" Esc ", Style::default().fg(Color::Blue)),
            Span::styled("Clear", Style::default().fg(Color::DarkGray)),
        ])
    };
    frame.render_widget(Paragraph::new(hint), hint_area);
}

fn render_approval_panel(frame: &mut Frame, area: Rect, state: &UiState) {
    let tool_name = state.approval_state.tool_name.as_deref().unwrap_or("unknown");
    let arguments = state.approval_state.arguments.as_deref().unwrap_or("");

    let block = Block::default().borders(Borders::ALL).title(" Approval ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let text = Text::from(vec![
        Line::from(vec![
            Span::styled(format!(" ⚠ {}", tool_name),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled(format!("  {}", arguments.chars().take(100).collect::<String>()),
                Style::default().fg(Color::DarkGray)),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled(" [A] ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled("Approve  ", Style::default().fg(Color::DarkGray)),
            Span::styled(" [R] ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::styled("Reject  ", Style::default().fg(Color::DarkGray)),
            Span::styled(" [S] ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::styled("Stop", Style::default().fg(Color::DarkGray)),
        ]),
    ]);

    frame.render_widget(Paragraph::new(text), inner);
}

// === Workspace Panel ========================================================

fn render_workspace(frame: &mut Frame, area: Rect, state: &UiState) {
    let block = Block::default().borders(Borders::ALL).title(" Workspace ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.workspace.entries.is_empty() {
        let empty = Paragraph::new("Workspace directory empty or unavailable")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(empty, inner);
        return;
    }

    let lines: Vec<Line> = state.workspace.entries.iter().map(|entry| {
        let indent = "  ".repeat(entry.indent);
        let name = entry.path.split('/').last().unwrap_or(&entry.path);
        let prefix = if entry.is_dir {
            format!("{}[DIR] {}", indent, name)
        } else {
            let modified = if entry.modified { " M" } else { "" };
            format!("{}[FILE] {}{}", indent, name, modified)
        };
        let style = if entry.is_dir {
            Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)
        } else if entry.modified {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::White)
        };
        Line::from(vec![Span::styled(prefix, style)])
    }).collect();

    let paragraph = Paragraph::new(Text::from(lines)).scroll((state.workspace_scroll, 0));
    frame.render_widget(paragraph, inner);
}

// === Log Viewer =============================================================

fn render_log_viewer(frame: &mut Frame, area: Rect, state: &UiState) {
    if state.log_viewer_selected_run.is_some() {
        render_log_entries(frame, area, state);
    } else {
        render_run_list(frame, area, state);
    }
}

fn render_run_list(frame: &mut Frame, area: Rect, state: &UiState) {
    let mut lines = Vec::new();
    if state.log_viewer_run_logs.is_empty() {
        lines.push(Line::from(Span::styled(" No log files found.", Style::default().fg(Color::DarkGray))));
    } else {
        for run in &state.log_viewer_run_logs {
            let short_id = if run.run_id.len() > 12 { &run.run_id[..12] } else { &run.run_id };
            lines.push(Line::from(vec![
                Span::styled(format!(" {:<14}", short_id), Style::default().fg(Color::Gray)),
                Span::styled(format!(" {:>5} events", run.event_count), Style::default().fg(Color::DarkGray)),
                Span::styled(format!("  {}", run.last_event), Style::default().fg(Color::DarkGray)),
                Span::styled(format!(" ({})", run.last_event_time), Style::default().fg(Color::DarkGray)),
            ]));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(" Enter to view  Esc to go back", Style::default().fg(Color::DarkGray))));

    let paragraph = Paragraph::new(lines)
        .block(Block::default().title(" Log Runs ").borders(Borders::ALL));
    frame.render_widget(paragraph, area);
}

fn render_log_entries(frame: &mut Frame, area: Rect, state: &UiState) {
    let lines: Vec<Line> = state.log_viewer_entries.iter().map(|entry| {
        let color = match entry.event_type.as_str() {
            "AgentStart" | "AgentComplete" => Color::Green,
            "ToolCallBegin" | "ToolCallComplete" => Color::Yellow,
            "ToolCallError" | "AgentAborted" => Color::Red,
            _ => Color::White,
        };
        Line::from(vec![
            Span::styled(format!("[{}] ", entry.timestamp), Style::default().fg(Color::DarkGray)),
            Span::styled(entry.event_type.clone(), Style::default().fg(color)),
            Span::styled(format!(" — {}", entry.summary), Style::default().fg(color)),
        ])
    }).collect();

    let run_id = state.log_viewer_selected_run.as_deref().unwrap_or("");
    let paragraph = Paragraph::new(lines)
        .block(Block::default().title(format!(" Log: {} ", run_id)).borders(Borders::ALL))
        .scroll((state.log_viewer_scroll, 0));
    frame.render_widget(paragraph, area);
}

// === Skills Panel ===========================================================

fn render_skills(frame: &mut Frame, area: Rect, state: &UiState) {
    let block = Block::default().borders(Borders::ALL).title(format!(" Skills ({}) ", state.skills.len()));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.skills.is_empty() {
        let empty = Paragraph::new("No skills discovered")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(empty, inner);
        return;
    }

    let max_width = inner.width as usize;
    let name_w = 22.min(max_width.saturating_sub(20));
    let version_w = 8.min(max_width.saturating_sub(name_w + 12));
    let scope_w = 10.min(max_width.saturating_sub(name_w + version_w + 4));
    let desc_w = max_width.saturating_sub(name_w + version_w + scope_w + 4);

    let lines: Vec<Line> = state.skills.iter().map(|s| {
        let scope_color = match s.scope.as_str() {
            "User" => Color::Green,
            "Repo" => Color::Blue,
            _ => Color::Yellow,
        };
        Line::from(vec![
            Span::styled(pad_or_truncate(&s.name, name_w), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::raw(" | "),
            Span::styled(pad_or_truncate(&s.version, version_w), Style::default().fg(Color::DarkGray)),
            Span::raw(" | "),
            Span::styled(pad_or_truncate(&s.scope, scope_w), Style::default().fg(scope_color)),
            Span::raw(" | "),
            Span::styled(pad_or_truncate(&s.description, desc_w), Style::default().fg(Color::DarkGray)),
        ])
    }).collect();

    frame.render_widget(Paragraph::new(Text::from(lines)), inner);
}

fn pad_or_truncate(s: &str, width: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= width {
        format!("{}{}", s, " ".repeat(width.saturating_sub(char_count)))
    } else {
        format!("{}…", s.chars().take(width.saturating_sub(1)).collect::<String>())
    }
}

// === Session Dialog =========================================================

fn render_session_dialog(frame: &mut Frame, area: Rect, state: &UiState) {
    if !state.session_dialog_open { return; }

    let width = 60.min(area.width);
    let height = (state.session_dialog_sessions.len() as u16 + 6).min(area.height - 2);
    let x = (area.width - width) / 2;
    let y = (area.height - height) / 2;
    let rect = Rect::new(x, y, width, height);

    frame.render_widget(ratatui::widgets::Clear, rect);

    let mut lines = Vec::new();
    if state.session_dialog_sessions.is_empty() {
        lines.push(Line::from(Span::styled("  No saved sessions found.", Style::default().fg(Color::DarkGray))));
    } else {
        for (i, entry) in state.session_dialog_sessions.iter().enumerate() {
            let is_selected = i == state.session_dialog_selected;
            let prefix = if is_selected { "> " } else { "  " };
            let short_id = if entry.session_id.len() > 8 { &entry.session_id[..8] } else { &entry.session_id };
            let style = if is_selected {
                Style::default().fg(Color::White).bg(Color::DarkGray).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            lines.push(Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(format!("{:<10}", short_id), style),
                Span::styled(format!(" {:>4} entries", entry.entry_count), Style::default().fg(Color::DarkGray)),
                Span::styled(format!("    {}", entry.age_label), Style::default().fg(Color::DarkGray)),
            ]));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " [n] New  [Enter] Resume  [d] Delete  [Esc] Cancel",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines)
        .block(Block::default()
            .title(" Sessions (Ctrl+S to dismiss) ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow)));
    frame.render_widget(paragraph, rect);
}
```

- [ ] **Step 2: Create input.rs — keyboard handling**

```rust
// crates/vol-llm-ui/src/tui/input.rs

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crate::state::{UiState, ActiveTab};

/// Result of processing a key event.
pub enum InputAction {
    /// Exit the application.
    Exit,
    /// Send the input text to the agent.
    Send(String),
    /// Resume a saved session.
    ResumeSession(String),
    /// No action (key consumed for navigation).
    None,
}

/// Process a key event and return the resulting action.
pub fn handle_key(key: KeyEvent, state: &mut UiState, input_text: &str) -> InputAction {
    // Approval response keys — highest priority
    if state.approval_state.has_pending() {
        match key.code {
            KeyCode::Char('a') | KeyCode::Char('A') => {
                state.approval_state.clear();
                return InputAction::None; // Will be handled via connection
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                state.approval_state.clear();
                return InputAction::None;
            }
            KeyCode::Char('s') | KeyCode::Char('S') => {
                state.approval_state.clear();
                return InputAction::None;
            }
            _ => {}
        }
    }

    // Session dialog
    if state.session_dialog_open {
        return handle_session_dialog_key(key, state);
    }

    match (key.modifiers, key.code) {
        (KeyModifiers::ALT, KeyCode::Enter) => InputAction::None, // Insert newline

        (_, KeyCode::Enter) => {
            if state.is_running { return InputAction::None; }
            let input = input_text.trim().to_string();
            if input.is_empty() { return InputAction::None; }
            InputAction::Send(input)
        }

        (_, KeyCode::Esc) => InputAction::None, // Clear input

        (_, KeyCode::Tab) => {
            state.active_tab = state.active_tab.toggle();
            InputAction::None
        }

        (_, KeyCode::PageUp) => {
            state.conversation_scroll = state.conversation_scroll.saturating_sub(10);
            state.conversation_auto_scroll = false;
            InputAction::None
        }
        (_, KeyCode::PageDown) => {
            state.conversation_scroll = state.conversation_scroll.saturating_add(10);
            state.conversation_auto_scroll = false;
            InputAction::None
        }
        (_, KeyCode::Up) => {
            state.conversation_scroll = state.conversation_scroll.saturating_sub(1);
            state.conversation_auto_scroll = false;
            InputAction::None
        }
        (_, KeyCode::Down) => {
            state.conversation_scroll = state.conversation_scroll.saturating_add(1);
            state.conversation_auto_scroll = false;
            InputAction::None
        }

        (KeyModifiers::CONTROL, KeyCode::Char('1')) => {
            state.active_tab = ActiveTab::Conversation;
            InputAction::None
        }
        (KeyModifiers::CONTROL, KeyCode::Char('2')) => {
            state.active_tab = ActiveTab::Workspace;
            InputAction::None
        }
        (KeyModifiers::CONTROL, KeyCode::Char('3')) => {
            state.active_tab = ActiveTab::Logs;
            InputAction::None
        }
        (KeyModifiers::CONTROL, KeyCode::Char('4')) => {
            state.active_tab = ActiveTab::Skills;
            InputAction::None
        }

        (KeyModifiers::CONTROL, KeyCode::Char('s')) => {
            if !state.is_running {
                state.session_dialog_open = !state.session_dialog_open;
            }
            InputAction::None
        }

        (_, KeyCode::Char('q')) if key.modifiers == KeyModifiers::CONTROL => {
            if !state.is_running { state.exiting = true; }
            InputAction::Exit
        }

        (_, KeyCode::Char('u')) if key.modifiers == KeyModifiers::CONTROL => {
            state.unsafe_mode = !state.unsafe_mode;
            state.conversation.push(crate::state::ConversationEntry::AgentAnswer {
                text: if state.unsafe_mode {
                    "Unsafe mode enabled — all tool approvals auto-approved".to_string()
                } else {
                    "Unsafe mode disabled".to_string()
                },
            });
            InputAction::None
        }

        _ => InputAction::None,
    }
}

fn handle_session_dialog_key(key: KeyEvent, state: &mut UiState) -> InputAction {
    match key.code {
        KeyCode::Esc => {
            state.session_dialog_open = false;
            InputAction::None
        }
        KeyCode::Enter => {
            if let Some(entry) = state.session_dialog_sessions.get(state.session_dialog_selected) {
                let id = entry.session_id.clone();
                state.session_dialog_open = false;
                return InputAction::ResumeSession(id);
            }
            InputAction::None
        }
        KeyCode::Up => {
            if state.session_dialog_selected > 0 {
                state.session_dialog_selected -= 1;
            }
            InputAction::None
        }
        KeyCode::Down => {
            if state.session_dialog_selected + 1 < state.session_dialog_sessions.len() {
                state.session_dialog_selected += 1;
            }
            InputAction::None
        }
        KeyCode::Char('n') => {
            state.session_dialog_open = false;
            state.session_id = uuid::Uuid::new_v4().to_string();
            InputAction::None
        }
        KeyCode::Char('d') => {
            // Delete session — placeholder, needs session store access
            if let Some(entry) = state.session_dialog_sessions.get(state.session_dialog_selected) {
                if entry.session_id != state.session_id {
                    state.session_dialog_sessions.remove(state.session_dialog_selected);
                    state.session_dialog_selected = 0.min(state.session_dialog_sessions.len().saturating_sub(1));
                }
            }
            InputAction::None
        }
        _ => InputAction::None,
    }
}
```

- [ ] **Step 3: Create tui.rs — TUI binary entry point**

```rust
// crates/vol-llm-ui/src/tui/bin/tui.rs

//! vol-llm-tui: Terminal UI for agent interaction using ratatui.

use std::io::{self, stdout};
use std::sync::Arc;
use std::time::Duration;

use vol_llm_ui::state::{UiState, EventBuffer};
use vol_llm_ui::connection::{AgentConnection, LocalConnection};

mod render;
mod input;

use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    event::{Event, EventStream, KeyCode, KeyEvent},
};
use futures::StreamExt;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use input::{handle_key, InputAction};
use render::render_ui;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Verify API key
    let _api_key = std::env::var("ANTHROPIC_AUTH_TOKEN")
        .expect("ANTHROPIC_AUTH_TOKEN must be set");

    // Parse args
    let working_dir = std::env::current_dir().unwrap_or_default();
    let home = std::env::var("HOME").unwrap_or_default();
    let project = working_dir.file_name().unwrap_or(std::ffi::OsStr::new("default")).to_string_lossy();
    let store_dir = std::path::PathBuf::from(home).join(".vol-coding").join(project.as_ref()).join("sessions");

    // Create state
    let session_id = uuid::Uuid::new_v4().to_string();
    let mut state = UiState::new(session_id, working_dir.to_string_lossy().as_ref());

    // Setup terminal
    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic| {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen);
        original_hook(panic);
    }));

    // Create connection
    let connection: Arc<dyn AgentConnection> = Arc::new(LocalConnection::new(working_dir, store_dir));

    // Setup terminal
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    // Event buffer for converting agent events
    let mut event_buffer = EventBuffer::new();

    // Main loop
    let mut render_interval = tokio::time::interval(Duration::from_millis(33));
    let mut events = EventStream::new();
    let mut input_buf = String::new();

    loop {
        tokio::select! {
            biased;

            // Input
            maybe_event = events.next() => {
                match maybe_event {
                    Some(Ok(Event::Key(key))) => {
                        let action = handle_key(key, &mut state, &input_buf);
                        match action {
                            InputAction::Exit => break,
                            InputAction::Send(text) => {
                                input_buf.clear();
                                state.is_running = true;
                                // TODO: Submit to connection
                                let conn = connection.clone();
                                let text = text.clone();
                                tokio::spawn(async move {
                                    // Placeholder: full agent run goes here
                                    let _ = conn.submit(text).await;
                                });
                            }
                            InputAction::ResumeSession(_id) => {
                                // TODO: Resume session via connection
                            }
                            InputAction::None => {}
                        }
                    }
                    Some(Ok(Event::Resize(_, _))) => {}
                    _ => {}
                }
            }

            // Render
            _ = render_interval.tick() => {
                terminal.draw(|f| render_ui(f, &state))?;
            }
        }
    }

    // Cleanup
    execute!(stdout(), LeaveAlternateScreen)?;
    disable_raw_mode()?;

    Ok(())
}
```

- [ ] **Step 4: Verify TUI compiles**

Run: `cargo check -p vol-llm-ui --bin vol-llm-tui --features tui 2>&1 | tail -10`
Expected: compiles cleanly (may have warnings about unused imports)

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-ui/src/tui/bin/tui.rs crates/vol-llm-ui/src/tui/render.rs crates/vol-llm-ui/src/tui/input.rs
git commit -m "feat: add TUI frontend with ratatui (migrated from vol-llm-tui)"
```

---

### Task 8: Web Frontend (Dioxus WASM)

**Files:**
- Create: `crates/vol-llm-ui/src/web/bin/web.rs`
- Create: `crates/vol-llm-ui/src/web/components/mod.rs`
- Create: `crates/vol-llm-ui/src/web/components/app.rs`
- Create: `crates/vol-llm-ui/src/web/components/conversation.rs`
- Create: `crates/vol-llm-ui/src/web/components/status_bar.rs`
- Create: `crates/vol-llm-ui/src/web/components/input_area.rs`
- Create: `crates/vol-llm-ui/src/web/components/tools_panel.rs`

- [ ] **Step 1: Create component modules**

```rust
// crates/vol-llm-ui/src/web/components/mod.rs
pub mod app;
pub mod conversation;
pub mod status_bar;
pub mod input_area;
pub mod tools_panel;
```

- [ ] **Step 2: Create app.rs — root component**

```rust
// crates/vol-llm-ui/src/web/components/app.rs

use dioxus::prelude::*;
use crate::state::UiState;

use super::conversation::ConversationView;
use super::status_bar::StatusBar;
use super::input_area::InputArea;
use super::tools_panel::ToolsPanel;

#[component]
pub fn App(state: Signal<UiState>) -> Element {
    rsx! {
        div {
            class: "app",
            style: "display: flex; flex-direction: column; height: 100vh; font-family: monospace;",

            StatusBar { state }

            div {
                style: "display: flex; flex: 1; overflow: hidden;",

                // Tools panel (30%)
                div { style: "width: 30%; border-right: 1px solid #333; overflow: auto;",
                    ToolsPanel { state }
                }

                // Content panel (70%)
                div { style: "width: 70%; display: flex; flex-direction: column;",
                    // Tab bar
                    div { style: "display: flex; border-bottom: 1px solid #333; padding: 4px;",
                        TabButton { state, label: "Conversation", tab: crate::state::ActiveTab::Conversation }
                        TabButton { state, label: "Workspace", tab: crate::state::ActiveTab::Workspace }
                        TabButton { state, label: "Skills", tab: crate::state::ActiveTab::Skills }
                        TabButton { state, label: "Logs", tab: crate::state::ActiveTab::Logs }
                    }

                    // Tab content
                    div { style: "flex: 1; overflow: auto; padding: 8px;",
                        ConversationView { state }
                    }

                    // Input area
                    InputArea { state }
                }
            }
        }
    }
}

#[component]
fn TabButton(state: Signal<UiState>, label: String, tab: crate::state::ActiveTab) -> Element {
    let is_active = state.read().active_tab == tab;
    let style = if is_active {
        "background: #fff; color: #000; font-weight: bold; padding: 2px 8px; cursor: pointer;"
    } else {
        "color: #666; padding: 2px 8px; cursor: pointer;"
    };

    rsx! {
        button {
            style,
            onclick: move |_| state.write().active_tab = tab,
            {label}
        }
    }
}
```

- [ ] **Step 3: Create conversation.rs**

```rust
// crates/vol-llm-ui/src/web/components/conversation.rs

use dioxus::prelude::*;
use crate::state::{UiState, ConversationEntry};

#[component]
pub fn ConversationView(state: Signal<UiState>) -> Element {
    let entries = state.read().conversation.clone();

    if entries.is_empty() {
        return rsx! {
            div { style: "color: #666; text-align: center; padding: 20px;",
                "No messages yet. Type a query below."
            }
        };
    }

    rsx! {
        div { style: "display: flex; flex-direction: column;",
            for entry in entries {
                {render_entry(entry)}
            }
        }
    }
}

fn render_entry(entry: ConversationEntry) -> Element {
    match entry {
        ConversationEntry::UserInput { text } => {
            rsx! {
                div { style: "margin: 8px 0;",
                    div { style: "color: #0ff; font-weight: bold;", ">>> " }
                    div { style: "color: #fff; white-space: pre-wrap;", "{text}" }
                    div { style: "height: 8px;" }
                }
            }
        }
        ConversationEntry::Thinking { content } => {
            rsx! {
                div { style: "margin: 4px 0;",
                    div { style: "color: #ff0; font-weight: bold;", "Thinking" }
                    div { style: "color: #666; padding-left: 16px; white-space: pre-wrap;", "{content}" }
                }
            }
        }
        ConversationEntry::ContentStreaming { content } => {
            rsx! {
                div { style: "color: #fff; white-space: pre-wrap;", "{content}" }
            }
        }
        ConversationEntry::ToolCall { tool_name, arg_preview } => {
            rsx! {
                div { style: "margin: 4px 0;",
                    div { style: "color: #00f; font-weight: bold;", "[{tool_name}]" }
                    if !arg_preview.is_empty() {
                        div { style: "color: #666; padding-left: 16px;", "{arg_preview}" }
                    }
                }
            }
        }
        ConversationEntry::ToolResult { tool_name, preview, success } => {
            let color = if success { "#0f0" } else { "#f00" };
            let status = if success { "OK" } else { "ERR" };
            rsx! {
                div { style: "margin: 4px 0;",
                    div { style: "color: {color};", "  {status} {tool_name}" }
                    div { style: "color: #666; padding-left: 32px; white-space: pre-wrap; max-height: 120px; overflow: hidden;", "{preview}" }
                    div { style: "height: 8px;" }
                }
            }
        }
        ConversationEntry::AgentAnswer { text } => {
            rsx! {
                div { style: "margin: 8px 0; color: #fff; white-space: pre-wrap;", "{text}" }
                div { style: "height: 8px;" }
            }
        }
        ConversationEntry::RunSummary { iterations, tool_calls, elapsed_ms } => {
            rsx! {
                div { style: "color: #0f0; font-weight: bold;",
                    "Done · {iterations} iteration{if iterations != 1 { "s" } else { "" }} · {tool_calls} tool call{if tool_calls != 1 { "s" } else { "" }} · {elapsed_ms}ms"
                }
            }
        }
        ConversationEntry::Error { message } => {
            rsx! {
                div { style: "color: #f00; font-weight: bold;", "Error: {message}" }
            }
        }
    }
}
```

- [ ] **Step 4: Create status_bar.rs**

```rust
// crates/vol-llm-ui/src/web/components/status_bar.rs

use dioxus::prelude::*;
use crate::state::UiState;

#[component]
pub fn StatusBar(state: Signal<UiState>) -> Element {
    let elapsed = if state.read().is_running {
        state.read().run_start.map(|s| s.elapsed()).unwrap_or_default()
    } else {
        state.read().run_elapsed
    };
    let secs = elapsed.as_secs();
    let time_str = format!("{:02}:{:02}", secs / 60, secs % 60);
    let status = if state.read().is_running { "Running" } else { "Idle" };
    let session_id = &state.read().session_id;
    let run_count = state.read().run_count;
    let iteration = state.read().iteration;
    let tool_count = state.read().tool_call_count;

    rsx! {
        div {
            style: "background: #333; color: #fff; padding: 4px 8px; font-size: 12px;",
            "Session: {session_id} │ Run: {run_count} │ Iter: {iteration} │ Tools: {tool_count} │ Time: {time_str} │ {status}"
        }
    }
}
```

- [ ] **Step 5: Create input_area.rs**

```rust
// crates/vol-llm-ui/src/web/components/input_area.rs

use dioxus::prelude::*;
use crate::state::UiState;

#[component]
pub fn InputArea(
    state: Signal<UiState>,
    on_submit: EventHandler<String>,
) -> Element {
    let is_running = state.read().is_running;

    rsx! {
        div { style: "border-top: 1px solid #333; padding: 8px;",
            if is_running {
                div { style: "color: #ff0;", "Running... (input disabled)" }
            } else {
                form {
                    onsubmit: move |evt| {
                        let value = evt.values().get("input").and_then(|v| v.as_string()).unwrap_or_default();
                        if !value.is_empty() {
                            on_submit.call(value);
                        }
                    },
                    textarea {
                        name: "input",
                        style: "width: 100%; background: #111; color: #fff; border: 1px solid #333; padding: 8px; resize: vertical;",
                        rows: "3",
                        placeholder: "Type your message...",
                        disabled: is_running,
                    }
                    div { style: "color: #666; font-size: 11px; margin-top: 4px;",
                        "Enter to send · Esc to clear"
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 6: Create tools_panel.rs**

```rust
// crates/vol-llm-ui/src/web/components/tools_panel.rs

use dioxus::prelude::*;
use crate::state::UiState;

#[component]
pub fn ToolsPanel(state: Signal<UiState>) -> Element {
    let tool_calls = state.read().tool_calls.clone();

    rsx! {
        div { style: "padding: 8px;",
            div { style: "color: #00f; font-weight: bold; margin-bottom: 8px;",
                "Tools Called ({tool_calls.len()})"
            }
            if tool_calls.is_empty() {
                div { style: "color: #666;", "No tool calls yet" }
            } else {
                for entry in tool_calls {
                    ToolCallItem { entry }
                }
            }
        }
    }
}

#[component]
fn ToolCallItem(entry: crate::state::ToolCallEntry) -> Element {
    let (status_str, color) = match entry.status {
        crate::state::ToolCallStatus::Running => ("…", "#ff0"),
        crate::state::ToolCallStatus::Success => ("✓", "#0f0"),
        crate::state::ToolCallStatus::Error => ("ERR", "#f00"),
        crate::state::ToolCallStatus::Skipped => ("SKIP", "#666"),
    };

    let dur = entry.duration_ms.map(|ms| format!("{ms}ms")).unwrap_or_default();

    rsx! {
        div { style: "margin-bottom: 8px;",
            div { style: "color: {color}; font-weight: bold;",
                "{entry.sequence}. [{entry.tool_name}] {status_str} {dur}"
            }
            div { style: "color: #666; font-size: 11px;", "{entry.arg_preview}" }
        }
    }
}
```

- [ ] **Step 7: Create web.rs — WASM entry point**

```rust
// crates/vol-llm-ui/src/web/bin/web.rs

//! vol-llm-ui-web: Web UI for agent interaction using Dioxus WASM.

#![allow(non_snake_case)]

use dioxus::prelude::*;
use vol_llm_ui::state::UiState;

mod components;

use components::app::App;

fn main() {
    dioxus::launch(move || {
        // Initialize state
        let state = use_signal(|| UiState::new(
            uuid::Uuid::new_v4().to_string(),
            ".",
        ));

        // Remote URL from env or default
        let remote_url = use_signal(|| {
            web_sys::window()
                .and_then(|w| w.location().search().ok())
                .and_then(|qs| {
                    qs.split('&')
                        .find(|p| p.starts_with("url="))
                        .map(|p| p.strip_prefix("url=").unwrap().to_string())
                })
                .unwrap_or_else(|| "ws://localhost:3001/ws".to_string())
        });

        rsx! {
            App { state }
        }
    });
}
```

- [ ] **Step 8: Verify Web compiles**

Run: `cargo check -p vol-llm-ui --bin vol-llm-ui-web --features web 2>&1 | tail -10`
Expected: compiles cleanly (may have WASM-specific warnings)

- [ ] **Step 9: Commit**

```bash
git add crates/vol-llm-ui/src/web/
git commit -m "feat: add Dioxus Web WASM frontend with all components"
```

---

### Task 9: JSON-RPC Server (vol-llm-agent-channel extension)

**Files:**
- Create: `crates/vol-llm-agent-channel/src/jsonrpc/mod.rs`
- Create: `crates/vol-llm-agent-channel/src/jsonrpc/handler.rs`
- Create: `crates/vol-llm-agent-channel/examples/agent-service.rs`
- Modify: `crates/vol-llm-agent-channel/Cargo.toml`

- [ ] **Step 1: Add jsonrpsee to Cargo.toml**

Add to `[dependencies]` in `crates/vol-llm-agent-channel/Cargo.toml`:

```toml
jsonrpsee = { version = "0.26", features = ["server", "ws-server"] }
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p vol-llm-agent-channel 2>&1 | tail -5`
Expected: compiles cleanly

- [ ] **Step 3: Create jsonrpc/mod.rs**

```rust
// crates/vol-llm-agent-channel/src/jsonrpc/mod.rs

mod handler;

pub use handler::JsonRpcHandler;
```

- [ ] **Step 4: Create jsonrpc/handler.rs**

```rust
// crates/vol-llm-agent-channel/src/jsonrpc/handler.rs

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use serde_json::{json, Value, Map};
use tokio::sync::RwLock;
use crate::connection::ConnectionHolder;
use crate::router::AgentRouter;
use crate::request::AgentRequest;
use crate::transport::WsServer;
use jsonrpsee::core::Error as RpcError;

pub struct JsonRpcHandler {
    pub router: AgentRouter,
    pub holders: Arc<RwLock<HashMap<String, Arc<ConnectionHolder>>>>,
    pub working_dir: PathBuf,
    pub store_dir: PathBuf,
    pub logs_dir: PathBuf,
}

impl JsonRpcHandler {
    pub fn new(router: AgentRouter, working_dir: PathBuf, store_dir: PathBuf, logs_dir: PathBuf) -> Self {
        Self {
            router,
            holders: Arc::new(RwLock::new(HashMap::new())),
            working_dir,
            store_dir,
            logs_dir,
        }
    }

    pub async fn handle_request(&self, method: &str, params: Value) -> Result<Value, RpcError> {
        match method {
            "agent.submit" => self.agent_submit(params).await,
            "agent.cancel" => self.agent_cancel(params).await,
            "agent.approve" => self.agent_approve(params).await,
            "file.list" => self.file_list(params).await,
            "file.read" => self.file_read(params).await,
            "log.list" => self.log_list(params).await,
            "log.read" => self.log_read(params).await,
            "session.list" => self.session_list(params).await,
            "session.resume" => self.session_resume(params).await,
            _ => Err(RpcError::MethodNotFound(method.to_string())),
        }
    }

    async fn agent_submit(&self, params: Value) -> Result<Value, RpcError> {
        let agent_id = params.get("agent_id").and_then(|v| v.as_str()).unwrap_or("default");
        let input = params.get("input").and_then(|v| v.as_str()).ok_or_else(|| {
            RpcError::InvalidParams("missing 'input' field".into())
        })?;

        let request = AgentRequest::new(agent_id, input);
        match self.router.send(agent_id, request).await {
            Ok(rx) => {
                // Spawn a task to forward events back to the client
                // In a real implementation, this uses jsonrpsee subscriptions
                // For this example, we return the req_id
                Ok(json!({ "status": "submitted", "agent_id": agent_id }))
            }
            Err(e) => Err(RpcError::InternalError(e.to_string().into())),
        }
    }

    async fn agent_cancel(&self, _params: Value) -> Result<Value, RpcError> {
        Ok(json!({ "ok": true }))
    }

    async fn agent_approve(&self, _params: Value) -> Result<Value, RpcError> {
        Ok(json!({ "ok": true }))
    }

    async fn file_list(&self, params: Value) -> Result<Value, RpcError> {
        let path = params.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let full_path = self.working_dir.join(path);
        let entries = std::fs::read_dir(&full_path).map_err(|e| {
            RpcError::InternalError(format!("cannot read {}: {}", full_path.display(), e).into())
        })?;

        let items: Vec<Value> = entries.filter_map(|e| {
            let e = e.ok()?;
            let metadata = e.metadata().ok()?;
            Some(json!({
                "name": e.file_name().to_string_lossy(),
                "is_dir": metadata.is_dir(),
                "size": metadata.len(),
            }))
        }).collect();

        Ok(json!({ "entries": items }))
    }

    async fn file_read(&self, params: Value) -> Result<Value, RpcError> {
        let path = params.get("path").and_then(|v| v.as_str()).ok_or_else(|| {
            RpcError::InvalidParams("missing 'path' field".into())
        })?;
        let full_path = self.working_dir.join(path);
        let content = std::fs::read_to_string(&full_path).map_err(|e| {
            RpcError::InternalError(format!("cannot read {}: {}", full_path.display(), e).into())
        })?;
        Ok(json!({ "content": content, "encoding": "utf8" }))
    }

    async fn log_list(&self, _params: Value) -> Result<Value, RpcError> {
        let runs = Vec::<Value>::new(); // Placeholder — implement when log format is finalized
        Ok(json!({ "runs": runs }))
    }

    async fn log_read(&self, params: Value) -> Result<Value, RpcError> {
        let run_id = params.get("run_id").and_then(|v| v.as_str()).ok_or_else(|| {
            RpcError::InvalidParams("missing 'run_id' field".into())
        })?;
        let log_path = self.logs_dir.join(format!("{}.jsonl", run_id));
        let content = std::fs::read_to_string(&log_path).map_err(|e| {
            RpcError::InternalError(format!("cannot read log: {}", e).into())
        })?;
        Ok(json!({ "entries": content }))
    }

    async fn session_list(&self, _params: Value) -> Result<Value, RpcError> {
        use vol_session::FileSessionEntryStore;
        let store = FileSessionEntryStore::new(&self.store_dir);
        let summaries = store.list_sessions().map_err(|e| {
            RpcError::InternalError(format!("cannot list sessions: {}", e).into())
        })?;
        let items: Vec<Value> = summaries.into_iter().map(|s| {
            json!({ "id": s.session_id, "entry_count": s.entry_count, "created_at": s.created_at })
        }).collect();
        Ok(json!({ "sessions": items }))
    }

    async fn session_resume(&self, params: Value) -> Result<Value, RpcError> {
        // Placeholder — session resume logic
        Ok(json!({ "ok": true }))
    }
}
```

- [ ] **Step 5: Create agent-service.rs example**

```rust
// crates/vol-llm-agent-channel/examples/agent-service.rs

//! Remote agent service with JSON-RPC WebSocket endpoint.
//!
//! Run with:
//! ```bash
//! ANTHROPIC_AUTH_TOKEN=your_key RUST_LOG=info \
//!   cargo run --example agent-service -p vol-llm-agent-channel
//! ```
//!
//! Endpoints:
//! - `GET /ws` — WebSocket for JSON-RPC communication

use std::collections::HashMap;
use std::sync::Arc;
use std::path::PathBuf;

use axum::extract::ws::{WebSocket, Message as WsMessage};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tracing::info;
use vol_llm_agent::agent_def::AgentDef;
use vol_llm_agent::react::{AgentConfig, ReActAgent};
use vol_llm_agent_channel::{AgentDispatcher, AgentRouter, ConnectionHolder};
use vol_llm_agent_channel::jsonrpc::JsonRpcHandler;
use vol_llm_provider::create_provider;
use vol_llm_tool::ToolRegistry;
use vol_session::{FileSessionEntryStore, InMemoryEntryStore, Session};

#[derive(Clone)]
struct AppState {
    handler: Arc<JsonRpcHandler>,
}

fn make_agent(llm: Arc<dyn vol_llm_core::LLMClient>, name: &str, prompt: &str) -> ReActAgent {
    let def = AgentDef::new(name, prompt).with_type(name);
    let session = Arc::new(Session::new(Arc::new(InMemoryEntryStore::new())));
    let tools = Arc::new(ToolRegistry::new());
    let mut config = AgentConfig::new(llm, tools, session);
    config.def = Some(def);
    ReActAgent::new(config)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let llm = create_provider(&vol_llm_provider::LLMConfig::with_env_key(
        vol_llm_core::LLMProvider::Anthropic,
        "claude-sonnet-4-6",
        "ANTHROPIC_AUTH_TOKEN",
        "https://coding.dashscope.aliyuncs.com/apps/anthropic",
    ))
    .expect("failed to create LLM provider — set ANTHROPIC_AUTH_TOKEN");

    let llm: Arc<dyn vol_llm_core::LLMClient> = Arc::from(llm);

    let agents = [
        ("default", "You are a helpful AI assistant."),
    ];

    let router = AgentRouter::new();
    let mut holders = HashMap::new();

    for (id, prompt) in &agents {
        let agent = make_agent(llm.clone(), id, prompt);
        let holder = Arc::new(ConnectionHolder::new(id.to_string(), "client".to_string()));
        let dispatcher = Arc::new(AgentDispatcher::new(agent));
        router.register(id.to_string(), dispatcher.clone()).await;
        holders.insert(id.to_string(), holder);
        info!(agent_id = id, "Agent registered");
    }

    let working_dir = std::env::current_dir().unwrap_or_default();
    let home = std::env::var("HOME").unwrap_or_default();
    let project = working_dir.file_name().unwrap_or(std::ffi::OsStr::new("default")).to_string_lossy();
    let base = PathBuf::from(home).join(".vol-coding").join(project.as_ref());

    let handler = Arc::new(JsonRpcHandler::new(
        router,
        working_dir,
        base.join("sessions"),
        base.join("logs"),
    ));

    *handler.holders.write().await = holders;

    let state = AppState { handler };

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/health", get(|| async { Json(json!({"status": "ok"})) }))
        .with_state(state);

    info!("Starting agent service on 0.0.0.0:3001");
    info!("  WS    /ws  (JSON-RPC)");
    info!("  GET   /health");

    let listener = TcpListener::bind("0.0.0.0:3001").await.expect("failed to bind");
    axum::serve(listener, app).await.expect("server failed");
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

async fn handle_ws(socket: WebSocket, state: AppState) {
    let (mut tx, mut rx) = socket.split();

    while let Some(Ok(msg)) = rx.next().await {
        let text = match msg {
            WsMessage::Text(t) => t,
            WsMessage::Binary(b) => String::from_utf8_lossy(&b).to_string(),
            _ => continue,
        };

        let request: Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => {
                let _ = tx.send(WsMessage::Text(json!({
                    "jsonrpc": "2.0",
                    "error": { "code": -32700, "message": format!("parse error: {}", e) },
                    "id": null,
                }).to_string())).await;
                continue;
            }
        };

        let method = request.get("method").and_then(|v| v.as_str()).unwrap_or("");
        let params = request.get("params").cloned().unwrap_or(Value::Null);
        let id = request.get("id").cloned();

        let result = state.handler.handle_request(method, params).await;

        let response = match result {
            Ok(value) => json!({
                "jsonrpc": "2.0",
                "result": value,
                "id": id,
            }),
            Err(e) => json!({
                "jsonrpc": "2.0",
                "error": { "code": -32603, "message": e.to_string() },
                "id": id,
            }),
        };

        let _ = tx.send(WsMessage::Text(response.to_string())).await;
    }
}
```

- [ ] **Step 6: Verify compilation**

Run: `cargo check -p vol-llm-agent-channel --all-targets 2>&1 | tail -10`
Expected: compiles cleanly

- [ ] **Step 7: Commit**

```bash
git add crates/vol-llm-agent-channel/Cargo.toml crates/vol-llm-agent-channel/src/jsonrpc/ crates/vol-llm-agent-channel/examples/agent-service.rs
git commit -m "feat: add JSON-RPC WebSocket server to vol-llm-agent-channel"
```

---

### Task 10: Final Verification

- [ ] **Step 1: Build entire workspace**

Run: `cargo build --workspace 2>&1 | tail -10`
Expected: compiles cleanly, existing crates unaffected

- [ ] **Step 2: Verify existing TUI still compiles**

Run: `cargo build -p vol-llm-tui 2>&1 | tail -5`
Expected: compiles with no changes

- [ ] **Step 3: Run vol-llm-ui tests**

Run: `cargo test -p vol-llm-ui 2>&1 | tail -10`
Expected: all tests pass (6+ tests)

- [ ] **Step 4: Run vol-llm-agent-channel tests**

Run: `cargo test -p vol-llm-agent-channel 2>&1 | tail -10`
Expected: all tests pass

- [ ] **Step 5: Commit**

```bash
git add .
git commit -m "chore: final verification — all crates compile and tests pass"
```
