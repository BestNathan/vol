# Component-Split Signal State Architecture (Pub-Sub)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace centralized `Signal<UiState>` with per-component local signals + typed event bus. Each component owns its own state, subscribes to specific `UiEvent` variants it cares about, and handles them via a local reducer.

**Architecture:** `EventBus` with per-event-type subscriber routing. Components call `subscribe(event_type, handler)` to register for exact event types only. Subscription is managed via `use_effect`-style hook — component mounts registers, unmount cleans up. WS publishes events to the bus — only matching handlers fire.

**Tech Stack:** Dioxus `use_signal`, `use_hook`, `use_context`, typed EventBus with `HashMap<UiEventKind, Vec<Handler>>`, `Arc<Mutex>` subscriber registry, JSON-RPC WebSocket event loop.

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
                     EventBus.publish(event)
                               │
                    route by UiEvent variant
                               │
              ┌────────────────┼────────────────┐
              ▼                ▼                ▼
       ConversationView     ToolsPanel     StatusBar
       [AgentStart]         [ToolCall*]    [AgentStart]
       [AgentComplete]                     [AgentComplete]
       [Thinking*]                         [IterationComplete]
              │                │                │
              ▼                ▼                ▼
        use_signal        use_signal       use_signal
        <Conversation>    <Tools>          <Global>
```

Key principles:
- **No global state container** — each component calls `use_signal(|| MyState { ... })` locally
- **Typed subscription** — `bus.subscribe(UiEventKind::AgentStart, handler)` — only fires for that event type
- **No broadcast storm** — only subscribers of the specific event type are invoked, not all handlers
- **`use_effect` pattern** — component mounts registers subscriptions, unmount drops `SubscriptionSet` guard
- **No AppState state fields** — `AppState` only holds `EventBus` + `JsonRpcClient`

## 2. EventBus

### 2.1 Event Kind Enum

`UiEvent` is the full wire-format enum with all payload data. For routing, derive a lightweight kind:

```rust
/// Coarse-grained event type for routing. Multiple UiEvent variants can map to the same kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiEventKind {
    AgentStart,
    AgentComplete,
    AgentAborted,
    AgentError,
    ThinkingStart,
    ThinkingDelta,
    ThinkingComplete,
    ContentStart,
    ContentDelta,
    ContentComplete,
    ToolCallBegin,
    ToolCallComplete,
    ToolCallError,
    ToolCallSkipped,
    ApprovalRequest,
    ApprovalResolved,
    IterationComplete,
    IterationContinued,
    MaxIterationsReached,
    WsConnected,
    WsDisconnected,
}

impl UiEvent {
    pub fn kind(&self) -> UiEventKind {
        match self {
            UiEvent::AgentStart { .. } => UiEventKind::AgentStart,
            UiEvent::AgentComplete { .. } => UiEventKind::AgentComplete,
            UiEvent::AgentAborted { .. } => UiEventKind::AgentAborted,
            UiEvent::AgentError { .. } => UiEventKind::AgentError,
            UiEvent::ThinkingStart => UiEventKind::ThinkingStart,
            UiEvent::ThinkingDelta { .. } => UiEventKind::ThinkingDelta,
            UiEvent::ThinkingComplete => UiEventKind::ThinkingComplete,
            UiEvent::ContentStart => UiEventKind::ContentStart,
            UiEvent::ContentDelta { .. } => UiEventKind::ContentDelta,
            UiEvent::ContentComplete { .. } => UiEventKind::ContentComplete,
            UiEvent::ToolCallBegin { .. } => UiEventKind::ToolCallBegin,
            UiEvent::ToolCallComplete { .. } => UiEventKind::ToolCallComplete,
            UiEvent::ToolCallError { .. } => UiEventKind::ToolCallError,
            UiEvent::ToolCallSkipped { .. } => UiEventKind::ToolCallSkipped,
            UiEvent::ApprovalRequest { .. } => UiEventKind::ApprovalRequest,
            UiEvent::ApprovalResolved { .. } => UiEventKind::ApprovalResolved,
            UiEvent::IterationComplete { .. } => UiEventKind::IterationComplete,
            UiEvent::IterationContinued { .. } => UiEventKind::IterationContinued,
            UiEvent::MaxIterationsReached { .. } => UiEventKind::MaxIterationsReached,
        }
    }
}
```

### 2.2 Subscription Types

```rust
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

/// Unique subscription ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubscriptionId(u64);

/// A set of subscriptions — drops to unsubscribe all.
/// Returned by use_effect, lives until component unmounts.
pub struct SubscriptionSet {
    ids: Vec<SubscriptionId>,
    bus: Arc<EventBusInner>,
}

