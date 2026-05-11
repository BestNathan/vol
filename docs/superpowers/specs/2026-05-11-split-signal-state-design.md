# Component-Split Signal State Architecture

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the single `Signal<UiState>` with per-component Signals so each component only re-renders when its own state changes.

**Architecture:** Trait-based `Reducer<T>` pattern. Each domain defines its own `Reducer` that handles a subset of `UiEvent` variants. The WS event loop calls `reducer.reduce(signal, event)`. Cross-domain events (like `agent_start`) call multiple reducers in sequence.

**Tech Stack:** Dioxus `Signal<T>`, `use_context_provider`, trait-based Reducer pattern, JSON-RPC event loop.

---

## 1. Architecture Overview

```
WebSocket (JsonRpcClient)
    │
    ├─ on_state_change callback ──→ Signal<GlobalState>
    │
    └─ next_event() loop ──→ AgentEvent
                               │
                               ▼
                    agent_event_to_ui() → UiEvent
                               │
                               ▼
              ┌────────────────┼────────────────┐
              ▼                ▼                ▼
        Reducer<T>        Reducer<T>       Reducer<T>
        (Conversation)    (Tool)           (Approval)
              │                │                │
              ▼                ▼                ▼
        Signal<Conv>      Signal<Tool>     Signal<Approval>
              │                │                │
              ▼                ▼                ▼
        ConversationView  ToolsPanel       ApprovalDialog
```

Key principles:
- Each component owns exactly **one** `Signal<T>` it reads from
- Each `UiEvent` variant maps to **one** reducer (except cross-domain events)
- Cross-domain events (e.g. `agent_start`) touch multiple signals sequentially — no locking needed since Dioxus `with_mut` is synchronous
- WS connection state changes go directly to `GlobalState`

## 2. State Definitions

### 2.1 GlobalState (StatusBar, InputArea, TabBar)

```rust
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
```

### 2.2 WorkspaceState (FileTree, FileContentView)

```rust
pub struct WorkspaceState {
    pub workspace: WorkspaceTreeNode,
    pub modified_files: HashSet<String>,
    pub open_files: Vec<OpenFileTab>,
    pub selected_file_tab: Option<usize>,
    pub collapsed_dirs: HashSet<String>,
    pub workspace_scroll: u16,
}
```

### 2.3 ConversationState (ConversationView)

```rust
pub struct ConversationState {
    pub entries: Vec<ConversationEntry>,
    pub conversation_scroll: u16,
    pub auto_scroll: bool,
}
```

### 2.4 ToolState (ToolsPanel, ToolsTabContent)

```rust
pub struct ToolState {
    pub calls: Vec<ToolCallEntry>,
    pub expanded: HashSet<usize>,
    pub scroll: u16,
}
```

### 2.5 LogState (LogViewer)

```rust
pub struct LogState {
    pub selected_run: Option<String>,
    pub entries: Vec<LogLine>,
    pub scroll: u16,
    pub auto_scroll: bool,
    pub run_logs: Vec<LogRunSummary>,
}
```

### 2.6 SkillState (SkillsPanel)

```rust
pub struct SkillState {
    pub skills: Vec<SkillDisplayEntry>,
}
```

### 2.7 ApprovalState (ApprovalDialog)

```rust
pub struct ApprovalState {
    pub tool_name: Option<String>,
    pub reason: Option<String>,
    pub arguments: Option<String>,
    pub response: Option<(bool, Option<String>)>,
}
```

### 2.8 SessionState (SessionDialog)

```rust
pub struct SessionState {
    pub open: bool,
    pub sessions: Vec<SessionDialogEntry>,
    pub selected: usize,
}
```

## 3. AppState Container

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

## 4. Reducer Pattern

### 4.1 Trait Definition

```rust
/// A reducer handles a subset of UiEvent variants for a specific domain.
pub trait Reducer<T> {
    /// Apply an event to the state. Returns `true` if the event was handled.
    fn reduce(state: &mut T, event: &UiEvent) -> bool;
}
```

