# Split Signal State Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace centralized `Signal<UiState>` with per-component local signals + typed EventBus. Each component owns its own state, subscribes to specific `UiEvent` variants via `use_effect`, and handles them via a local reducer.

**Architecture:** `EventBus` with per-`UiEventKind` subscriber routing via `HashMap`. `App()` creates shared `Signal<GlobalState>` and `Signal<ApprovalUiState>` via `use_context_provider` for cross-component reads. EventBus handlers in `App()` update these shared signals. Components like `ConversationView`, `ToolState` create their own local signals + EventBus subscriptions. WS event loop publishes events to bus; only matching handlers fire. `AppState` simplified to `EventBus` + `JsonRpcClient` + `Signal<ActiveTab>` only.

**Tech Stack:** Dioxus 0.6 (`use_signal`, `use_effect`, `use_drop`, `use_context`, `use_context_provider`), `HashMap<UiEventKind, Vec<Subscriber>>`, `Arc<Mutex>`, JSON-RPC WebSocket (web_sys), WASM compilation.

---

### Task 1: Add UiEvent WS variants, UiEventKind enum, and kind() method

**Files:**
- Modify: `crates/vol-llm-ui/src/state/mod.rs`

- [ ] **Step 1: Add WsConnected, WsConnecting, WsDisconnected to UiEvent enum**

Add these 3 variants right after `ApprovalResolved { approved: bool },` in the `UiEvent` enum:

```rust
    // WebSocket connection state
    WsConnected,
    WsConnecting,
    WsDisconnected { reason: Option<String> },
```

- [ ] **Step 2: Add UiEventKind enum**

Add right after the `UiEvent` enum closing brace:

```rust
/// Coarse-grained event type for routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiEventKind {
    AgentStart, AgentComplete, AgentAborted, AgentError,
    ThinkingStart, ThinkingDelta, ThinkingComplete,
    ContentStart, ContentDelta, ContentComplete,
    ToolCallBegin, ToolCallArgumentDelta, ToolCallComplete, ToolCallError, ToolCallSkipped,
    ApprovalRequest, ApprovalResolved,
    IterationComplete, IterationContinued, MaxIterationsReached,
    WsConnected, WsConnecting, WsDisconnected,
}
```

- [ ] **Step 3: Add UiEvent::kind() method**

```rust
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
            UiEvent::ToolCallArgumentDelta { .. } => UiEventKind::ToolCallArgumentDelta,
            UiEvent::ToolCallComplete { .. } => UiEventKind::ToolCallComplete,
            UiEvent::ToolCallError { .. } => UiEventKind::ToolCallError,
            UiEvent::ToolCallSkipped { .. } => UiEventKind::ToolCallSkipped,
            UiEvent::ApprovalRequest { .. } => UiEventKind::ApprovalRequest,
            UiEvent::ApprovalResolved { .. } => UiEventKind::ApprovalResolved,
            UiEvent::IterationComplete { .. } => UiEventKind::IterationComplete,
            UiEvent::IterationContinued { .. } => UiEventKind::IterationContinued,
            UiEvent::MaxIterationsReached { .. } => UiEventKind::MaxIterationsReached,
            UiEvent::WsConnected => UiEventKind::WsConnected,
            UiEvent::WsConnecting => UiEventKind::WsConnecting,
            UiEvent::WsDisconnected { .. } => UiEventKind::WsDisconnected,
        }
    }
}
```

- [ ] **Step 4: Add test**

In the existing `#[cfg(test)] mod tests` block, add:

```rust
    #[test]
    fn test_ui_event_kind_mapping() {
        assert_eq!(UiEvent::AgentStart { input: "hi".into() }.kind(), UiEventKind::AgentStart);
        assert_eq!(UiEvent::WsConnected.kind(), UiEventKind::WsConnected);
        assert_eq!(UiEvent::WsDisconnected { reason: None }.kind(), UiEventKind::WsDisconnected);
        assert_eq!(UiEvent::ToolCallBegin { tool_name: "x".into(), arguments: "{}".into() }.kind(), UiEventKind::ToolCallBegin);
    }
```

- [ ] **Step 5: Verify and commit**

```bash
cargo test --package vol-llm-ui -- --test-threads=1
git add crates/vol-llm-ui/src/state/mod.rs
git commit -m "feat: add UiEvent WS variants and UiEventKind routing enum"
```

---

### Task 2: Add per-component state structs to state/mod.rs

**Files:**
- Modify: `crates/vol-llm-ui/src/state/mod.rs`

All structs gated behind `#[cfg(all(feature = "web", not(feature = "tui")))]`.

- [ ] **Step 1: Add imports and GlobalState**

Add before `// === UiState ===`:

```rust
#[cfg(all(feature = "web", not(feature = "tui")))]
use web_time::Instant;

/// Local state for StatusBar — global run/session/connection info.
#[cfg(all(feature = "web", not(feature = "tui")))]
#[derive(Debug)]
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

#[cfg(all(feature = "web", not(feature = "tui")))]
impl GlobalState {
    pub fn new(ws_url: String) -> Self {
        Self {
            session_id: "web-session".into(), run_count: 0, iteration: 0,
            tool_call_count: 0, run_start: None, run_elapsed: std::time::Duration::ZERO,
            is_running: false, exiting: false, ws_url, ws_connected: false,
            ws_last_error: None, unsafe_mode: false, active_tab: ActiveTab::Conversation,
        }
    }
}
```

- [ ] **Step 2: Add ConversationState, ToolState, WorkspaceState**

```rust
#[cfg(all(feature = "web", not(feature = "tui")))]
#[derive(Debug)]
pub struct ConversationState {
    pub entries: Vec<ConversationEntry>,
    pub conversation_scroll: u16,
    pub auto_scroll: bool,
}
#[cfg(all(feature = "web", not(feature = "tui")))]
impl ConversationState {
    pub fn new() -> Self {
        Self { entries: Vec::new(), conversation_scroll: 0, auto_scroll: true }
    }
}

#[cfg(all(feature = "web", not(feature = "tui")))]
#[derive(Debug)]
pub struct ToolState {
    pub calls: Vec<ToolCallEntry>,
    pub expanded: HashSet<usize>,
    pub scroll: u16,
}
#[cfg(all(feature = "web", not(feature = "tui")))]
impl ToolState {
    pub fn new() -> Self {
        Self { calls: Vec::new(), expanded: HashSet::new(), scroll: 0 }
    }
}

#[cfg(all(feature = "web", not(feature = "tui")))]
#[derive(Debug)]
pub struct WorkspaceState {
    pub workspace: WorkspaceTreeNode,
    pub modified_files: HashSet<String>,
    pub open_files: Vec<OpenFileTab>,
    pub selected_file_tab: Option<usize>,
    pub collapsed_dirs: HashSet<String>,
}
#[cfg(all(feature = "web", not(feature = "tui")))]
impl WorkspaceState {
    pub fn new(working_dir: &str) -> Self {
        Self {
            workspace: WorkspaceTreeNode::root(working_dir.to_string(), ".".into()),
            modified_files: HashSet::new(), open_files: Vec::new(),
            selected_file_tab: None, collapsed_dirs: HashSet::new(),
        }
    }
}
```

- [ ] **Step 3: Add SkillsState, LogViewerState, SessionDialogState, ApprovalUiState**

```rust
#[cfg(all(feature = "web", not(feature = "tui")))]
#[derive(Debug)]
pub struct SkillsState { pub skills: Vec<SkillDisplayEntry> }
#[cfg(all(feature = "web", not(feature = "tui")))]
impl SkillsState { pub fn new() -> Self { Self { skills: Vec::new() } } }

#[cfg(all(feature = "web", not(feature = "tui")))]
#[derive(Debug)]
pub struct LogViewerState {
    pub selected_run: Option<String>, pub entries: Vec<LogLine>,
    pub scroll: u16, pub auto_scroll: bool, pub run_logs: Vec<LogRunSummary>,
}
#[cfg(all(feature = "web", not(feature = "tui")))]
impl LogViewerState {
    pub fn new() -> Self {
        Self { selected_run: None, entries: Vec::new(), scroll: 0, auto_scroll: true, run_logs: Vec::new() }
    }
}

#[cfg(all(feature = "web", not(feature = "tui")))]
#[derive(Debug)]
pub struct SessionDialogState {
    pub open: bool, pub sessions: Vec<SessionDialogEntry>, pub selected: usize,
}
#[cfg(all(feature = "web", not(feature = "tui")))]
impl SessionDialogState {
    pub fn new() -> Self {
        Self { open: false, sessions: Vec::new(), selected: 0 }
    }
}

#[cfg(all(feature = "web", not(feature = "tui")))]
#[derive(Debug)]
pub struct ApprovalUiState {
    pub tool_name: Option<String>, pub reason: Option<String>, pub arguments: Option<String>,
}
#[cfg(all(feature = "web", not(feature = "tui")))]
impl ApprovalUiState {
    pub fn new() -> Self {
        Self { tool_name: None, reason: None, arguments: None }
    }
    pub fn has_pending(&self) -> bool { self.tool_name.is_some() }
    pub fn clear(&mut self) {
        self.tool_name = None; self.reason = None; self.arguments = None;
    }
}
```

