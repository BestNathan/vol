# Component-Split Signal State Architecture (Pub-Sub)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace centralized `Signal<UiState>` with per-component local signals + pub-sub event bus. Each component owns its own state, subscribes to WS events it cares about, and handles them via a local reducer.

**Architecture:** `EventBus` (publish-subscribe) at `AppState` level. Components create `use_signal` locally, subscribe to event types on mount with a reducer callback, unsubscribe on unmount. WS publishes events to the bus — no global state container, no centralized dispatch loop.

**Tech Stack:** Dioxus `use_signal`, `use_hook`, `use_context`, EventBus pub-sub pattern, `Arc<Mutex>` subscriber registry, JSON-RPC WebSocket event loop.

---

## 1. Architecture Overview

```
WebSocket (JsonRpcClient)
    │
    └─ next_event() loop ──→ AgentEvent
                               │
                               ▼
                     agent_event_to_ui() → UiEvent
                               │
                               ▼
                        EventBus.publish()
                               │
              ┌────────────────┼────────────────┬──────────────┐
              ▼                ▼                ▼              ▼
        FileTree          Conversation     ToolsPanel    StatusBar
        .reduce()         .reduce()        .reduce()     .reduce()
              │                │                │              │
              ▼                ▼                ▼              ▼
        use_signal        use_signal       use_signal    use_signal
        <Workspace>       <Conversation>   <Tools>       <Global>
              │                │                │              │
              ▼                ▼                ▼              ▼
        FileTree UI       ConversationUI   ToolsPanelUI  StatusBarUI
```

Key principles:
- **No global state container** — each component calls `use_signal(|| MyState { ... })` locally
- **Pub-sub event routing** — `EventBus.publish(event)` broadcasts to all subscribers
- **Component owns its reducer** — `struct FileTree; impl HasReducer<WorkspaceState> for FileTree { ... }`
- **Auto-unsubscribe on unmount** — `use_hook` creates `Subscription` guard, drops on unmount
- **No AppState state fields** — `AppState` only holds `EventBus` + `JsonRpcClient`

## 2. EventBus

### 2.1 Core Types

```rust
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Unique subscription ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubscriptionId(u64);

/// A live subscription — drops to unsubscribe.
pub struct Subscription {
    id: SubscriptionId,
    bus: Arc<EventBusInner>,
}

impl Drop for Subscription {
    fn drop(&mut self) {
        self.bus.unsubscribe(self.id);
    }
}
```

### 2.2 EventBus Implementation

```rust
type Handler = Box<dyn Fn(&UiEvent) + Send + Sync>;

struct Subscriber {
    id: SubscriptionId,
    handler: Handler,
}

struct EventBusInner {
    next_id: AtomicU64,
    subscribers: Mutex<Vec<Subscriber>>,
}

#[derive(Clone)]
pub struct EventBus {
    inner: Arc<EventBusInner>,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(EventBusInner {
                next_id: AtomicU64::new(0),
                subscribers: Mutex::new(Vec::new()),
            }),
        }
    }

    /// Subscribe to all events. Returns a Subscription guard that unsubscribes on drop.
    pub fn subscribe<F>(&self, handler: F) -> Subscription
    where
        F: Fn(&UiEvent) + Send + Sync + 'static,
    {
        let id = SubscriptionId(self.inner.next_id.fetch_add(1, Ordering::Relaxed));
        let subscriber = Subscriber {
            id,
            handler: Box::new(handler),
        };
        self.inner.subscribers.lock().unwrap().push(subscriber);
        Subscription {
            id,
            bus: self.inner.clone(),
        }
    }

    /// Publish an event to all subscribers.
    pub fn publish(&self, event: &UiEvent) {
        let subscribers = self.inner.subscribers.lock().unwrap();
        for sub in subscribers.iter() {
            (sub.handler)(event);
        }
    }
}

impl EventBusInner {
    fn unsubscribe(&self, id: SubscriptionId) {
        let mut subs = self.subscribers.lock().unwrap();
        subs.retain(|s| s.id != id);
    }
}
```

### 2.3 AppState (Transport Only, No State)