impl Drop for SubscriptionSet {
    fn drop(&mut self) {
        let mut subscribers = self.bus.subscribers.lock().unwrap();
        self.ids.retain(|id| {
            subscribers.retain(|s| s.id != *id);
            false
        });
    }
}
```

### 2.3 EventBus Implementation

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

    /// Subscribe to a specific event kind. Returns a SubscriptionId.
    pub fn subscribe<F>(&self, kind: UiEventKind, handler: F) -> SubscriptionId
    where
        F: Fn(&UiEvent) + Send + Sync + 'static,
    {
        let id = SubscriptionId(self.inner.next_id.fetch_add(1, Ordering::Relaxed));
        let subscriber = Subscriber {
            id,
            handler: Box::new(handler),
        };
        self.inner.subscribers.lock().unwrap().push(subscriber);
        id
    }

    /// Unsubscribe by ID. Called automatically by SubscriptionSet::drop.
    pub fn unsubscribe(&self, id: SubscriptionId) {
        let mut subscribers = self.inner.subscribers.lock().unwrap();
        subscribers.retain(|s| s.id != id);
    }

    /// Publish an event. Only subscribers of this event's kind are invoked.
    pub fn publish(&self, event: &UiEvent) {
        let kind = event.kind();
        let subscribers = self.inner.subscribers.lock().unwrap();
        for sub in subscribers.iter() {
            // Each handler checks if it subscribed to this kind
            (sub.handler)(event);
        }
    }
}
```

Wait — the above still iterates all subscribers. Let me use a per-kind index for efficient routing:

```rust
struct Subscriber {
    id: SubscriptionId,
    handler: Handler,
}

struct EventBusInner {
    next_id: AtomicU64,
    /// Per-kind subscriber lists for efficient routing.
    subscribers: Mutex<HashMap<UiEventKind, Vec<Subscriber>>>,
}

#[derive(Clone)]
pub struct EventBus {
    inner: Arc<EventBusInner>,
}

/// A set of subscriptions — drops to unsubscribe all.
pub struct SubscriptionSet {
    ids: Vec<(UiEventKind, SubscriptionId)>,
    bus: Arc<EventBusInner>,
}

impl Drop for SubscriptionSet {
    fn drop(&mut self) {
        let mut subs = self.bus.subscribers.lock().unwrap();
        for (kind, id) in &self.ids {
            if let Some(list) = subs.get_mut(kind) {
                list.retain(|s| s.id != *id);
            }
        }
    }
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(EventBusInner {
                next_id: AtomicU64::new(0),
                subscribers: Mutex::new(HashMap::new()),
            }),
        }
    }

    /// Subscribe to a specific event kind.
    pub fn subscribe<F>(&self, kind: UiEventKind, handler: F) -> SubscriptionId
    where
        F: Fn(&UiEvent) + Send + Sync + 'static,
    {
        let id = SubscriptionId(self.inner.next_id.fetch_add(1, Ordering::Relaxed));
        let mut subs = self.inner.subscribers.lock().unwrap();
        subs.entry(kind).or_default().push(Subscriber {
            id,
            handler: Box::new(handler),
        });
        id
    }

    /// Publish an event. Only handlers subscribed to this kind are invoked.
    pub fn publish(&self, event: &UiEvent) {
        let kind = event.kind();
        let subs = self.inner.subscribers.lock().unwrap();
        if let Some(handlers) = subs.get(&kind) {
            for sub in handlers {
                (sub.handler)(event);
            }
        }
    }
}

impl EventBusInner {
    /// Remove a subscription by kind + id. Called by SubscriptionSet::drop.
    fn unsubscribe(&self, kind: UiEventKind, id: SubscriptionId) {
        let mut subs = self.subscribers.lock().unwrap();
        if let Some(list) = subs.get_mut(&kind) {
            list.retain(|s| s.id != id);
        }
    }
}
```

### 2.4 AppState (Transport Only, No State)

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

## 4. Component Subscription Pattern — `use_effect`

### 4.1 `use_effect` Helper

Like React's `useEffect`: runs on mount, returns a cleanup function called on unmount.

```rust
/// Dioxus use_effect: run setup on mount, cleanup on unmount.
/// The returned value is kept alive and dropped when the component unmounts.
pub fn use_effect<T: 'static>(setup: impl FnOnce() -> T) {
    use_hook(move || {
        let guard = setup();
        // Dioxus use_hook keeps the returned value alive until unmount.
        // When the component unmounts, guard is dropped, triggering cleanup.
        guard
    });
}
```