- [ ] **Step 4: Verify and commit**

```bash
cargo check -p vol-llm-ui --features web --bin vol-llm-ui-web
git add crates/vol-llm-ui/src/state/mod.rs
git commit -m "feat: add per-component local state structs for web feature"
```

---

### Task 3: Add EventBus, SubscriptionSet, HasReducer to state/mod.rs

**Files:**
- Modify: `crates/vol-llm-ui/src/state/mod.rs`

- [ ] **Step 1: Add imports and core types**

Add right before `// === UiState ===`:

```rust
#[cfg(all(feature = "web", not(feature = "tui")))]
use std::collections::HashMap;
#[cfg(all(feature = "web", not(feature = "tui")))]
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(all(feature = "web", not(feature = "tui")))]
use std::sync::{Arc, Mutex};

#[cfg(all(feature = "web", not(feature = "tui")))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubscriptionId(u64);

#[cfg(all(feature = "web", not(feature = "tui")))]
type EventHandler = Box<dyn Fn(&UiEvent) + Send + Sync>;

#[cfg(all(feature = "web", not(feature = "tui")))]
struct Subscriber { id: SubscriptionId, handler: EventHandler }

#[cfg(all(feature = "web", not(feature = "tui")))]
struct EventBusInner {
    next_id: AtomicU64,
    subscribers: Mutex<HashMap<UiEventKind, Vec<Subscriber>>>,
}

#[cfg(all(feature = "web", not(feature = "tui")))]
#[derive(Clone)]
pub struct EventBus { inner: Arc<EventBusInner> }

#[cfg(all(feature = "web", not(feature = "tui")))]
impl EventBus {
    pub fn new() -> Self {
        Self { inner: Arc::new(EventBusInner { next_id: AtomicU64::new(0), subscribers: Mutex::new(HashMap::new()) }) }
    }
    pub fn subscribe<F>(&self, kind: UiEventKind, handler: F) -> SubscriptionId
    where F: Fn(&UiEvent) + Send + Sync + 'static {
        let id = SubscriptionId(self.inner.next_id.fetch_add(1, Ordering::Relaxed));
        let mut subs = self.inner.subscribers.lock().unwrap();
        subs.entry(kind).or_default().push(Subscriber { id, handler: Box::new(handler) });
        id
    }
    pub fn publish(&self, event: &UiEvent) {
        let kind = event.kind();
        let subs = self.inner.subscribers.lock().unwrap();
        if let Some(handlers) = subs.get(&kind) {
            for sub in handlers { (sub.handler)(event); }
        }
    }
}
```

- [ ] **Step 2: Add SubscriptionSet and HasReducer**

```rust
#[cfg(all(feature = "web", not(feature = "tui")))]
pub struct SubscriptionSet {
    ids: Vec<(UiEventKind, SubscriptionId)>,
    bus: Arc<EventBusInner>,
}

#[cfg(all(feature = "web", not(feature = "tui")))]
impl SubscriptionSet {
    pub fn new(bus: EventBus) -> Self {
        Self { ids: Vec::new(), bus: bus.inner.clone() }
    }
    pub fn subscribe<F>(&mut self, _bus: &EventBus, kind: UiEventKind, handler: F)
    where F: Fn(&UiEvent) + Send + Sync + 'static {
        let id = self.bus.subscribe(kind, handler);
        self.ids.push((kind, id));
    }
}

#[cfg(all(feature = "web", not(feature = "tui")))]
impl Drop for SubscriptionSet {
    fn drop(&mut self) {
        let mut subs = self.bus.subscribers.lock().unwrap();
        for (kind, id) in &self.ids {
            if let Some(list) = subs.get_mut(kind) { list.retain(|s| s.id != *id); }
        }
    }
}

#[cfg(all(feature = "web", not(feature = "tui")))]
pub trait HasReducer<T> {
    fn reduce(state: &mut T, event: &UiEvent) -> bool;
}
```

- [ ] **Step 3: Add helper functions**

```rust
#[cfg(all(feature = "web", not(feature = "tui")))]
fn flush_pending_content(entries: &mut Vec<ConversationEntry>) {
    if let Some(ConversationEntry::ContentStreaming { content }) = entries.last() {
        let text = content.clone();
        if !text.is_empty() {
            let entry = entries.last_mut().unwrap();
            *entry = ConversationEntry::AgentAnswer { text };
        }
    }
}

#[cfg(all(feature = "web", not(feature = "tui")))]
fn update_tool_call_status_in_calls(
    calls: &mut Vec<ToolCallEntry>, tool_name: &str,
    status: ToolCallStatus, duration_ms: Option<u64>,
) {
    for entry in calls.iter_mut().rev() {
        if entry.tool_name == tool_name && matches!(entry.status, ToolCallStatus::Running) {
            entry.status = status.clone();
            entry.duration_ms = duration_ms;
            break;
        }
    }
}
```

- [ ] **Step 4: Verify and commit**

```bash
cargo check -p vol-llm-ui --features web --bin vol-llm-ui-web
git add crates/vol-llm-ui/src/state/mod.rs
git commit -m "feat: add EventBus, SubscriptionSet, and HasReducer trait"
```

---

### Task 4: Refactor app.rs — EventBus, shared signals, WS event loop, simplified AppState

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/app.rs`
- Modify: `crates/vol-llm-ui/src/web/client.rs` (add `url()` method + store URL in `ClientInner`)

- [ ] **Step 1: Add url field and method to JsonRpcClient**

In `crates/vol-llm-ui/src/web/client.rs`, modify `ClientInner` to add `url: String`:

```rust
struct ClientInner {
    ws: web_sys::WebSocket,
    url: String,
    state: Cell<ConnectionState>,
    event_tx: mpsc::UnboundedSender<AgentEvent>,
    pending: RefCell<HashMap<u64, ResponseCallback>>,
    on_state_change: Cell<Option<Box<dyn Fn(ConnectionState)>>>,
}
```

In `JsonRpcClient::new()`, add `url: url.to_string()` to the `ClientInner` initialization.

Add this method to `impl JsonRpcClient`:

```rust
    /// Get the WebSocket URL this client connected to.
    pub fn url(&self) -> &str {
        &self.inner.url
    }
```

- [ ] **Step 2: Rewrite app.rs imports and AppState**

```rust
//! Root App component with state management, event loop, and routing.

use dioxus::prelude::*;
use std::time::Duration;

use crate::state::{ActiveTab, ApprovalUiState, EventBus, GlobalState, UiEvent, UiEventKind};
use crate::web::client::{AgentEvent, JsonRpcClient};

use super::approval_dialog::ApprovalDialog;
use super::conversation::ConversationView;
use super::file_content::FileContentView;
use super::file_tree::FileTree;
use super::input_area::InputArea;
use super::log_viewer::LogViewer;
use super::session_dialog::SessionDialog;
use super::skills::SkillsPanel;
use super::status_bar::StatusBar;
use super::tools_tab::ToolsTabContent;

fn derive_ws_url() -> String {
    if let Some(window) = web_sys::window() {
        let location = window.location();
        if let Ok(hostname) = location.hostname() {
            return format!("ws://{}:3001/ws", hostname);
        }
    }
    "ws://localhost:3001".to_string()
}

#[derive(Clone)]
pub struct AppState {
    pub event_bus: EventBus,
    pub rpc_client: JsonRpcClient,
    pub active_tab: Signal<ActiveTab>,
}