### 4.2 Conversation Reducer

```rust
pub struct ConversationReducer;

impl Reducer<ConversationState> for ConversationReducer {
    fn reduce(s: &mut ConversationState, event: &UiEvent) -> bool {
        match event {
            UiEvent::AgentStart { input } => {
                s.entries.push(ConversationEntry::UserInput { text: input.clone() });
                if s.auto_scroll { s.conversation_scroll = 0; }
                true
            }
            UiEvent::AgentComplete { response } => {
                flush_pending_content(&mut s.entries);
                if !response.is_empty() {
                    s.entries.push(ConversationEntry::AgentAnswer { text: response.clone() });
                }
                if s.auto_scroll { s.conversation_scroll = 0; }
                true
            }
            UiEvent::AgentAborted { reason } | UiEvent::AgentError { message: reason } => {
                flush_pending_content(&mut s.entries);
                s.entries.push(ConversationEntry::Error { message: reason.clone() });
                true
            }
            UiEvent::ThinkingStart => {
                s.entries.push(ConversationEntry::Thinking { content: String::new() });
                true
            }
            UiEvent::ThinkingDelta { delta } => {
                if let Some(ConversationEntry::Thinking { content }) = s.entries.last_mut() {
                    content.push_str(delta);
                }
                true
            }
            UiEvent::ThinkingComplete => true, // No-op, content already streamed
            UiEvent::ContentStart => {
                s.entries.push(ConversationEntry::ContentStreaming { content: String::new() });
                true
            }
            UiEvent::ContentDelta { delta } => {
                if let Some(ConversationEntry::ContentStreaming { content }) = s.entries.last_mut() {
                    content.push_str(delta);
                }
                true
            }
            UiEvent::ContentComplete { content } => {
                if let Some(ConversationEntry::ContentStreaming { .. }) = s.entries.last() {
                    let entry = s.entries.last_mut().unwrap();
                    *entry = ConversationEntry::AgentAnswer { text: content.clone() };
                } else if !content.is_empty() {
                    s.entries.push(ConversationEntry::AgentAnswer { text: content.clone() });
                }
                true
            }
            UiEvent::MaxIterationsReached { current, max } => {
                s.entries.push(ConversationEntry::Error {
                    message: format!("Max iterations reached ({}/{}) — waiting for user decision...", current, max),
                });
                true
            }
            UiEvent::IterationContinued { from_iteration } => {
                s.entries.push(ConversationEntry::AgentAnswer {
                    text: format!("Continuing from iteration {from_iteration} (counter reset to 0)"),
                });
                true
            }
            UiEvent::IterationComplete { iteration, final_answer } => {
                if let Some(answer) = final_answer {
                    s.entries.push(ConversationEntry::AgentAnswer { text: answer.clone() });
                }
                true
            }
            _ => false,
        }
    }
}
```

### 4.3 Tool Reducer

```rust
pub struct ToolReducer;

impl Reducer<ToolState> for ToolReducer {
    fn reduce(s: &mut ToolState, event: &UiEvent) -> bool {
        match event {
            UiEvent::ToolCallBegin { tool_name, arguments } => {
                let seq = s.calls.len() as u32 + 1;
                let preview = extract_arg_preview(arguments);
                s.calls.push(ToolCallEntry {
                    sequence: seq,
                    tool_name: tool_name.clone(),
                    arg_preview: preview,
                    status: ToolCallStatus::Running,
                    duration_ms: None,
                });
                s.scroll = s.calls.len() as u16;
                true
            }
            UiEvent::ToolCallComplete { tool_name, duration_ms, .. } => {
                update_tool_call_status(&mut s.calls, tool_name, ToolCallStatus::Success, *duration_ms);
                true
            }
            UiEvent::ToolCallError { tool_name, duration_ms, .. } => {
                update_tool_call_status(&mut s.calls, tool_name, ToolCallStatus::Error, *duration_ms);
                true
            }
            UiEvent::ToolCallSkipped { tool_name, duration_ms, .. } => {
                update_tool_call_status(&mut s.calls, tool_name, ToolCallStatus::Skipped, *duration_ms);
                true
            }
            _ => false,
        }
    }
}
```