```rust
#[derive(Clone)]
pub struct AppState {
    pub event_bus: EventBus,
    pub rpc_client: JsonRpcClient,
}

impl PartialEq for AppState {
    fn eq(&self, _: &Self) -> bool { true }
}
```

`AppState` no longer holds any `Signal<T>`. It only provides the event bus and RPC client as shared infrastructure.

## 3. Reducer Trait

### 3.1 Trait Definition

```rust
/// A component that can reduce UiEvent into its local state.
pub trait HasReducer<T> {
    /// Apply an event to the state. Returns `true` if the event was handled.
    fn reduce(state: &mut T, event: &UiEvent) -> bool;
}
```

### 3.2 Per-Component Reducer Implementations

```rust
// --- FileTree ---
pub struct WorkspaceState {
    pub workspace: WorkspaceTreeNode,
    pub modified_files: HashSet<String>,
    pub open_files: Vec<OpenFileTab>,
    pub selected_file_tab: Option<usize>,
    pub collapsed_dirs: HashSet<String>,
}

pub struct FileTree;

impl HasReducer<WorkspaceState> for FileTree {
    fn reduce(s: &mut WorkspaceState, event: &UiEvent) -> bool {
        match event {
            // FileTree may care about agent file modifications if the server sends them
            _ => false,
        }
    }
}
```

Note: WorkspaceState mutations happen primarily via user interaction (clicking files, expanding dirs), not WS events. The reducer is mostly a no-op for workspace.

```rust
// --- ConversationView ---
pub struct ConversationState {
    pub entries: Vec<ConversationEntry>,
    pub conversation_scroll: u16,
    pub auto_scroll: bool,
}

pub struct ConversationView;

impl HasReducer<ConversationState> for ConversationView {
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
            UiEvent::ThinkingComplete => true,
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

```rust
// --- ToolsPanel ---
pub struct ToolState {
    pub calls: Vec<ToolCallEntry>,
    pub expanded: HashSet<usize>,
    pub scroll: u16,
}

pub struct ToolsPanel;