impl PartialEq for AppState {
    fn eq(&self, _other: &Self) -> bool { true }
}
```

- [ ] **Step 3: Keep agent_event_to_ui unchanged** (already handles all variants correctly)

- [ ] **Step 4: Rewrite App() component**

```rust
#[component]
pub fn App() -> Element {
    let ws_url = derive_ws_url();
    let event_bus = use_signal(|| EventBus::new());
    let active_tab = use_signal(|| ActiveTab::Conversation);
    let global_signal = use_signal(|| GlobalState::new(ws_url.clone()));
    let approval_signal = use_signal(|| ApprovalUiState::new());

    let client = use_hook(|| {
        let c = JsonRpcClient::new(&ws_url);
        let bus = event_bus.with(|eb| eb.clone());
        let global = global_signal.clone();
        let approval = approval_signal.clone();

        // Connection state → EventBus + global signal
        let bus_conn = bus.clone();
        let global_conn = global.clone();
        c.on_state_change(move |cs| {
            let event = match cs {
                crate::web::client::ConnectionState::Connected => UiEvent::WsConnected,
                crate::web::client::ConnectionState::Connecting => UiEvent::WsConnecting,
                crate::web::client::ConnectionState::Disconnected =>
                    UiEvent::WsDisconnected { reason: Some("Disconnected".to_string()) },
            };
            bus_conn.publish(&event);
            global_conn.with_mut(|s| match cs {
                crate::web::client::ConnectionState::Connected => {
                    s.ws_connected = true; s.ws_last_error = None;
                }
                crate::web::client::ConnectionState::Connecting => { s.ws_connected = false; }
                crate::web::client::ConnectionState::Disconnected => {
                    s.ws_connected = false;
                    s.ws_last_error = Some("Disconnected".to_string());
                }
            });
        });

        // EventBus subscriptions for shared signals (global + approval)
        let bus_sub = bus.clone();
        let global_sub = global.clone();
        let approval_sub = approval.clone();
        wasm_bindgen_futures::spawn_local(async move {
            // Small delay to ensure signals are provided
            gloo_timers::future::TimeoutFuture::new(0).await;
        });

        // Use use_effect for subscriptions (runs on mount)
        c
    });

    // EventBus subscriptions for shared signals
    use_effect(move || {
        let bus = event_bus.with(|eb| eb.clone());
        let mut set = crate::state::SubscriptionSet::new(bus.clone());
        let global = global_signal.clone();
        let approval = approval_signal.clone();

        // GlobalState: agent lifecycle events
        set.subscribe(&bus, UiEventKind::AgentStart, {
            let global = global.clone();
            move |_e| {
                global.with_mut(|s| {
                    s.run_count += 1; s.iteration = 0; s.tool_call_count = 0;
                    s.run_start = Some(web_time::Instant::now());
                    s.run_elapsed = Duration::ZERO; s.is_running = true;
                });
            }
        });
        for kind in [UiEventKind::AgentComplete, UiEventKind::AgentAborted, UiEventKind::AgentError] {
            set.subscribe(&bus, kind, {
                let global = global.clone();
                move |_e| {
                    global.with_mut(|s| {
                        if let Some(start) = s.run_start { s.run_elapsed = start.elapsed(); }
                        s.is_running = false;
                    });
                }
            });
        }
        set.subscribe(&bus, UiEventKind::IterationComplete, {
            let global = global.clone();
            move |e| {
                if let UiEvent::IterationComplete { iteration, .. } = e {
                    global.with_mut(|s| s.iteration = *iteration);
                }
            }
        });

        // ApprovalUiState
        set.subscribe(&bus, UiEventKind::ApprovalRequest, {
            let approval = approval.clone();
            move |e| {
                if let UiEvent::ApprovalRequest { tool_name, reason, arguments } = e {
                    approval.with_mut(|s| {
                        s.tool_name = Some(tool_name.clone());
                        s.reason = Some(reason.clone());
                        s.arguments = Some(arguments.clone());
                    });
                }
            }
        });
        set.subscribe(&bus, UiEventKind::ApprovalResolved, {
            let approval = approval.clone();
            move |_e| { approval.with_mut(|s| s.clear()); }
        });

        Box::new(move || { drop(set); })
    });

    // WS event loop
    let bus_ev = event_bus.with(|eb| eb.clone());
    let client_ev = client.clone();
    wasm_bindgen_futures::spawn_local(async move {
        loop {
            match client_ev.next_event().await {
                Some(event) => {
                    if let Some(ui_event) = agent_event_to_ui(&event) {
                        bus_ev.publish(&ui_event);
                    }
                }
                None => {
                    log::warn!("Event stream closed");
                    bus_ev.publish(&UiEvent::AgentError { message: "Event stream closed".to_string() });
                    bus_ev.publish(&UiEvent::WsDisconnected { reason: Some("Event stream closed".to_string()) });
                    break;
                }
            }
        }
    });

    use_context_provider(|| AppState {
        event_bus: event_bus.with(|eb| eb.clone()),
        rpc_client: client.clone(),
        active_tab,
    });
    use_context_provider(|| global_signal);
    use_context_provider(|| approval_signal);

    rsx! {
        style { {GLOBAL_CSS} }
        div { class: "app-container",
            StatusBar {}
            div { class: "main-layout",
                FileTree {}
                div { class: "right-panel",
                    TabBar {}
                    TabContent {}
                    InputArea {}
                }
            }
            SessionDialog {}
            ApprovalDialog {}
        }
    }
}
```

- [ ] **Step 5: Update TabButton**

```rust
#[component]
fn TabButton(state: AppState, tab: ActiveTab, label: String) -> Element {
    let current_tab = state.active_tab.read();
    let active = *current_tab == tab;
    let tab_class = if active { "tab active" } else { "tab" };
    let mut active_tab_signal = state.active_tab;
    rsx! {
        button {
            class: tab_class,
            onclick: move |_| { active_tab_signal.set(tab); },
            "{label}"
        }
    }
}
```

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-ui/src/web/client.rs crates/vol-llm-ui/src/web/components/app.rs
git commit -m "refactor: App creates EventBus + shared signals, publishes WS events to bus"
```

---

### Task 5: Refactor StatusBar — reads shared GlobalState via use_context

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/status_bar.rs`

StatusBar reads from the shared `Signal<GlobalState>` provided by App(). No local subscriptions needed.

- [ ] **Step 1: Rewrite StatusBar**

Replace entire file:

```rust
//! Status bar showing connection status, build info, and session details.

use dioxus::prelude::*;
use std::time::Duration;

use crate::state::GlobalState;

const BUILD_TIME: &str = env!("BUILD_TIME");

