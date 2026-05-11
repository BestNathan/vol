# Component-Split Signal State Design

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the single `Signal<UiState>` with per-component Signals so each component only re-renders when its own state changes.

**Architecture:** Minimal `Signal<GlobalState>` for shared app-level state (running, connection). Separate `Signal<T>` for each component domain (workspace, conversation, tools, logs, skills, approval, session). WS events route directly to the relevant signal.

**Tech Stack:** Dioxus `Signal<T>`, `use_context_provider`, JSON-RPC event loop.

---

## Design

### 1. State Definitions

```rust
// Global — StatusBar, InputArea read only
pub struct GlobalState {
    pub session_id: String,
    pub run_count: u32,
    pub iteration: u32,
    pub tool_call_count: u32,
    pub run_start: Option<Instant>,
    pub run_elapsed: std::time::Duration,
    pub is_running: bool,
    pub exiting: bool,
    pub ws_url: String,
    pub ws_connected: bool,
    pub ws_last_error: Option<String>,
    pub unsafe_mode: bool,
    pub active_tab: ActiveTab,
}

// Workspace — FileTree, FileContentView
pub struct WorkspaceState {
    pub workspace: WorkspaceTreeNode,
    pub modified_files: HashSet<String>,
    pub open_files: Vec<OpenFileTab>,
    pub selected_file_tab: Option<usize>,
    pub collapsed_dirs: HashSet<String>,
    pub workspace_scroll: u16,
}

// Conversation — ConversationView
pub struct ConversationState {
    pub entries: Vec<ConversationEntry>,
    pub conversation_scroll: u16,
    pub auto_scroll: bool,
}

// Tools — ToolsPanel, ToolsTabContent
pub struct ToolState {
    pub calls: Vec<ToolCallEntry>,
    pub expanded: HashSet<usize>,
    pub scroll: u16,
}

// Logs — LogViewer
pub struct LogState {
    pub selected_run: Option<String>,
    pub entries: Vec<LogLine>,
    pub scroll: u16,
    pub auto_scroll: bool,
    pub run_logs: Vec<LogRunSummary>,
}

// Skills — SkillsPanel
pub struct SkillState {
    pub skills: Vec<SkillDisplayEntry>,
}

// Approval — ApprovalDialog
pub struct ApprovalState {
    pub tool_name: Option<String>,
    pub reason: Option<String>,
    pub arguments: Option<String>,
    pub response: Option<(bool, Option<String>)>,
}

// Session — SessionDialog
pub struct SessionState {
    pub open: bool,
    pub sessions: Vec<SessionDialogEntry>,
    pub selected: usize,
}
```

### 2. AppState Container

```rust
#[derive(Clone)]
pub struct AppState {
    pub global: Signal<GlobalState>,
    pub workspace: Signal<WorkspaceState>,
    pub conversation: Signal<ConversationState>,
    pub tools: Signal<ToolState>,
    pub logs: Signal<LogState>,
    pub skills: Signal<SkillState>,
    pub approval: Signal<ApprovalState>,
    pub session: Signal<SessionState>,
    pub rpc_client: JsonRpcClient,
}

impl PartialEq for AppState {
    fn eq(&self, _: &Self) -> bool { true }
}
```

### 3. Event Routing

Replace `UiState::apply(UiEvent)` with direct routing in the WS event loop in `App`:

```rust
// In App component's event loop:
match event.event_type.as_str() {
    // → ConversationState
    "agent_start" | "agent_complete" | "agent_error" | "agent_aborted" |
    "thinking_start" | "thinking_delta" | "thinking_complete" |
    "content_start" | "content_delta" | "content_complete" |
    "max_iterations_reached" | "iteration_continued" | "iteration_complete" => {
        state.conversation.with_mut(|cs| { /* apply to cs.entries */ });
    }
    // → ToolState
    "tool_call_begin" | "tool_call_argument_delta" |
    "tool_call_complete" | "tool_call_error" | "tool_call_skipped" => {
        state.tools.with_mut(|ts| { /* apply to ts.calls */ });
    }
    // → ApprovalState
    "approval_request" | "approval_resolved" => {
        state.approval.with_mut(|as| { /* apply */ });
    }
    // → GlobalState
    connection_state_change => {
        state.global.with_mut(|gs| { /* apply */ });
    }
}
```

### 4. Component Migration

Each component reads its own signal instead of `state.signal`:

```rust
// FileTree
let workspace_state: Signal<WorkspaceState> = use_context();
let workspace = workspace_state.read();
// ... uses workspace.workspace, workspace.collapsed_dirs

// ConversationView
let conversation: Signal<ConversationState> = use_context();
let entries = conversation.read().entries.clone();

// etc.
```

### 5. Cross-Signal Operations

Some actions affect multiple signals (e.g., starting a run clears tools AND resets conversation). These are handled in the App event loop:

```rust
"agent_start" => {
    state.global.with_mut(|g| { g.is_running = true; g.run_count += 1; ... });
    state.conversation.with_mut(|c| { c.entries.push(UserInput{...}); });
    state.tools.with_mut(|t| { t.calls.clear(); });
}
```

### 6. File Impact

| File | Change |
|------|--------|
| `src/state/mod.rs` | Replace `UiState` with 8 smaller structs. Delete `UiState::apply()`. Keep `UiEvent` enum. Keep all data types (`ConversationEntry`, `ToolCallEntry`, etc.). |
| `src/web/components/app.rs` | Create 8 signals. Route WS events directly to relevant signals. Update `AppState` struct. |
| `src/web/components/file_tree.rs` | Read `Signal<WorkspaceState>` from context instead of `Signal<UiState>`. |
| `src/web/components/conversation.rs` | Read `Signal<ConversationState>`. |
| `src/web/components/tools_panel.rs` | Read `Signal<ToolState>`. |
| `src/web/components/tools_tab.rs` | Read `Signal<ToolState>`. |
| `src/web/components/log_viewer.rs` | Read `Signal<LogState>`. |
| `src/web/components/skills.rs` | Read `Signal<SkillState>`. |
| `src/web/components/approval_dialog.rs` | Read `Signal<ApprovalState>`. |
| `src/web/components/session_dialog.rs` | Read `Signal<SessionState>`. |
| `src/web/components/workspace.rs` | Read `Signal<WorkspaceState>`. |
| `src/web/components/file_content.rs` | Read `Signal<WorkspaceState>`. |
| `src/web/components/status_bar.rs` | Read `Signal<GlobalState>`. |
| `src/web/components/input_area.rs` | Read `Signal<GlobalState>`. |

### 7. TUI Compatibility

The TUI uses `Arc<RwLock<UiState>>`, not `Signal`. Keep `UiState` as a separate type for TUI, or create a `TuiState` type that mirrors the split. Recommendation: keep `UiState` as a flat struct for TUI only, since TUI re-renders the whole frame every tick anyway — splitting signals only benefits Dioxus's selective rendering.

### 8. Error Handling

- If a WS event type has no handler, log a warning
- Signal mutations are atomic per `with_mut` call
- Cross-signal operations are sequential in the event loop — no race conditions possible

### 9. Backward Compatibility

- `UiEvent` enum stays the same — it's the wire format from WS
- `UiState` can be kept for TUI or deprecated
- All component interfaces stay the same (props, context types)

### 10. Verification

- `cargo test --package vol-llm-ui` — existing tests should pass (they test state application, which will be refactored)
- `cargo check -p vol-llm-ui --features web --bin vol-llm-ui-web` — WASM build
- `cargo check -p vol-llm-ui --features tui` — TUI build