### 4.4 Global Reducer

```rust
pub struct GlobalReducer;

impl Reducer<GlobalState> for GlobalReducer {
    fn reduce(s: &mut GlobalState, event: &UiEvent) -> bool {
        match event {
            UiEvent::AgentStart { .. } => {
                s.run_count += 1;
                s.iteration = 0;
                s.tool_call_count = 0;
                s.run_start = Some(Instant::now());
                s.run_elapsed = std::time::Duration::ZERO;
                s.is_running = true;
                true
            }
            UiEvent::AgentComplete { .. } | UiEvent::AgentAborted { .. } | UiEvent::AgentError { .. } => {
                if let Some(start) = s.run_start {
                    s.run_elapsed = start.elapsed();
                }
                s.is_running = false;
                true
            }
            UiEvent::IterationComplete { iteration, .. } => {
                s.iteration = *iteration;
                true
            }
            _ => false,
        }
    }
}
```

### 4.5 Approval Reducer

```rust
pub struct ApprovalReducer;

impl Reducer<ApprovalState> for ApprovalReducer {
    fn reduce(s: &mut ApprovalState, event: &UiEvent) -> bool {
        match event {
            UiEvent::ApprovalRequest { tool_name, reason, arguments } => {
                s.tool_name = Some(tool_name.clone());
                s.reason = Some(reason.clone());
                s.arguments = Some(arguments.clone());
                true
            }
            UiEvent::ApprovalResolved { .. } => {
                s.tool_name = None;
                s.reason = None;
                s.arguments = None;
                s.response = None;
                true
            }
            _ => false,
        }
    }
}
```

### 4.6 Workspace Reducer

```rust
pub struct WorkspaceReducer;

impl Reducer<WorkspaceState> for WorkspaceReducer {
    fn reduce(s: &mut WorkspaceState, event: &UiEvent) -> bool {
        match event {
            // File modification events from agent (if the server sends them)
            _ => false, // Workspace state is primarily user-driven, not event-driven
        }
    }
}
```

Note: WorkspaceState mutations happen via user interaction (clicking files, expanding dirs), not WS events. The reducer exists for completeness but is mostly a no-op.

## 5. WS Binding

### 5.1 Event Parsing

The existing `agent_event_to_ui()` function converts `AgentEvent` → `Option<UiEvent>`:

```rust
fn agent_event_to_ui(event: &AgentEvent) -> Option<UiEvent> {
    let data = &event.data;
    match event.event_type.as_str() {
        "agent_start" => Some(UiEvent::AgentStart {
            input: data.get("input").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        }),
        // ... all other variants ...
        _ => None,
    }
}
```

### 5.2 Event Loop with Reducer Routing

```rust
// In App component's event loop:
let app_state = AppState { ... }; // All 8 signals + rpc_client

wasm_bindgen_futures::spawn_local(async move {
    loop {
        match client_ev.next_event().await {
            Some(agent_event) => {
                if let Some(ui_event) = agent_event_to_ui(&agent_event) {
                    // Route to each reducer sequentially
                    ConversationReducer::reduce(&mut app_state.conversation, &ui_event);
                    ToolReducer::reduce(&mut app_state.tools, &ui_event);
                    GlobalReducer::reduce(&mut app_state.global, &ui_event);
                    ApprovalReducer::reduce(&mut app_state.approval, &ui_event);
                    WorkspaceReducer::reduce(&mut app_state.workspace, &ui_event);
                    LogReducer::reduce(&mut app_state.logs, &ui_event);
                    SkillReducer::reduce(&mut app_state.skills, &ui_event);
                    SessionReducer::reduce(&mut app_state.session, &ui_event);
                }
            }
            None => {
                app_state.global.with_mut(|g| {
                    g.ws_connected = false;
                    g.ws_last_error = Some("Event stream closed".to_string());
                });
                break;
            }
        }
    }
});
```