fn format_elapsed(d: std::time::Duration) -> String {
    let secs = d.as_secs();
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

#[component]
pub fn StatusBar() -> Element {
    let g: Signal<GlobalState> = use_context();
    let gs = g.read();

    let elapsed = if gs.is_running {
        gs.run_start.map(|s: web_time::Instant| s.elapsed()).unwrap_or_default()
    } else {
        gs.run_elapsed
    };
    let time_str = format_elapsed(elapsed);
    let status = if gs.is_running { "Running" } else { "Idle" };
    let badge_cls = if gs.is_running { "status-badge badge-running" } else { "status-badge badge-idle" };
    let session_id = gs.session_id.clone();
    let run_count = gs.run_count;
    let iteration = gs.iteration;
    let tool_call_count = gs.tool_call_count;
    let is_running = gs.is_running;
    let is_exiting = gs.exiting;
    let unsafe_mode = gs.unsafe_mode;
    let ws_connected = gs.ws_connected;
    let ws_error = gs.ws_last_error.clone();
    drop(gs);

    let status_class = if is_running { "status-bar status-running" } else { "status-bar status-idle" };

    rsx! {
        div { class: status_class,
            div { class: "status-left",
                ConnectionIndicator { connected: ws_connected, error: ws_error.clone() }
                span { class: "status-item", "Session: {session_id}" }
                span { class: "status-divider" }
                span { class: "status-item", "Run: {run_count}" }
                span { class: "status-divider" }
                span { class: "status-item", "Iter: {iteration}" }
                span { class: "status-divider" }
                span { class: "status-item", "Tools: {tool_call_count}" }
                span { class: "status-divider" }
                span { class: "status-item", "Time: {time_str}" }
                span { class: "status-divider" }
                span { class: badge_cls, "{status}" }
                if unsafe_mode { span { class: "status-badge badge-unsafe", "!! UNSAFE" } }
                if is_exiting { span { class: "status-badge badge-exiting", "QUITTING" } }
            }
            div { class: "status-right",
                span { class: "build-info",
                    span { class: "build-label", "UI " }
                    span { class: "build-version", {env!("CARGO_PKG_VERSION")} }
                    span { class: "build-separator", " | " }
                    span { class: "build-time", {BUILD_TIME} }
                }
            }
        }
    }
}

#[component]
fn ConnectionIndicator(connected: bool, error: Option<String>) -> Element {
    if connected {
        rsx! {
            span { class: "conn-indicator", title: "Connected",
                span { class: "conn-dot conn-dot-connected", style: "background-color: #40c040;" }
                span { class: "conn-label", "Connected" }
            }
        }
    } else if let Some(ref err) = error {
        rsx! {
            span { class: "conn-indicator", title: "{err}",
                span { class: "conn-dot conn-dot-error", style: "background-color: #ff4040;" }
                span { class: "conn-label", "Error" }
            }
        }
    } else {
        rsx! {
            span { class: "conn-indicator", title: "Connecting...",
                span { class: "conn-dot conn-dot-connecting", style: "background-color: #f0c040;" }
                span { class: "conn-label", "Connecting" }
            }
        }
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/status_bar.rs
git commit -m "refactor: StatusBar reads shared GlobalState via use_context"
```

---

### Task 6: Refactor ConversationView — local signal + use_effect with 13 event types

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/conversation.rs`

- [ ] **Step 1: Rewrite ConversationView**

Replace entire file:

```rust
//! Conversation view showing all message types.

use dioxus::prelude::*;

use crate::state::{
    ConversationEntry, ConversationState, EventBus, SubscriptionSet,
    UiEvent, UiEventKind,
};
use crate::web::components::app::AppState;

fn truncate_lines(s: &str, max_lines: usize, max_chars: usize) -> String {
    let lines: Vec<&str> = s.lines().take(max_lines).collect();
    let result = lines.join("\n");
    if result.chars().count() > max_chars {
        format!("{}...", result.chars().take(max_chars.saturating_sub(3)).collect::<String>())
    } else { result }
}

fn flush_pending_content(entries: &mut Vec<ConversationEntry>) {
    if let Some(ConversationEntry::ContentStreaming { content }) = entries.last() {
        let text = content.clone();
        if !text.is_empty() {
            *entries.last_mut().unwrap() = ConversationEntry::AgentAnswer { text };
        }
    }
}

fn reduce_conversation(s: &mut ConversationState, event: &UiEvent) {
    match event {
        UiEvent::AgentStart { input } => {
            s.entries.push(ConversationEntry::UserInput { text: input.clone() });
            if s.auto_scroll { s.conversation_scroll = 0; }
        }
        UiEvent::AgentComplete { response } => {
            flush_pending_content(&mut s.entries);
            let tc = s.entries.iter().filter(|e| matches!(e, ConversationEntry::ToolCall { .. })).count() as u32;
            s.entries.push(ConversationEntry::RunSummary { iterations: 0, tool_calls: tc, elapsed_ms: 0 });
            if !response.is_empty() {
                s.entries.push(ConversationEntry::AgentAnswer { text: response.clone() });
            }
            if s.auto_scroll { s.conversation_scroll = 0; }
        }
        UiEvent::AgentAborted { reason } | UiEvent::AgentError { message: reason } => {
            flush_pending_content(&mut s.entries);
            s.entries.push(ConversationEntry::Error { message: reason.clone() });
        }
        UiEvent::ThinkingStart => {
            s.entries.push(ConversationEntry::Thinking { content: String::new() });
        }
        UiEvent::ThinkingDelta { delta } => {
            if let Some(ConversationEntry::Thinking { content }) = s.entries.last_mut() {
                content.push_str(delta);
            }
        }
        UiEvent::ContentStart => {
            s.entries.push(ConversationEntry::ContentStreaming { content: String::new() });
        }
        UiEvent::ContentDelta { delta } => {
            if let Some(ConversationEntry::ContentStreaming { content }) = s.entries.last_mut() {
                content.push_str(delta);
            }
        }
        UiEvent::ContentComplete { content } => {
            if let Some(ConversationEntry::ContentStreaming { .. }) = s.entries.last() {
                *s.entries.last_mut().unwrap() = ConversationEntry::AgentAnswer { text: content.clone() };
            } else if !content.is_empty() {
                s.entries.push(ConversationEntry::AgentAnswer { text: content.clone() });
            }
        }
        UiEvent::MaxIterationsReached { current, max } => {
            s.entries.push(ConversationEntry::Error {
                message: format!("Max iterations reached ({}/{}) — waiting for user decision...", current, max),
            });
        }
        UiEvent::IterationContinued { from_iteration } => {
            s.entries.push(ConversationEntry::AgentAnswer {
                text: format!("Continuing from iteration {from_iteration} (counter reset to 0)"),
            });
        }
        UiEvent::IterationComplete { final_answer, .. } => {
            if let Some(answer) = final_answer {
                s.entries.push(ConversationEntry::AgentAnswer { text: answer.clone() });
            }
        }
        _ => {}
    }
}

#[component]
pub fn ConversationView() -> Element {
    let app_state: AppState = use_context();
    let signal = use_signal(|| ConversationState::new());

    use_effect(move || {
        let bus = app_state.event_bus.clone();
        let mut set = SubscriptionSet::new(bus.clone());

        for kind in [
            UiEventKind::AgentStart, UiEventKind::AgentComplete, UiEventKind::AgentAborted,
            UiEventKind::AgentError, UiEventKind::ThinkingStart, UiEventKind::ThinkingDelta,
            UiEventKind::ThinkingComplete, UiEventKind::ContentStart, UiEventKind::ContentDelta,
            UiEventKind::ContentComplete, UiEventKind::MaxIterationsReached,
            UiEventKind::IterationContinued, UiEventKind::IterationComplete,
        ] {
            set.subscribe(&bus, kind, {
                let signal = signal.clone();
                move |event| {
                    signal.with_mut(|s| { reduce_conversation(s, event); });
                }
            });
        }

        Box::new(move || { drop(set); })
    });

    let count = signal.read().entries.len();
    if count == 0 {
        return rsx! {
            div { class: "conversation",
                div { class: "conversation-empty", "No messages yet. Type a query and press Send." }
            }
        };
    }

    let entries = signal.read().entries.clone();
    let messages: Vec<Element> = (0..count).map(|index| {
        let entry = entries[index].clone();
        rsx! { MessageEntry { entry } }
    }).collect();
    rsx! {
        div { class: "conversation", {messages.into_iter()} }
    }
}

#[component]
fn MessageEntry(entry: ConversationEntry) -> Element {
    match entry {
        ConversationEntry::UserInput { text } => {
            rsx! { div { class: "msg msg-user", div { class: "msg-user-prefix", ">>> " } {text} } }
        }
        ConversationEntry::Thinking { content } => {
            rsx! { div { class: "msg msg-thinking", div { class: "msg-thinking-prefix", "Thinking" } div { class: "msg-thinking-content", {content} } } }
        }
        ConversationEntry::ContentStreaming { content } => {
            if content.is_empty() { rsx! { div { class: "msg msg-streaming", "Generating..." } } }
            else { rsx! { div { class: "msg msg-streaming", {content} } } }
        }
        ConversationEntry::ToolCall { tool_name, arg_preview } => {
            rsx! { div { class: "msg msg-tool", div { class: "msg-tool-name", "[{tool_name}]" } if !arg_preview.is_empty() { div { class: "msg-tool-arg", "{arg_preview}" } } } }
        }
        ConversationEntry::ToolResult { tool_name, preview, success } => {
            let cls = if success { "msg-tool-result" } else { "msg-tool-result-error" };
            let status = if success { "OK" } else { "ERR" };
            let color = if success { "#40c040" } else { "#c04040" };
            let display = truncate_lines(&preview, 6, 90);
            rsx! { div { class: "msg {cls}", div { span { class: "msg-tool-result-prefix", style: "color: {color};", "[{status}] " } span { style: "color: {color}; font-weight: bold;", "{tool_name}" } } div { class: "msg-tool-result-content", {display} } } }
        }
        ConversationEntry::AgentAnswer { text } => { rsx! { div { class: "msg msg-answer", {text} } } }
        ConversationEntry::RunSummary { iterations, tool_calls, elapsed_ms } => {
            let iw = if iterations == 1 { "iteration" } else { "iterations" };
            let tw = if tool_calls == 1 { "tool call" } else { "tool calls" };
            rsx! { div { class: "msg msg-summary", "Done | {iterations} {iw} | {tool_calls} {tw} | {elapsed_ms}ms" } }
        }
        ConversationEntry::Error { message } => { rsx! { div { class: "msg msg-error", "Error: {message}" } } }
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/conversation.rs
git commit -m "refactor: ConversationView uses local ConversationState signal + EventBus subscriptions"
```

---

### Task 7: Refactor ToolsPanel and ToolsTabContent — local ToolState signals

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/tools_panel.rs`
- Modify: `crates/vol-llm-ui/src/web/components/tools_tab.rs`

- [ ] **Step 1: Rewrite tools_panel.rs**

```rust
//! Left panel showing tool calls with status indicators.

use dioxus::prelude::*;
use crate::state::{ToolState, ToolCallEntry, ToolCallStatus, UiEvent, UiEventKind, EventBus, SubscriptionSet};
use crate::web::components::app::AppState;

fn update_status(calls: &mut Vec<ToolCallEntry>, name: &str, status: ToolCallStatus, dur: Option<u64>) {
    for e in calls.iter_mut().rev() {
        if e.tool_name == name && matches!(e.status, ToolCallStatus::Running) {
            e.status = status.clone(); e.duration_ms = dur; break;
        }
    }
}

fn arg_preview(arguments: &str) -> String {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(arguments) {
        if let Some(c) = v.get("command").and_then(|v| v.as_str()) {
            return if c.chars().count() > 80 { format!("Command: {}...", c.chars().take(77).collect::<String>()) } else { format!("Command: {}", c) };
        }
        if let Some(p) = v.get("path").and_then(|v| v.as_str()) { return format!("Path: {}", p); }
        if let Some(f) = v.get("file_path").and_then(|v| v.as_str()) { return format!("File: {}", f); }
        if arguments.chars().count() > 80 { return format!("Args: {}...", arguments.chars().take(77).collect::<String>()); }
        return format!("Args: {}", arguments);
    }
    String::new()
}

pub fn status_label(s: ToolCallStatus) -> &'static str {
    match s { ToolCallStatus::Running => "...", ToolCallStatus::Success => "OK", ToolCallStatus::Error => "ERR", ToolCallStatus::Skipped => "SKIP" }
}
pub fn status_class(s: ToolCallStatus) -> &'static str {
    match s { ToolCallStatus::Running => "status-running", ToolCallStatus::Success => "status-success", ToolCallStatus::Error => "status-error", ToolCallStatus::Skipped => "status-skipped" }
}

#[component]
pub fn ToolsPanel() -> Element {
    let app_state: AppState = use_context();
    let signal = use_signal(|| ToolState::new());

    use_effect(move || {
        let bus = app_state.event_bus.clone();
        let mut set = SubscriptionSet::new(bus.clone());
        let sig = signal.clone();

        for kind in [UiEventKind::ToolCallBegin, UiEventKind::ToolCallComplete, UiEventKind::ToolCallError, UiEventKind::ToolCallSkipped] {
            set.subscribe(&bus, kind, {
                let signal = signal.clone();
                move |event| {
                    signal.with_mut(|s| match event {
                        UiEvent::ToolCallBegin { tool_name, arguments } => {
                            let seq = s.calls.len() as u32 + 1;
                            s.calls.push(ToolCallEntry { sequence: seq, tool_name: tool_name.clone(), arg_preview: arg_preview(arguments), status: ToolCallStatus::Running, duration_ms: None });
                            s.scroll = s.calls.len() as u16;
                        }
                        UiEvent::ToolCallComplete { tool_name, duration_ms, .. } => update_status(&mut s.calls, tool_name, ToolCallStatus::Success, *duration_ms),
                        UiEvent::ToolCallError { tool_name, duration_ms, .. } => update_status(&mut s.calls, tool_name, ToolCallStatus::Error, *duration_ms),
                        UiEvent::ToolCallSkipped { tool_name, duration_ms, .. } => update_status(&mut s.calls, tool_name, ToolCallStatus::Skipped, *duration_ms),
                        _ => {}
                    });
                }
            });
        }
        Box::new(move || { drop(set); })
    });

    let count = signal.read().calls.len();
    rsx! {
        div { class: "tools-panel",
            div { class: "tools-panel-header", "Tools Called ({count})" }
            div { class: "tools-panel-list",
                if count == 0 {
                    div { style: "padding: 10px; color: #666; text-align: center;", "No tool calls yet" }
                } else {
                    {(0..count).map(|idx| { let s = signal.clone(); rsx! { ToolItem { signal: s, index: idx } } }).collect::<Vec<Element>>().into_iter()}
                }
            }
        }
    }
}

#[component]
fn ToolItem(signal: Signal<ToolState>, index: usize) -> Element {
    let (seq, name, arg, status, dur) = {
        let ui = signal.read();
        match ui.calls.get(index) {
            Some(e) => (e.sequence, e.tool_name.clone(), e.arg_preview.clone(), e.status.clone(), e.duration_ms),
            None => return rsx! {},
        }
    };
    let scls = status_class(status.clone());
    let label = status_label(status);
    let dur_s = dur.map(|ms| format!(" {}ms", ms)).unwrap_or_default();
    rsx! {
        div { class: "tool-item",
            div { span { class: "tool-item-name", "{seq}. [{name}]" } span { class: "tool-item-status {scls}", "{label}" } if !dur_s.is_empty() { span { style: "color: #888; font-size: 11px; margin-left: 6px;", "{dur_s}" } } }
            if !arg.is_empty() { div { class: "tool-item-arg", "{arg}" } }
        }
    }
}
```

- [ ] **Step 2: Rewrite tools_tab.rs**

Same pattern — local ToolState signal + same EventBus subscriptions:

```rust
//! Tools tab with expandable tool call details.

use dioxus::prelude::*;
use crate::state::{ToolState, ToolCallEntry, ToolCallStatus, UiEvent, UiEventKind, EventBus, SubscriptionSet};
use crate::web::components::app::AppState;

fn update_status(calls: &mut Vec<ToolCallEntry>, name: &str, status: ToolCallStatus, dur: Option<u64>) {
    for e in calls.iter_mut().rev() {
        if e.tool_name == name && matches!(e.status, ToolCallStatus::Running) {
            e.status = status.clone(); e.duration_ms = dur; break;
        }
    }
}
fn arg_preview(arguments: &str) -> String {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(arguments) {
        if let Some(c) = v.get("command").and_then(|v| v.as_str()) {
            return if c.chars().count() > 80 { format!("Command: {}...", c.chars().take(77).collect::<String>()) } else { format!("Command: {}", c) };
        }
        if let Some(p) = v.get("path").and_then(|v| v.as_str()) { return format!("Path: {}", p); }
        if let Some(f) = v.get("file_path").and_then(|v| v.as_str()) { return format!("File: {}", f); }
        if arguments.chars().count() > 80 { return format!("Args: {}...", arguments.chars().take(77).collect::<String>()); }
        return format!("Args: {}", arguments);
    }
    String::new()
}

#[component]
pub fn ToolsTabContent() -> Element {
    let app_state: AppState = use_context();
    let signal = use_signal(|| ToolState::new());

    use_effect(move || {
        let bus = app_state.event_bus.clone();
        let mut set = SubscriptionSet::new(bus.clone());
        for kind in [UiEventKind::ToolCallBegin, UiEventKind::ToolCallComplete, UiEventKind::ToolCallError, UiEventKind::ToolCallSkipped] {
            set.subscribe(&bus, kind, {
                let signal = signal.clone();
                move |event| {
                    signal.with_mut(|s| match event {
                        UiEvent::ToolCallBegin { tool_name, arguments } => {
                            let seq = s.calls.len() as u32 + 1;
                            s.calls.push(ToolCallEntry { sequence: seq, tool_name: tool_name.clone(), arg_preview: arg_preview(arguments), status: ToolCallStatus::Running, duration_ms: None });
                            s.scroll = s.calls.len() as u16;
                        }
                        UiEvent::ToolCallComplete { tool_name, duration_ms, .. } => update_status(&mut s.calls, tool_name, ToolCallStatus::Success, *duration_ms),
                        UiEvent::ToolCallError { tool_name, duration_ms, .. } => update_status(&mut s.calls, tool_name, ToolCallStatus::Error, *duration_ms),
                        UiEvent::ToolCallSkipped { tool_name, duration_ms, .. } => update_status(&mut s.calls, tool_name, ToolCallStatus::Skipped, *duration_ms),
                        _ => {}
                    });
                }
            });
        }
        Box::new(move || { drop(set); })
    });

    let count = signal.read().calls.len();
    if count == 0 {
        return rsx! { div { class: "tools-tab", div { class: "tools-tab-empty", "No tool calls yet" } } };
    }
    let items: Vec<Element> = (0..count).map(|idx| {
        let s = signal.clone();
        rsx! { ToolCallItem { signal: s, index: idx } }
    }).collect();
    rsx! { div { class: "tools-tab", {items.into_iter()} } }
}

#[component]
fn ToolCallItem(signal: Signal<ToolState>, index: usize) -> Element {
    let is_expanded = signal.read().expanded.contains(&index);
    let (seq, name, arg, status, dur) = {
        let ui = signal.read();
        match ui.calls.get(index) {
            Some(e) => (e.sequence, e.tool_name.clone(), e.arg_preview.clone(), e.status.clone(), e.duration_ms),
            None => return rsx! {},
        }
    };
    let scls = match status { ToolCallStatus::Running => "status-running", ToolCallStatus::Success => "status-success", ToolCallStatus::Error => "status-error", ToolCallStatus::Skipped => "status-skipped" };
    let label = match status { ToolCallStatus::Running => "...", ToolCallStatus::Success => "OK", ToolCallStatus::Error => "ERR", ToolCallStatus::Skipped => "SKIP" };
    let dur_s = dur.map(|ms| format!("{ms}ms")).unwrap_or_default();
    rsx! {
        div { class: "tool-call-item",
            div { class: "tool-call-header",
                onclick: move |_: Event<MouseData>| {
                    let mut sig = signal.clone(); let idx = index;
                    sig.with_mut(|s| { if s.expanded.contains(&idx) { s.expanded.remove(&idx); } else { s.expanded.insert(idx); } });
                },
                span { class: "tool-call-seq", "{seq}." }
                span { class: "tool-call-name", "[{name}]" }
                span { class: "tool-call-status {scls}", "{label}" }
                if !dur_s.is_empty() { span { class: "tool-call-duration", "{dur_s}" } }
                span { class: "tool-call-chevron", "▾" }
            }
            if is_expanded { div { class: "tool-call-detail", div { span { class: "tool-detail-label", "Input: " } "{arg}" } } }
        }
    }
}
```

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/tools_panel.rs crates/vol-llm-ui/src/web/components/tools_tab.rs
git commit -m "refactor: ToolsPanel/ToolsTabContent use local ToolState signals + EventBus"
```

---

### Task 8: Refactor FileTree — local WorkspaceState signal (no WS subscriptions)

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/file_tree.rs`

- [ ] **Step 1: Rewrite FileTree**

Replace entire file:

```rust
//! Left sidebar file tree with collapsible directories.

use dioxus::prelude::*;

use crate::state::{ActiveTab, OpenFileTab, WorkspaceState, WorkspaceTreeNode};
use crate::web::components::app::AppState;

pub(crate) fn file_icon(is_dir: bool, name: &str) -> &'static str {
    if is_dir { return "\u{1f4c2}"; }
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "rs" => "\u{1f980}", "toml" | "lock" => "\u{2699}\u{fe0f}", "md" => "\u{1f4dd}",
        "json" => "\u{1f4ca}", "yaml" | "yml" => "\u{1f4dc}", "sh" | "bash" => "\u{1f41a}",
        "html" | "htm" => "\u{1f310}", "css" => "\u{1f3a8}", "js" | "ts" | "jsx" | "tsx" => "\u{1f4dc}",
        "txt" => "\u{1f4c4}", _ => "\u{1f4c4}",
    }
}