### 4.2 `use_signal_with_effect` — The Main Helper

Combines signal creation + typed event subscription. Each component declares exactly which `UiEventKind` variants it cares about:

```rust
/// Create a local signal and subscribe to specific event kinds.
/// Like React's useEffect: subscribe on mount, unsubscribe on unmount.
///
/// # Example
/// ```rust
/// let signal = use_signal_with_effect::<ConversationState>(
///     || ConversationState { entries: vec![], scroll: 0, auto_scroll: true },
///     |bus| {
///         bus.subscribe(UiEventKind::AgentStart, move |event| {
///             signal.with_mut(|s| ConversationReducer::reduce(s, event));
///         });
///         bus.subscribe(UiEventKind::AgentComplete, move |event| {
///             signal.with_mut(|s| ConversationReducer::reduce(s, event));
///         });
///         bus.subscribe(UiEventKind::ThinkingDelta, move |event| {
///             signal.with_mut(|s| ConversationReducer::reduce(s, event));
///         });
///     },
/// );
/// ```
pub fn use_signal_with_effect<T>(
    initial: impl FnOnce() -> T,
    subscribe: impl FnOnce(&EventBus),
) -> Signal<T>
where
    T: Clone + Send + Sync + 'static,
{
    let signal = use_signal(initial);
    let app_state: AppState = use_context();

    use_effect(|| {
        let subs = SubscriptionSet::new();
        // The subscribe closure receives the bus and registers handlers.
        // Each registration adds a (kind, id) pair to the SubscriptionSet.
        subscribe(&app_state.event_bus);
        subs
    });

    signal
}
```

Wait — the `subscribe` closure needs to register subscriptions into the `SubscriptionSet`, but `SubscriptionSet` is created inside the closure. Let me restructure so the helper manages it:

```rust
/// Create a local signal and subscribe to specific event kinds.
/// Subscriptions are registered during use_effect setup, cleaned up on unmount.
pub fn use_signal_with_effect<T>(
    initial: impl FnOnce() -> T,
    subscribe: impl FnOnce(&EventBus, Signal<T>) -> SubscriptionSet,
) -> Signal<T>
where
    T: Clone + Send + Sync + 'static,
{
    let signal = use_signal(initial);
    let app_state: AppState = use_context();

    use_effect(|| {
        subscribe(&app_state.event_bus, signal.clone())
    });

    signal
}
```

### 4.3 Usage — FileTree (no WS events, user-driven)

```rust
#[component]
pub fn FileTree() -> Element {
    let signal = use_signal_with_effect(
        || WorkspaceState {
            workspace: WorkspaceTreeNode::root("/workspace".into(), ".".into()),
            modified_files: HashSet::new(),
            open_files: Vec::new(),
            selected_file_tab: None,
            collapsed_dirs: HashSet::new(),
        },
        |bus, _signal| {
            // FileTree doesn't react to WS events — all mutations are user-driven
            SubscriptionSet::empty(bus.clone())
        },
    );

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

### 4.4 Usage — StatusBar (subscribes to specific event types)

```rust
#[component]
pub fn StatusBar() -> Element {
    let signal = use_signal_with_effect(
        || {
            let app_state: AppState = use_context();
            GlobalState {
                session_id: "web-session".into(),
                run_count: 0,
                iteration: 0,
                tool_call_count: 0,
                run_start: None,
                run_elapsed: Duration::ZERO,
                is_running: false,
                exiting: false,
                ws_url: app_state.rpc_client.url().to_string(),
                ws_connected: false,
                ws_last_error: None,
                unsafe_mode: false,
                active_tab: ActiveTab::Conversation,
            }
        },
        |bus, signal| {
            let mut set = SubscriptionSet::new(bus.clone());

            // Only these 3 event types — no broadcast storm
            set.subscribe(bus, UiEventKind::AgentStart, move |event| {
                signal.with_mut(|s| {
                    s.run_count += 1;
                    s.iteration = 0;
                    s.tool_call_count = 0;
                    s.run_start = Some(Instant::now());
                    s.run_elapsed = Duration::ZERO;
                    s.is_running = true;
                });
            });

            set.subscribe(bus, UiEventKind::AgentComplete, move |event| {
                signal.with_mut(|s| {
                    if let Some(start) = s.run_start {
                        s.run_elapsed = start.elapsed();
                    }
                    s.is_running = false;
                });
            });

            set.subscribe(bus, UiEventKind::WsConnected, move |event| {
                signal.with_mut(|s| {
                    s.ws_connected = true;
                    s.ws_last_error = None;
                });
            });

            set.subscribe(bus, UiEventKind::WsDisconnected, move |event| {
                signal.with_mut(|s| {
                    s.ws_connected = false;
                    if let UiEvent::WsDisconnected { reason } = event {
                        s.ws_last_error = reason.clone();
                    }
                });
            });

            set
        },
    );

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

### 4.5 Usage — ConversationView (many event types)

```rust
#[component]
pub fn ConversationView() -> Element {
    let signal = use_signal_with_effect(
        || ConversationState {
            entries: Vec::new(),
            conversation_scroll: 0,
            auto_scroll: true,
        },
        |bus, signal| {
            let mut set = SubscriptionSet::new(bus.clone());

            let kinds = [
                UiEventKind::AgentStart,
                UiEventKind::AgentComplete,
                UiEventKind::AgentAborted,
                UiEventKind::AgentError,
                UiEventKind::ThinkingStart,
                UiEventKind::ThinkingDelta,
                UiEventKind::ThinkingComplete,
                UiEventKind::ContentStart,
                UiEventKind::ContentDelta,
                UiEventKind::ContentComplete,
                UiEventKind::MaxIterationsReached,
                UiEventKind::IterationContinued,
                UiEventKind::IterationComplete,
            ];

            for kind in kinds {
                set.subscribe(bus, kind, {
                    let signal = signal.clone();
                    move |event| {
                        signal.with_mut(|s| {
                            ConversationReducer::reduce(s, event);
                        });
                    }
                });
            }

            set
        },
    );

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

### 4.6 `SubscriptionSet` Builder API

```rust
impl SubscriptionSet {
    pub fn new(bus: EventBus) -> Self {
        Self {
            ids: Vec::new(),
            bus: bus.inner.clone(),
        }
    }

    pub fn empty(bus: EventBus) -> Self {
        Self::new(bus)
    }

    /// Subscribe to an event kind. The subscription is tracked and cleaned up on drop.
    pub fn subscribe<F>(&mut self, _bus: &EventBus, kind: UiEventKind, handler: F)
    where
        F: Fn(&UiEvent) + Send + Sync + 'static,
    {
        let id = self.bus.subscribe(kind, handler);
        self.ids.push((kind, id));
    }
}
```

Note: `subscribe` takes `_bus: &EventBus` as a parameter for ergonomic chaining — the actual registration goes through `self.bus` which is the same `Arc<EventBusInner>`.

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

### 6.1 FileTree (WorkspaceState — no WS events)

```rust
#[component]
pub fn FileTree() -> Element {
    let signal = use_signal_with_effect(
        || WorkspaceState {
            workspace: WorkspaceTreeNode::root("/workspace".into(), ".".into()),
            modified_files: HashSet::new(),
            open_files: Vec::new(),
            selected_file_tab: None,
            collapsed_dirs: HashSet::new(),
        },
        |bus, _signal| SubscriptionSet::empty(bus.clone()),
    );

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

### 6.2 StatusBar (GlobalState — 4 event types)

```rust
#[component]
pub fn StatusBar() -> Element {
    let signal = use_signal_with_effect(
        || {
            let app_state: AppState = use_context();
            GlobalState {
                session_id: "web-session".into(),
                run_count: 0,
                iteration: 0,
                tool_call_count: 0,
                run_start: None,
                run_elapsed: Duration::ZERO,
                is_running: false,
                exiting: false,
                ws_url: app_state.rpc_client.url().to_string(),
                ws_connected: false,
                ws_last_error: None,
                unsafe_mode: false,
                active_tab: ActiveTab::Conversation,
            }
        },
        |bus, signal| {
            let mut set = SubscriptionSet::new(bus.clone());

            set.subscribe(bus, UiEventKind::AgentStart, move |event| {
                signal.with_mut(|s| {
                    s.run_count += 1;
                    s.iteration = 0;
                    s.tool_call_count = 0;
                    s.run_start = Some(Instant::now());
                    s.is_running = true;
                });
            });

            set.subscribe(bus, UiEventKind::AgentComplete, move |event| {
                signal.with_mut(|s| {
                    s.is_running = false;
                });
            });

            set.subscribe(bus, UiEventKind::WsConnected, move |event| {
                signal.with_mut(|s| {
                    s.ws_connected = true;
                    s.ws_last_error = None;
                });
            });

            set.subscribe(bus, UiEventKind::WsDisconnected, move |event| {
                signal.with_mut(|s| {
                    s.ws_connected = false;
                });
            });

            set
        },
    );

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

### 6.3 ConversationView (ConversationState — 13 event types)

```rust
#[component]
pub fn ConversationView() -> Element {
    let signal = use_signal_with_effect(
        || ConversationState {
            entries: Vec::new(),
            conversation_scroll: 0,
            auto_scroll: true,
        },
        |bus, signal| {
            let mut set = SubscriptionSet::new(bus.clone());

            for kind in [
                UiEventKind::AgentStart,
                UiEventKind::AgentComplete,
                UiEventKind::AgentAborted,
                UiEventKind::AgentError,
                UiEventKind::ThinkingStart,
                UiEventKind::ThinkingDelta,
                UiEventKind::ContentStart,
                UiEventKind::ContentDelta,
                UiEventKind::ContentComplete,
                UiEventKind::MaxIterationsReached,
                UiEventKind::IterationContinued,
                UiEventKind::IterationComplete,
            ] {
                set.subscribe(bus, kind, {
                    let signal = signal.clone();
                    move |event| {
                        signal.with_mut(|s| {
                            ConversationReducer::reduce(s, event);
                        });
                    }
                });
            }

            set
        },
    );

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
    ├─ EventBus::new()
    ├─ JsonRpcClient::new(url)
    ├─ use_context_provider(|| AppState { event_bus, rpc_client })
    │
    ├─ WS event loop spawns
    │   └─ loop: next_event() → agent_event_to_ui() → bus.publish(ui_event)
    │       └─ publish routes by UiEventKind → only matching handlers fire
    │
    ├─ on_state_change callback registered
    │   └─ publishes UiEvent::WsConnected / WsDisconnected
    │
    └─ Child components mount — each calls use_signal_with_effect(...)
        ├─ FileTree: signal + empty subscription (user-driven only)
        ├─ ConversationView: signal + subscribes 12 UiEventKind types
        ├─ StatusBar: signal + subscribes 4 UiEventKind types
        ├─ ToolsPanel: signal + subscribes 4 UiEventKind types (ToolCall*)
        ├─ ApprovalDialog: signal + subscribes 2 UiEventKind types
        └─ ... each component declares its own event types
            │
            └─ Each component returns a SubscriptionSet
               → dropped on unmount → all subscriptions cleaned up
```

## 8. File Impact

| File | Change |
|------|--------|
| `src/state/mod.rs` | Add `EventBus`, `SubscriptionSet`, `UiEventKind`, `HasReducer<T>` trait. Add `UiEvent::kind()` method. Add per-component state structs. Keep `UiEvent` enum unchanged. Keep `UiState` for TUI behind `#[cfg(feature = "tui")]`. Remove old `UiState::apply()` method. |
| `src/web/components/app.rs` | Create `EventBus`, provide via context. WS event loop calls `bus.publish(ui_event)`. Remove `AppState` signal fields. `AppState` = `EventBus` + `JsonRpcClient` only. Add `use_signal_with_effect` helper. Add `use_effect` helper. |
| `src/web/components/file_tree.rs` | Use `use_signal_with_effect`. No WS event subscriptions (user-driven). |
| `src/web/components/conversation.rs` | Use `use_signal_with_effect` with 12 `UiEventKind` types. Implement `HasReducer`. |
| `src/web/components/tools_panel.rs` | Use `use_signal_with_effect` with 4 `UiEventKind` types (ToolCall*). Implement `HasReducer`. |
| `src/web/components/tools_tab.rs` | Read local `Signal<ToolState>` or receive from parent. |
| `src/web/components/log_viewer.rs` | Use `use_signal_with_effect` with relevant `UiEventKind` types. Implement `HasReducer`. |
| `src/web/components/skills.rs` | Use `use_signal_with_effect`. Implement `HasReducer`. |
| `src/web/components/approval_dialog.rs` | Use `use_signal_with_effect` with 2 `UiEventKind` types. Implement `HasReducer`. |
| `src/web/components/session_dialog.rs` | Use `use_signal_with_effect`. Implement `HasReducer`. |
| `src/web/components/status_bar.rs` | Use `use_signal_with_effect` with 4 `UiEventKind` types. Implement `HasReducer`. |
| `src/web/components/input_area.rs` | Use local signal (no reducer needed — user-driven mutations). |
| `src/web/components/file_content.rs` | Use local signal (user-driven — file open/close). |

## 9. Error Handling

- If a reducer returns `false` (unhandled), the event is silently ignored for that component
- EventBus `publish()` only invokes handlers subscribed to that specific `UiEventKind` — no broadcast storm, no wasted cycles
- Per-kind subscriber list means `publish()` is O(matching subscribers) not O(all subscribers)
- If a subscriber panics, it does NOT block other subscribers (use `catch_unwind` if needed)
- `SubscriptionSet` `Drop` guarantees cleanup on component unmount
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