The reducer `reduce()` method returns `true` if it handled the event. The `Signal::with_mut()` wrapper is handled **inside** each reducer call, so the reducer trait doesn't need to know about signals:

```rust
// Actual implementation — each reducer wraps in with_mut:
impl AppState {
    pub fn dispatch(&self, event: &UiEvent) {
        self.conversation.with_mut(|s| { ConversationReducer::reduce(s, event); });
        self.tools.with_mut(|s| { ToolReducer::reduce(s, event); });
        self.global.with_mut(|s| { GlobalReducer::reduce(s, event); });
        self.approval.with_mut(|s| { ApprovalReducer::reduce(s, event); });
        self.workspace.with_mut(|s| { WorkspaceReducer::reduce(s, event); });
        self.logs.with_mut(|s| { LogReducer::reduce(s, event); });
        self.skills.with_mut(|s| { SkillReducer::reduce(s, event); });
        self.session.with_mut(|s| { SessionReducer::reduce(s, event); });
    }
}
```

### 5.3 Connection State Binding

```rust
c.on_state_change(move |cs| {
    app_state.global.with_mut(|g| match cs {
        ConnectionState::Connected => {
            g.ws_connected = true;
            g.ws_last_error = None;
        }
        ConnectionState::Connecting => {
            g.ws_connected = false;
        }
        ConnectionState::Disconnected => {
            g.ws_connected = false;
            g.ws_last_error = Some("Disconnected from server".to_string());
        }
    });
    // Initial workspace load
    let rpc = app_state.rpc_client.clone();
    let ws = app_state.workspace.clone();
    rpc.file_list(".", move |result| {
        if let Ok(entries) = result {
            ws.with_mut(|w| {
                w.workspace.children.clear();
                for entry in &entries {
                    w.workspace.children.push(WorkspaceTreeNode { ... });
                    if entry.is_dir {
                        w.collapsed_dirs.insert(entry.name.clone());
                    }
                }
                w.workspace.loaded = true;
            });
        }
    });
});
```

## 6. Component Subscription

Each component reads **only** its relevant signal:

```rust
// FileTree — only reads WorkspaceState
#[component]
pub fn FileTree() -> Element {
    let workspace_state: Signal<WorkspaceState> = use_context();
    let workspace = workspace_state.read();

    rsx! {
        div { class: "sidebar",
            for child in &workspace.workspace.children {
                TreeNode { node: child.clone(), depth: 0, key: "{child.path}" }
            }
        }
    }
}
```

```rust
// StatusBar — only reads GlobalState
#[component]
pub fn StatusBar() -> Element {
    let global: Signal<GlobalState> = use_context();
    let g = global.read();

    rsx! {
        div { class: "status-bar",
            span { "{g.is_running}" }
            span { "{g.ws_connected}" }
        }
    }
}
```

```rust
// ConversationView — only reads ConversationState
#[component]
pub fn ConversationView() -> Element {
    let conversation: Signal<ConversationState> = use_context();
    let entries = conversation.read().entries.clone();

    rsx! {
        div { class: "conversation",
            for entry in &entries {
                // render entry
            }
        }
    }
}
```

## 7. AppState Initialization