#[component]
fn TreeNode(node: WorkspaceTreeNode, depth: usize) -> Element {
    if node.is_dir {
        let app_state: AppState = use_context();
        let mut sig: Signal<WorkspaceState> = use_context();
        let collapsed = sig.read().collapsed_dirs.contains(&node.path);
        let indent_px = depth * 16;
        let chevron_cls = if collapsed { "file-tree-chevron collapsed" } else { "file-tree-chevron" };

        let dir_path = node.path.clone();
        let rpc = app_state.rpc_client.clone();
        let dir_onclick = move |_: Event<MouseData>| {
            let p = dir_path.clone();
            let rpc_clone = rpc.clone();
            let mut sig = sig.clone();
            let was_collapsed = sig.with_mut(|s| {
                if s.collapsed_dirs.contains(&p) { s.collapsed_dirs.remove(&p); false }
                else { s.collapsed_dirs.insert(p.clone()); true }
            });
            if !was_collapsed {
                let p_str = p.clone();
                rpc_clone.file_list(&p_str, move |result| {
                    let mut sig2 = sig.clone();
                    match result {
                        Ok(entries) => {
                            let flat: Vec<(String, bool)> = entries.into_iter().map(|e| (e.name, e.is_dir)).collect();
                            sig2.with_mut(|s2| { s2.workspace.replace_dir_children(&p, flat); });
                        }
                        Err(_) => {
                            sig2.with_mut(|s2| {
                                if let Some(nd) = s2.workspace.find_child_mut(&p) {
                                    nd.children.clear(); nd.loaded = true; nd.load_error = true;
                                }
                            });
                        }
                    }
                });
            }
        };

        let refresh_path = node.path.clone();
        let refresh_rpc = app_state.rpc_client.clone();
        let refresh_onclick = move |e: Event<MouseData>| {
            e.stop_propagation();
            let p = refresh_path.clone();
            let rpc_clone = refresh_rpc.clone();
            let mut sig = sig.clone();
            sig.with_mut(|s| {
                if let Some(nd) = s.workspace.find_child_mut(&p) {
                    nd.children.clear(); nd.loaded = false; nd.load_error = false;
                }
            });
            let p_str = p.clone();
            rpc_clone.file_list(&p_str, move |result| {
                let mut sig2 = sig.clone();
                match result {
                    Ok(entries) => {
                        let flat: Vec<(String, bool)> = entries.into_iter().map(|e| (e.name, e.is_dir)).collect();
                        sig2.with_mut(|s2| { s2.workspace.replace_dir_children(&p, flat); });
                    }
                    Err(_) => {
                        sig2.with_mut(|s2| {
                            if let Some(nd) = s2.workspace.find_child_mut(&p) {
                                nd.children.clear(); nd.loaded = true; nd.load_error = true;
                            }
                        });
                    }
                }
            });
        };

        rsx! {
            div {
                div { class: "file-tree-node file-tree-dir", style: format!("padding-left: {}px;", indent_px), onclick: dir_onclick,
                    span { class: "{chevron_cls}", "\u{25be}" }
                    span { class: "file-tree-icon", "{file_icon(true, &node.name)}" }
                    span { class: "file-tree-label dir", "{node.name}" }
                    span { class: "file-tree-refresh", onclick: refresh_onclick, "\u{21bb}" }
                }
                if !collapsed {
                    div { class: "file-tree-children",
                        for child in &node.children { TreeNode { node: child.clone(), depth: depth + 1, key: "{child.path}" } }
                    }
                }
            }
        }
    } else {
        let app_state: AppState = use_context();
        let mut sig: Signal<WorkspaceState> = use_context();
        let indent_px = depth * 16;
        let file_path = node.path.clone();
        let rpc = app_state.rpc_client.clone();
        let file_onclick = move |_: Event<MouseData>| {
            let p = file_path.clone();
            let rpc_clone = rpc.clone();
            let mut sig = sig.clone();
            let is_new_file = sig.with_mut(|s| {
                let existing = s.open_files.iter().position(|f| f.path == p);
                match existing {
                    Some(idx) => { s.selected_file_tab = Some(idx); false }
                    None => {
                        let new_idx = s.open_files.len();
                        s.open_files.push(OpenFileTab { path: p.clone(), content: None, error: None });
                        s.selected_file_tab = Some(new_idx); true
                    }
                }
            });
            if is_new_file {
                let p2 = p.clone();
                rpc_clone.file_read(&p, move |result| {
                    sig.with_mut(|st| {
                        if let Some(idx) = st.open_files.iter().position(|f| f.path == p2) {
                            match result { Ok(c) => st.open_files[idx].content = Some(c), Err(e) => st.open_files[idx].error = Some(e) }
                        }
                    });
                });
            }
        };
        rsx! {
            div { class: "file-tree-node file-tree-file", style: format!("padding-left: {}px;", indent_px), onclick: file_onclick,
                span { class: "file-tree-chevron hidden", "\u{25be}" }
                span { class: "file-tree-icon", "{file_icon(false, &node.name)}" }
                span { class: "file-tree-label file", "{node.name}" }
            }
        }
    }
}