impl HasReducer<ToolState> for ToolsPanel {
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

```rust
// --- StatusBar ---
pub struct GlobalState {
    pub session_id: String,
    pub run_count: u32,
    pub iteration: u32,
    pub tool_call_count: u32,
    pub run_start: Option<Instant>,
    pub run_elapsed: Duration,
    pub is_running: bool,
    pub exiting: bool,
    pub ws_url: String,
    pub ws_connected: bool,
    pub ws_last_error: Option<String>,
    pub unsafe_mode: bool,
    pub active_tab: ActiveTab,
}

pub struct StatusBar;

impl HasReducer<GlobalState> for StatusBar {
    fn reduce(s: &mut GlobalState, event: &UiEvent) -> bool {
        match event {
            UiEvent::AgentStart { .. } => {
                s.run_count += 1;
                s.iteration = 0;
                s.tool_call_count = 0;
                s.run_start = Some(Instant::now());
                s.run_elapsed = Duration::ZERO;
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

```rust
// --- ApprovalDialog ---
pub struct ApprovalState {
    pub tool_name: Option<String>,
    pub reason: Option<String>,
    pub arguments: Option<String>,
    pub response: Option<(bool, Option<String>)>,
}

pub struct ApprovalDialog;

impl HasReducer<ApprovalState> for ApprovalDialog {
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

Other components (LogViewer, SkillsPanel, SessionDialog, FileContentView, InputArea) follow the same pattern — define their state struct + implement `HasReducer`.

## 4. Component Subscription Pattern

### 4.1 Generic Subscribe Hook

```rust
/// Subscribe to EventBus events and reduce into the given signal.
/// Unsubscribes automatically when the component unmounts.
pub fn use_event_subscription<T, C>(signal: &Signal<T>)
where
    T: Clone + Send + Sync + 'static,
    C: HasReducer<T>,
{
    let signal = signal.clone();
    use_hook(move || {
        let _sub = signal
            .with(|_| {
                // We need access to the EventBus from context
                let app_state: AppState = use_context();
                app_state.event_bus.subscribe(move |event| {
                    signal.with_mut(|state| {
                        C::reduce(state, event);
                    });
                })
            });
        // Subscription is dropped when the hook's closure scope ends
        // — but we want it to live for the component lifetime.
        // See the correct pattern below.
    });
}
```

### 4.2 Correct Pattern — Store Subscription in Component Hook

The `Subscription` guard must be kept alive for the component's lifetime. Use `use_hook` with a `Cell<Option<Subscription>>` or store it directly:

```rust
#[component]
pub fn FileTree() -> Element {
    let signal = use_signal(|| WorkspaceState {
        workspace: WorkspaceTreeNode::root("/workspace".into(), ".".into()),
        modified_files: HashSet::new(),
        open_files: Vec::new(),
        selected_file_tab: None,
        collapsed_dirs: HashSet::new(),
    });

    // Subscribe on mount, unsubscribe on unmount
    use_hook(|| {
        let signal = signal.clone();
        let app_state: AppState = use_context();
        let _subscription = app_state.event_bus.subscribe(move |event| {
            signal.with_mut(|state| {
                FileTree::reduce(state, event);
            });
        });
        // Hold subscription in the hook's returned value
        // Dioxus use_hook keeps the returned value alive until unmount
        _subscription // This doesn't work — use_hook returns ()
    });

    // Correct: use use_hook to store the subscription
    let signal_clone = signal.clone();
    let _sub: Subscription = use_hook(move || {
        let app_state: AppState = use_context();
        app_state.event_bus.subscribe(move |event| {
            signal_clone.with_mut(|state| {
                FileTree::reduce(state, event);
            });
        })
    });

    // Rest of component...
}
```

### 4.3 Extracted Helper

To avoid repeating the subscription boilerplate:

```rust
/// Create a signal and subscribe to events for the given component reducer.
/// Returns the signal. Subscription lives until component unmounts.
pub fn use_signal_with_reducer<T, C>(initial: impl FnOnce() -> T) -> Signal<T>
where
    T: Clone + Send + Sync + 'static,
    C: HasReducer<T>,
{
    let signal = use_signal(initial);
    let sig = signal.clone();
    let _sub: Subscription = use_hook(move || {
        let app_state: AppState = use_context();
        app_state.event_bus.subscribe(move |event| {
            sig.with_mut(|state| {
                C::reduce(state, event);
            });
        })
    });
    signal
}
```

Usage:

```rust
#[component]
pub fn FileTree() -> Element {
    let signal = use_signal_with_reducer::<WorkspaceState, FileTree>(|| WorkspaceState {
        workspace: WorkspaceTreeNode::root("/workspace".into(), ".".into()),
        modified_files: HashSet::new(),
        open_files: Vec::new(),
        selected_file_tab: None,
        collapsed_dirs: HashSet::new(),
    });

    let ws = signal.read();
    rsx! {
        div { class: "sidebar",
            for child in &ws.workspace.children {
                TreeNode { node: child.clone(), depth: 0, key: "{child.path}" }
            }
        }
    }
}
```

## 5. WS Binding

### 5.1 Event Loop — Publish to EventBus

```rust
#[component]
pub fn App() -> Element {
    let ws_url = derive_ws_url();
    let event_bus = use_signal(|| EventBus::new());
    let client = use_hook(|| JsonRpcClient::new(&ws_url));

    // Provide AppState (event_bus + client only)
    use_context_provider(|| AppState {
        event_bus: event_bus.with(|eb| eb.clone()),
        rpc_client: client.clone(),
    });

    // WS event loop — publish events to the bus
    let bus = event_bus.with(|eb| eb.clone());
    let client_clone = client.clone();
    use_hook(move || {
        spawn(async move {
            loop {
                match client_ev.next_event().await {
                    Some(agent_event) => {
                        if let Some(ui_event) = agent_event_to_ui(&agent_event) {
                            bus.publish(&ui_event);
                        }
                    }
                    None => {
                        // Publish a connection-lost event
                        bus.publish(&UiEvent::AgentError {
                            message: "Event stream closed".to_string(),
                        });
                        break;
                    }
                }
            }
        });
    });

    // Connection state change — publish directly
    let bus_clone = bus.clone();
    client.on_state_change(move |cs| {
        let event = match cs {
            ConnectionState::Connected => UiEvent::WsConnected,
            ConnectionState::Connecting => UiEvent::WsConnecting,
            ConnectionState::Disconnected => UiEvent::WsDisconnected,
        };
        bus_clone.publish(&event);

        // Initial workspace load on connect
        if matches!(cs, ConnectionState::Connected) {
            // ... fetch workspace tree via rpc_client ...
        }
    });

    rsx! {
        StatusBar {}
        ConversationView {}
        ToolsPanel {}
        FileTree {}
        // ...
    }
}
```

### 5.2 Connection State as UiEvent

Add connection state variants to `UiEvent`:

```rust
pub enum UiEvent {
    // ... existing variants ...

    /// WebSocket connection state changes.
    WsConnected,
    WsConnecting,
    WsDisconnected { reason: Option<String> },
}
```

Components that care about connection state (e.g. StatusBar) handle these in their reducer:

```rust
impl HasReducer<GlobalState> for StatusBar {
    fn reduce(s: &mut GlobalState, event: &UiEvent) -> bool {
        match event {
            UiEvent::WsConnected => {
                s.ws_connected = true;
                s.ws_last_error = None;
                true
            }
            UiEvent::WsDisconnected { reason } => {
                s.ws_connected = false;
                s.ws_last_error = reason.clone();
                true
            }
            // ... other events ...
        }
    }
}
```

## 6. Component Examples

### 6.1 FileTree (WorkspaceState)

```rust
#[component]
pub fn FileTree() -> Element {
    let signal = use_signal_with_reducer::<WorkspaceState, FileTree>(|| WorkspaceState {
        workspace: WorkspaceTreeNode::root("/workspace".into(), ".".into()),
        modified_files: HashSet::new(),
        open_files: Vec::new(),
        selected_file_tab: None,
        collapsed_dirs: HashSet::new(),
    });

    let ws = signal.read();
    rsx! {
        div { class: "sidebar",
            for child in &ws.workspace.children {
                TreeNode { node: child.clone(), depth: 0, key: "{child.path}" }
            }
        }
    }
}
```

### 6.2 StatusBar (GlobalState)

```rust
#[component]
pub fn StatusBar() -> Element {
    let signal = use_signal_with_reducer::<GlobalState, StatusBar>(|| GlobalState {
        session_id: "web-session".into(),
        run_count: 0,
        iteration: 0,
        tool_call_count: 0,
        run_start: None,
        run_elapsed: Duration::ZERO,
        is_running: false,
        exiting: false,
        ws_url: use_context::<AppState>().rpc_client.url().to_string(),
        ws_connected: false,
        ws_last_error: None,
        unsafe_mode: false,
        active_tab: ActiveTab::Conversation,
    });

    let g = signal.read();
    rsx! {
        div { class: "status-bar",
            span { if *g.is_running { "Running" } else { "Idle" } }
            span { if *g.ws_connected { "Connected" } else { "Disconnected" } }
            span { "Runs: {g.run_count}" }
        }
    }
}
```

### 6.3 ConversationView (ConversationState)

```rust
#[component]
pub fn ConversationView() -> Element {
    let signal = use_signal_with_reducer::<ConversationState, ConversationView>(|| ConversationState {
        entries: Vec::new(),
        conversation_scroll: 0,
        auto_scroll: true,
    });

    let entries = signal.read().entries.clone();
    rsx! {
        div { class: "conversation",
            for entry in &entries {
                render_entry(entry)
            }
        }
    }
}
```

## 7. Initialization Flow

```
App component mounts
    │
    ├─ use_signal(|| EventBus::new())
    ├─ use_hook(|| JsonRpcClient::new(url))
    ├─ use_context_provider(|| AppState { event_bus, rpc_client })
    │
    ├─ WS event loop spawns
    │   └─ loop: next_event() → agent_event_to_ui() → bus.publish()
    │
    ├─ on_state_change callback registered
    │   └─ publishes WsConnected/WsDisconnected events
    │
    └─ Child components mount
        ├─ FileTree: use_signal_with_reducer → subscribe to bus
        ├─ ConversationView: use_signal_with_reducer → subscribe to bus
        ├─ StatusBar: use_signal_with_reducer → subscribe to bus
        ├─ ToolsPanel: use_signal_with_reducer → subscribe to bus
        └─ ... each component independently subscribes
```

## 8. File Impact

| File | Change |
|------|--------|
| `src/state/mod.rs` | Add `EventBus`, `Subscription`, `HasReducer<T>` trait. Add per-component state structs. Keep `UiEvent` enum unchanged. Keep `UiState` for TUI behind `#[cfg(feature = "tui")]`. Remove old `UiState::apply()` method. |
| `src/web/components/app.rs` | Create `EventBus`, provide via context. WS event loop calls `bus.publish()`. Remove `AppState` signal fields. `AppState` = `EventBus` + `JsonRpcClient` only. Add `WsConnected`/`WsDisconnected` to UiEvent. |
| `src/web/components/file_tree.rs` | Use `use_signal_with_reducer::<WorkspaceState, FileTree>`. Implement `HasReducer`. |
| `src/web/components/conversation.rs` | Use `use_signal_with_reducer::<ConversationState, ConversationView>`. Implement `HasReducer`. |
| `src/web/components/tools_panel.rs` | Use `use_signal_with_reducer::<ToolState, ToolsPanel>`. Implement `HasReducer`. |
| `src/web/components/tools_tab.rs` | Read `Signal<ToolState>` from local signal or parent. |
| `src/web/components/log_viewer.rs` | Use `use_signal_with_reducer::<LogState, LogViewer>`. Implement `HasReducer`. |
| `src/web/components/skills.rs` | Use `use_signal_with_reducer::<SkillState, SkillsPanel>`. Implement `HasReducer`. |
| `src/web/components/approval_dialog.rs` | Use `use_signal_with_reducer::<ApprovalState, ApprovalDialog>`. Implement `HasReducer`. |
| `src/web/components/session_dialog.rs` | Use `use_signal_with_reducer::<SessionState, SessionDialog>`. Implement `HasReducer`. |
| `src/web/components/status_bar.rs` | Use `use_signal_with_reducer::<GlobalState, StatusBar>`. Implement `HasReducer`. |
| `src/web/components/input_area.rs` | Use local signal (no reducer needed — user-driven mutations). |
| `src/web/components/file_content.rs` | Use local signal (user-driven — file open/close). |

## 9. Error Handling

- If a reducer returns `false` (unhandled), the event is silently ignored for that component — each component only handles what it cares about
- EventBus `publish()` iterates all subscribers synchronously — no race conditions
- If a subscriber panics, it does NOT block other subscribers (use `catch_unwind` if needed)
- Subscription `Drop` guarantees cleanup on component unmount
- WS disconnection publishes `WsDisconnected` event — components handle via their reducer

## 10. Backward Compatibility

- `UiEvent` enum stays the same — it's the wire format from WS
- `UiState` kept for TUI behind `#[cfg(feature = "tui")]` with existing `apply()` method
- Existing tests for `UiEvent` serialization/deserialization remain valid
- TUI doesn't use EventBus — it uses the existing `Arc<RwLock<UiState>>` pattern

## 11. TUI Compatibility

The TUI re-renders the entire frame at 30fps and doesn't benefit from signal splitting.

```rust
#[cfg(feature = "tui")]
pub struct UiState {
    // Same fields as current UiState — flat struct
}

#[cfg(all(feature = "web", not(feature = "tui")))]
pub use crate::state::pubsub::*; // EventBus, HasReducer, per-component state structs
```

## 12. Verification

- `cargo test --package vol-llm-ui` — all existing tests pass
- `cargo check -p vol-llm-ui --features web --bin vol-llm-ui-web` — WASM build
- `cargo check -p vol-llm-ui --features tui` — TUI build
- `cargo check -p vol-llm-ui --all-features` — both features