```rust
#[component]
pub fn App() -> Element {
    let ws_url = derive_ws_url();

    let global = use_signal(|| GlobalState {
        session_id: "web-session".into(),
        run_count: 0,
        iteration: 0,
        tool_call_count: 0,
        run_start: None,
        run_elapsed: Duration::ZERO,
        is_running: false,
        exiting: false,
        ws_url: ws_url.clone(),
        ws_connected: false,
        ws_last_error: None,
        unsafe_mode: false,
        active_tab: ActiveTab::Conversation,
    });

    let conversation = use_signal(|| ConversationState {
        entries: Vec::new(),
        conversation_scroll: 0,
        auto_scroll: true,
    });

    let tools = use_signal(|| ToolState {
        calls: Vec::new(),
        expanded: HashSet::new(),
        scroll: 0,
    });

    let workspace = use_signal(|| WorkspaceState {
        workspace: WorkspaceTreeNode::root("/workspace".into(), ".".into()),
        modified_files: HashSet::new(),
        open_files: Vec::new(),
        selected_file_tab: None,
        collapsed_dirs: HashSet::new(),
        workspace_scroll: 0,
    });

    let logs = use_signal(|| LogState { ... });
    let skills = use_signal(|| SkillState { ... });
    let approval = use_signal(|| ApprovalState::new());
    let session = use_signal(|| SessionState { ... });

    let client = use_hook(|| { ... });

    use_context_provider(|| AppState {
        global, conversation, tools, workspace, logs, skills, approval, session,
        rpc_client: client.clone(),
    });

    // ... WS binding, event loop ...
}
```

## 8. TUI Compatibility

The TUI uses `Arc<RwLock<UiState>>` and re-renders the entire frame at 30fps. Signal splitting is a Dioxus-specific optimization. Keep `UiState` as a flat struct for TUI.

```rust
#[cfg(feature = "tui")]
pub struct UiState {
    // Same fields as the current UiState — flat struct
    // TUI doesn't benefit from signal splitting since it renders the whole frame
}

#[cfg(all(feature = "web", not(feature = "tui")))]
pub use crate::state::split::*; // GlobalState, WorkspaceState, etc.
```

## 9. File Impact

| File | Change |
|------|--------|
| `src/state/mod.rs` | Add feature-gated split state structs + Reducer trait + 8 reducer impls. Keep `UiEvent` enum. Keep all data types. Keep `UiState` for TUI behind `#[cfg(feature = "tui")]`. |
| `src/web/components/app.rs` | Create 8 signals. Wire `AppState.dispatch()` method. Update `AppState` struct. WS event loop calls `app_state.dispatch(&ui_event)`. |
| `src/web/components/file_tree.rs` | Read `Signal<WorkspaceState>` from context. |
| `src/web/components/conversation.rs` | Read `Signal<ConversationState>`. |
| `src/web/components/tools_panel.rs` | Read `Signal<ToolState>`. |
| `src/web/components/tools_tab.rs` | Read `Signal<ToolState>`. |
| `src/web/components/log_viewer.rs` | Read `Signal<LogState>`. |
| `src/web/components/skills.rs` | Read `Signal<SkillState>`. |
| `src/web/components/approval_dialog.rs` | Read `Signal<ApprovalState>`. |
| `src/web/components/session_dialog.rs` | Read `Signal<SessionState>`. |
| `src/web/components/status_bar.rs` | Read `Signal<GlobalState>`. |
| `src/web/components/input_area.rs` | Read `Signal<GlobalState>`. |
| `src/web/components/workspace.rs` | Read `Signal<WorkspaceState>`. |
| `src/web/components/file_content.rs` | Read `Signal<WorkspaceState>`. |

## 10. Error Handling

- If a reducer returns `false` (unhandled), log a debug message
- Signal mutations are atomic per `with_mut` call
- Cross-signal operations are sequential in the dispatch loop — no race conditions
- WS connection state changes go directly to `GlobalState` without routing through `UiEvent`

## 11. Backward Compatibility

- `UiEvent` enum stays the same — it's the wire format from WS
- `UiState` kept for TUI behind `#[cfg(feature = "tui")]`
- All component interfaces stay the same (props, context types)
- Existing tests for `UiEvent` serialization/deserialization remain valid

## 12. Verification

- `cargo test --package vol-llm-ui` — all existing tests pass
- `cargo check -p vol-llm-ui --features web --bin vol-llm-ui-web` — WASM build
- `cargo check -p vol-llm-ui --features tui` — TUI build
- `cargo check -p vol-llm-ui --all-features` — both features