#[component]
pub fn FileTree() -> Element {
    let signal = use_signal(|| WorkspaceState::new("/workspace"));
    use_context_provider(move || signal);

    let workspace = signal.read().workspace.clone();
    if workspace.children.is_empty() && !workspace.loaded {
        return rsx! {
            div { class: "sidebar",
                div { class: "sidebar-header", "Explorer" }
                div { class: "file-tree", div { class: "file-tree-empty", "No files loaded" } }
            }
        };
    }
    rsx! {
        div { class: "sidebar",
            div { class: "sidebar-header", "Explorer" }
            div { class: "file-tree",
                for child in &workspace.children { TreeNode { node: child.clone(), depth: 0, key: "{child.path}" } }
            }
        }
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/file_tree.rs
git commit -m "refactor: FileTree uses local WorkspaceState signal, provides via context"
```

---

### Task 9: Refactor remaining components

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/approval_dialog.rs`
- Modify: `crates/vol-llm-ui/src/web/components/skills.rs`
- Modify: `crates/vol-llm-ui/src/web/components/session_dialog.rs`
- Modify: `crates/vol-llm-ui/src/web/components/log_viewer.rs`

- [ ] **Step 1: Rewrite approval_dialog.rs**

Reads from shared `Signal<ApprovalUiState>` via use_context:

```rust
//! HITL approval dialog for tool calls.

use dioxus::prelude::*;
use crate::state::ApprovalUiState;

#[component]
pub fn ApprovalDialog() -> Element {
    let sig: Signal<ApprovalUiState> = use_context();
    let has_pending = sig.read().has_pending();
    if !has_pending { return rsx! {}; }

    let tool_name = sig.read().tool_name.clone().unwrap_or_default();
    let reason = sig.read().reason.clone().unwrap_or_default();
    let arguments = sig.read().arguments.clone().unwrap_or_default();

    let mut sig_clear = sig;
    let on_approve = move |_: Event<MouseData>| { sig_clear.with_mut(|s| s.clear()); };
    let mut sig_reject = sig;
    let on_reject = move |_: Event<MouseData>| { sig_reject.with_mut(|s| s.clear()); };

    rsx! {
        div { class: "modal-overlay",
            div { class: "modal-content",
                div { class: "modal-title", "Tool Approval Required" }
                div { class: "approval-tool-name", "[!] {tool_name}" }
                if !reason.is_empty() { div { class: "approval-reason", "Reason: {reason}" } }
                if !arguments.is_empty() { div { class: "approval-args", "{arguments}" } }
                div { class: "modal-actions",
                    button { class: "btn-approve", onclick: on_approve, "Approve" }
                    button { class: "btn-reject", onclick: on_reject, "Reject" }
                }
            }
        }
    }
}
```

- [ ] **Step 2: Rewrite input_area.rs**

Reads from shared signals via use_context:

```rust
//! Text input for sending messages to the agent.

use dioxus::prelude::*;
use crate::state::{ApprovalUiState, GlobalState};
use crate::web::components::app::AppState;

#[component]
pub fn InputArea() -> Element {
    let app_state: AppState = use_context();
    let global: Signal<GlobalState> = use_context();
    let approval: Signal<ApprovalUiState> = use_context();
    let is_running = global.read().is_running;
    let has_approval = approval.read().has_pending();

    let mut input_text = use_signal(|| String::new());
    let client = app_state.rpc_client.clone();
    let on_submit = move |_| {
        let text = input_text.peek().clone();
        let text = text.trim().to_string();
        if text.is_empty() { return; }
        match client.submit(&text) {
            Ok(req_id) => log::info!("Submitted via JSON-RPC: {}", req_id),
            Err(e) => log::error!("Failed to submit via JSON-RPC: {}", e),
        }
        input_text.set(String::new());
    };
    let on_input = move |evt: Event<FormData>| { input_text.set(evt.value()); };

    let hint = if is_running {
        rsx! { span { class: "input-hint-running", " Running... (input disabled) " } }
    } else {
        rsx! { span { span { class: "input-hint-key", "Enter" } " Send  " span { class: "input-hint-key", "Esc" } " Clear" } }
    };

    rsx! {
        div { class: "input-area",
            if has_approval {
                div { p { class: "input-hint-running", "Tool approval pending in the dialog above." } }
            } else {
                div {
                    div { class: "input-row",
                        textarea { value: input_text(), oninput: on_input, disabled: is_running, placeholder: "Type a message to the agent...", rows: 2 }
                        button { onclick: on_submit, disabled: is_running, "Send" }
                    }
                    div { class: "input-hint", {hint} }
                }
            }
        }
    }
}
```

- [ ] **Step 3: Rewrite skills.rs**

```rust
//! Skills panel showing available skills.

use dioxus::prelude::*;
use crate::state::{SkillsState, SkillDisplayEntry};

#[component]
pub fn SkillsPanel() -> Element {
    let signal = use_signal(|| SkillsState::new());
    let count = signal.read().skills.len();
    if count == 0 {
        return rsx! { div { class: "skills-panel", div { class: "skills-empty", "No skills discovered" } } };
    }
    rsx! {
        div { class: "skills-panel",
            table { class: "skills-table",
                thead { tr { th { "Name" } th { "Version" } th { "Scope" } th { "Description" } } }
                tbody {
                    {(0..count).map(|i| { let s = signal.clone(); rsx! { SkillRow { signal: s, index: i } } }).collect::<Vec<Element>>().into_iter()}
                }
            }
        }
    }
}

#[component]
fn SkillRow(signal: Signal<SkillsState>, index: usize) -> Element {
    let skill = signal.read().skills.get(index).cloned();
    let Some(skill) = skill else { return rsx! {}; };
    let color = match skill.scope.as_str() { "User" => "#40c040", "Repo" => "#4080ff", _ => "#c0c040" };
    rsx! {
        tr {
            td { style: "color: #e0e0e0; font-weight: bold;", "{skill.name}" }
            td { style: "color: #888;", "{skill.version}" }
            td { style: "color: {color};", "{skill.scope}" }
            td { style: "color: #888;", "{skill.description}" }
        }
    }
}
```

- [ ] **Step 4: Rewrite session_dialog.rs**

```rust
//! Session management dialog (list, resume, new, delete).

use dioxus::prelude::*;
use crate::state::SessionDialogState;

fn uuid_v4_stub() -> String {
    let ts = js_sys::Date::now() as u128 * 1_000_000;
    format!("{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        (ts >> 96) as u32, (ts >> 80) as u16 & 0xffff, (ts >> 64) as u16 & 0xffff,
        ((ts >> 48) as u16 & 0x0fff) | 0x4000, ts & 0xffffffffffff)
}

#[component]
pub fn SessionDialog() -> Element {
    let signal = use_signal(|| SessionDialogState::new());
    let open = signal.read().open;
    if !open { return rsx! {}; }

    let sessions = signal.read().sessions.clone();
    let selected = signal.read().selected;

    let mut sig_new = signal;
    let on_new = move |_: Event<MouseData>| { sig_new.with_mut(|s| { s.open = false; let _ = uuid_v4_stub(); }); };
    let mut sig_resume = signal;
    let on_resume = move |_: Event<MouseData>| { sig_resume.with_mut(|s| s.open = false); };
    let mut sig_delete = signal;
    let on_delete = move |_: Event<MouseData>| {
        sig_delete.with_mut(|s| {
            let sel = s.selected;
            if s.sessions.get(sel).is_some() {
                s.sessions.remove(sel);
                if !s.sessions.is_empty() { s.selected = sel.min(s.sessions.len().saturating_sub(1)); }
            }
        });
    };

    let items: Vec<Element> = if sessions.is_empty() {
        vec![rsx! { div { class: "modal-empty", "No saved sessions found." } }]
    } else {
        sessions.iter().enumerate().map(|(i, entry)| {
            let is_sel = i == selected;
            let cls = if is_sel { "modal-session-item selected" } else { "modal-session-item" };
            let short = if entry.session_id.len() > 10 { format!("{}...", &entry.session_id[..7]) } else { entry.session_id.clone() };
            let mut sig_sel = signal;
            rsx! {
                div { class: cls, onclick: move |_: Event<MouseData>| { sig_sel.with_mut(|s| s.selected = i); },
                    span { class: "modal-session-id", "{short}" }
                    span { class: "modal-session-meta", "{entry.entry_count} entries | {entry.age_label}" }
                }
            }
        }).collect()
    };

    let mut sig_overlay = signal;
    let mut sig_cancel = signal;
    rsx! {
        div { class: "modal-overlay", onclick: move |_: Event<MouseData>| { sig_overlay.with_mut(|s| s.open = false); },
            div { class: "modal-content", onclick: |evt: Event<MouseData>| { evt.stop_propagation(); },
                div { class: "modal-title", "Sessions" }
                {items.into_iter()}
                div { class: "modal-actions",
                    button { class: "btn-new", onclick: on_new, "New" }
                    button { class: "btn-resume", onclick: on_resume, "Resume" }
                    button { class: "btn-delete", onclick: on_delete, "Delete" }
                    button { class: "btn-cancel", onclick: move |_: Event<MouseData>| { sig_cancel.with_mut(|s| s.open = false); }, "Cancel" }
                }
            }
        }
    }
}
```

- [ ] **Step 5: Rewrite log_viewer.rs**

```rust
//! Log run viewer with event details.

use dioxus::prelude::*;
use crate::state::{LogViewerState, LogLine, LogRunSummary};

#[component]
pub fn LogViewer() -> Element {
    let signal = use_signal(|| LogViewerState::new());
    let (selected, entries, run_logs) = {
        let ui = signal.read();
        (ui.selected_run.clone(), ui.entries.len(), ui.run_logs.len())
    };
    match selected {
        Some(run_id) => render_log_entries(&run_id, entries, signal),
        None => render_run_list(run_logs, signal),
    }
}

fn render_run_list(count: usize, signal: Signal<LogViewerState>) -> Element {
    if count == 0 { return rsx! { div { class: "log-viewer", div { class: "log-empty", "No log files found." } } }; }
    let items = (0..count).map(|i| { let s = signal.clone(); rsx! { LogRunItem { signal: s, index: i } } }).collect::<Vec<_>>();
    rsx! { div { class: "log-viewer log-run-list", {items.into_iter()} } }
}

#[component]
fn LogRunItem(signal: Signal<LogViewerState>, index: usize) -> Element {
    let run = signal.read().run_logs.get(index).cloned();
    let Some(run) = run else { return rsx! {}; };
    let short = if run.run_id.len() > 12 { format!("{}...", &run.run_id[..9]) } else { run.run_id.clone() };
    rsx! { div { class: "log-run-item", span { class: "log-run-item-id", "{short}" } span { class: "log-run-item-count", " {run.event_count} events" } span { class: "log-run-item-count", "  {run.last_event} ({run.last_event_time})" } } }
}

fn render_log_entries(run_id: &str, count: usize, signal: Signal<LogViewerState>) -> Element {
    if count == 0 { return rsx! { div { class: "log-viewer", div { class: "log-empty", "No events in this run." } } }; }
    let run_id = run_id.to_string();
    let items = (0..count).map(|i| { let s = signal.clone(); rsx! { LogEntryItem { signal: s, index: i } } }).collect::<Vec<_>>();
    rsx! { div { class: "log-viewer", div { style: "margin-bottom: 8px; font-size: 12px; color: #888;", "Log: {run_id}" } {items.into_iter()} } }
}

#[component]
fn LogEntryItem(signal: Signal<LogViewerState>, index: usize) -> Element {
    let entry = signal.read().entries.get(index).cloned();
    let Some(entry) = entry else { return rsx! {}; };
    let color = match entry.event_type.as_str() {
        "AgentStart" | "AgentComplete" => "#40c040", "ToolCallBegin" | "ToolCallComplete" => "#c0c040",
        "ToolCallError" | "AgentAborted" => "#c04040", _ => "#e0e0e0",
    };
    rsx! { div { class: "log-entry", span { class: "log-entry-time", "[{entry.timestamp}] " } span { class: "log-entry-type", style: "color: {color};", "{entry.event_type}" } span { style: "color: {color};", " -- {entry.summary}" } } }
}
```

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/approval_dialog.rs crates/vol-llm-ui/src/web/components/input_area.rs crates/vol-llm-ui/src/web/components/skills.rs crates/vol-llm-ui/src/web/components/session_dialog.rs crates/vol-llm-ui/src/web/components/log_viewer.rs
git commit -m "refactor: remaining components use local or shared signals"
```

---

### Task 10: Refactor FileContentView — reads WorkspaceState from context

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/file_content.rs`

- [ ] **Step 1: Rewrite file_content.rs**

```rust
//! File content preview shown in the Workspace tab when files are open.

use dioxus::prelude::*;
use crate::state::{OpenFileTab, WorkspaceState};
use crate::web::components::file_tree::file_icon;

#[component]
pub fn FileContentView() -> Element {
    let signal: Signal<WorkspaceState> = use_context();
    let (open_files, selected) = {
        let ws = signal.read();
        (ws.open_files.clone(), ws.selected_file_tab)
    };
    if open_files.is_empty() {
        return rsx! { div { class: "file-content-view", div { class: "file-content-empty", "Click a file in the explorer to open it" } } };
    }
    let tabs: Vec<Element> = open_files.iter().enumerate().map(|(i, tab)| render_tab(i, tab, signal)).collect();
    rsx! {
        div { class: "file-content-view",
            div { class: "file-tab-bar", {tabs.into_iter()} }
            {if let Some(idx) = selected {
                if let Some(tab) = open_files.get(idx) {
                    match (&tab.content, &tab.error) {
                        (Some(c), _) => rsx! { FileContentDisplay { content: c.clone() } },
                        (None, Some(e)) => rsx! { div { class: "file-content-error", "Error: {e}" } },
                        (None, None) => rsx! { div { class: "file-content-loading", "Loading..." } },
                    }
                } else { rsx! {} }
            } else { rsx! {} }}
        }
    }
}

fn render_tab(i: usize, tab: &OpenFileTab, signal: Signal<WorkspaceState>) -> Element {
    let name = tab.path.split('/').last().unwrap_or(&tab.path).to_string();
    let icon = file_icon(false, &name);
    let path = tab.path.clone();
    let is_sel = { let ws = signal.read(); Some(i) == ws.selected_file_tab };
    let cls = if is_sel { "file-tab active" } else { "file-tab" };
    let mut sig_sel = signal.clone();
    let mut sig_close = signal.clone();
    let close_path = path.clone();
    rsx! {
        div { class: cls, key: "{path}",
            onclick: move |_: Event<MouseData>| { sig_sel.with_mut(|s| s.selected_file_tab = Some(i)); },
            span { class: "file-tab-icon", "{icon}" }
            span { class: "file-tab-name", "{name}" }
            span { class: "file-tab-close", onclick: move |evt: Event<MouseData>| {
                evt.stop_propagation();
                sig_close.with_mut(|s| {
                    if let Some(pos) = s.open_files.iter().position(|t| t.path == close_path) {
                        s.open_files.remove(pos);
                        if s.open_files.is_empty() { s.selected_file_tab = None; }
                        else if s.selected_file_tab == Some(pos) { s.selected_file_tab = Some(pos.min(s.open_files.len().saturating_sub(1))); }
                        else if s.selected_file_tab.map(|x| x > pos).unwrap_or(false) { s.selected_file_tab = s.selected_file_tab.map(|x| x - 1); }
                    }
                });
            }, "\u{2715}" }
        }
    }
}

#[component]
fn FileContentDisplay(content: String) -> Element {
    rsx! { pre { class: "file-content", {content} } }
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/file_content.rs
git commit -m "refactor: FileContentView reads WorkspaceState from context"
```

---

### Task 11: Verify builds and fix any issues

- [ ] **Step 1: Verify web build**

```bash
cargo check -p vol-llm-ui --features web --bin vol-llm-ui-web
```

- [ ] **Step 2: Verify TUI build**

```bash
cargo check -p vol-llm-ui --features tui
```

- [ ] **Step 3: Run tests**

```bash
cargo test --package vol-llm-ui
```

- [ ] **Step 4: Fix any compilation errors**

Common issues to watch for:
- Missing imports (e.g., `gloo_timers` if used — remove if not needed)
- Dioxus hook ordering (use_context_provider before use_context)
- Type mismatches in signal contexts

- [ ] **Step 5: Final commit**

```bash
git add -A
git commit -m "fix: resolve compilation issues for web and tui builds"
```

---

## Verification Checklist

- [ ] `cargo check -p vol-llm-ui --features web --bin vol-llm-ui-web` — WASM build
- [ ] `cargo check -p vol-llm-ui --features tui` — TUI build
- [ ] `cargo test --package vol-llm-ui` — all tests pass
- [ ] `StatusBar` reads from shared `Signal<GlobalState>` — no local subscriptions
- [ ] `InputArea` reads from shared signals — no local subscriptions
- [ ] `ApprovalDialog` reads from shared `Signal<ApprovalUiState>` — no local subscriptions
- [ ] `ConversationView` creates local signal + subscribes to 13 event kinds via EventBus
- [ ] `ToolsPanel` creates local signal + subscribes to 4 ToolCall event kinds
- [ ] `ToolsTabContent` creates local signal + subscribes to 4 ToolCall event kinds
- [ ] `FileTree` creates local signal + provides via context — no WS subscriptions
- [ ] `FileContentView` reads `WorkspaceState` from context
- [ ] `SessionDialog`, `SkillsPanel`, `LogViewer` create local signals — no WS subscriptions
- [ ] `AppState` only holds `EventBus`, `JsonRpcClient`, `Signal<ActiveTab>`
- [ ] `UiState::apply()` still exists for TUI (not deleted, TUI still uses it)
- [ ] EventBus routes by `UiEventKind` — publish only invokes matching subscribers
