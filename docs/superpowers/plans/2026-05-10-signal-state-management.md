# Signal-First State Management Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace manual version-counter state management in web with Dioxus `Signal<UiState>`, replace tick-based TUI rendering with event-driven rendering.

**Architecture:** Web uses `Signal<UiState>` for automatic re-render subscriptions. TUI uses `Arc<RwLock<UiState>>` with event-driven render triggers via `tokio::sync::mpsc::channel`. No abstraction layer between frontends.

**Tech Stack:** Dioxus 0.6 `Signal<T>` (web), `Arc<RwLock<T>>` (TUI), `tokio::sync::mpsc` (TUI render trigger).

---

## Task Overview

There are two independent subsystems (web and TUI) that can be implemented in any order. This plan does **web first** (Task 1-3), then **TUI** (Tasks 4-5). Each task produces a working, compilable state.

### File Structure Map

| File | Responsibility | Change Type |
|------|---------------|-------------|
| `src/web/components/app.rs` | Root component: state creation, event loop, routing | Major rewrite — replace `Rc<RefCell>` + `Signal<u64>` with `Signal<UiState>` |
| `src/web/components/status_bar.rs` | Status bar, connection indicator | Replace `state.borrow()` with `signal.read()` |
| `src/web/components/conversation.rs` | Conversation panel | Replace `state.borrow()` with `signal.read()` |
| `src/web/components/input_area.rs` | User input form | Replace `state.borrow()` with `signal.read()` |
| `src/web/components/approval_dialog.rs` | HITL approval modal | Replace `borrow_mut`+`bump_ver` with `signal.with_mut()` |
| `src/web/components/session_dialog.rs` | Session management modal | Replace `borrow_mut`+`bump_ver` with `signal.with_mut()` |
| `src/web/components/skills.rs` | Skills table | Replace `state.borrow()` with `signal.read()` |
| `src/web/components/log_viewer.rs` | Log viewer | Replace `state.borrow()` with `signal.read()` |
| `src/web/components/tools_panel.rs` | Old tools panel (left sidebar) | Replace `state.borrow()` with `signal.read()` |
| `src/web/components/tools_tab.rs` | Tools tab content | Replace `state.borrow()` + version bump with `signal.read()` + `signal.with_mut()` |
| `src/web/components/file_tree.rs` | File explorer tree | Replace `state.borrow()` + `bump_version()` with `signal.read()` + `signal.with_mut()` |
| `src/web/components/file_content.rs` | File content tabs | Replace `state.borrow()` + `bump_version()` with `signal.read()` + `signal.with_mut()` |
| `src/web/components/workspace.rs` | Old workspace panel | Replace `state.borrow()` with `signal.read()` |
| `src/connection/local.rs` | Local agent connection with observer | Replace `Mutex` → `RwLock`, add `render_tx` to observer |
| `src/tui/bin/tui.rs` | TUI main loop | Replace `Mutex` → `RwLock`, remove tick, add `render_rx` channel |
| `src/state/mod.rs` | State types | No changes |
| `src/state/event_buffer.rs` | EventBuffer | No changes |

---

### Task 1: Web Root Component — Signal<UiState>

**Files:**
- Modify: `src/web/components/app.rs`

This is the foundational change. All other web components depend on `AppState` changing from `Rc<RefCell<UiState>>` + `Signal<u64>` to `Signal<UiState>`.

- [ ] **Step 1: Update AppState struct**

Replace the `ui_state` and `version` fields with a single `signal` field:

```rust
#[derive(Clone)]
pub struct AppState {
    pub signal: Signal<UiState>,
    pub active_tab: Signal<ActiveTab>,
    pub rpc_client: JsonRpcClient,
}
```

Remove the `PartialEq` impl for `AppState` (not needed with `Signal`).

Remove the `bump_ver` function entirely.

- [ ] **Step 2: Update App component state creation**

Replace:
```rust
let ui_state: Rc<RefCell<UiState>> = Rc::new(RefCell::new(
    UiState::new("web-session".into(), "/workspace", &ws_url)
));
let version = use_signal(|| 0u64);
```

With:
```rust
let signal = use_signal(|| UiState::new("web-session".into(), "/workspace", &ws_url));
```

- [ ] **Step 3: Update use_hook — connection state changes**

Replace the `on_state_change` callback. Old pattern:
```rust
let ui = ui_state.clone();
let ver = version;
c.on_state_change(move |state| {
    if let Ok(mut s) = ui.try_borrow_mut() {
        s.ws_connected = true;
    }
    bump_ver(ver);
});
```

New pattern:
```rust
let sig = signal.clone();
c.on_state_change(move |cs| {
    sig.with_mut(|s| match cs {
        crate::web::client::ConnectionState::Connected => {
            s.ws_connected = true;
            s.ws_last_error = None;
        }
        crate::web::client::ConnectionState::Connecting => {
            s.ws_connected = false;
        }
        crate::web::client::ConnectionState::Disconnected => {
            s.ws_connected = false;
            s.ws_last_error = Some("Disconnected from server".to_string());
        }
    });
});
```

- [ ] **Step 4: Update use_hook — file_list callback on connect**

Replace the `file_list` callback inside `on_state_change`. Old:
```rust
client_ws.file_list(".", move |result| {
    if let Ok(entries) = result {
        if let Ok(mut s) = ui_ws.try_borrow_mut() { /* ... */ }
        bump_ver(ver_cb);
    }
});
```

New:
```rust
client_ws.file_list(".", move |result| {
    if let Ok(entries) = result {
        sig_cb.with_mut(|s| {
            s.workspace.root = ".".to_string();
            s.workspace.entries.clear();
            for entry in &entries {
                let indent = entry.name.chars().filter(|&c| c == '/').count();
                s.workspace.entries.push(crate::state::WorkspaceEntry {
                    path: entry.name.clone(),
                    is_dir: entry.is_dir,
                    modified: false,
                    indent,
                });
            }
        });
    }
});
```

- [ ] **Step 5: Update use_hook — event loop**

Replace:
```rust
let ui_ev = ui_state.clone();
let ver_ev = version;
let client_ev = c.clone();
wasm_bindgen_futures::spawn_local(async move {
    loop {
        match client_ev.next_event().await {
            Some(event) => {
                if let Some(ui_event) = agent_event_to_ui(&event) {
                    if let Ok(mut s) = ui_ev.try_borrow_mut() {
                        s.apply(ui_event);
                    }
                    bump_ver(ver_ev);
                }
            }
            None => { /* ... */ }
        }
    }
});
```

With:
```rust
let sig_ev = signal.clone();
let client_ev = c.clone();
wasm_bindgen_futures::spawn_local(async move {
    loop {
        match client_ev.next_event().await {
            Some(event) => {
                if let Some(ui_event) = agent_event_to_ui(&event) {
                    sig_ev.with_mut(|s| s.apply(ui_event));
                }
            }
            None => {
                log::warn!("Event stream closed");
                sig_ev.with_mut(|s| {
                    s.ws_connected = false;
                    s.ws_last_error = Some("Event stream closed".to_string());
                });
                break;
            }
        }
    }
});
```

- [ ] **Step 6: Update use_context_provider**

Replace:
```rust
use_context_provider(|| AppState {
    ui_state,
    version,
    active_tab,
    rpc_client: client.clone(),
});
```

With:
```rust
use_context_provider(|| AppState {
    signal,
    active_tab,
    rpc_client: client.clone(),
});
```

Remove:
```rust
let ver = version.read();
let _ = *ver;
```

- [ ] **Step 7: Update TabButton onclick**

Replace:
```rust
onclick: move |_| {
    active_tab_signal.set(tab);
    if let Ok(mut s) = ui.try_borrow_mut() {
        s.active_tab = tab;
    }
    bump_ver(ver);
},
```

With:
```rust
onclick: move |_| {
    active_tab_signal.set(tab);
    state.signal.with_mut(|s| s.active_tab = tab);
},
```

- [ ] **Step 8: Verify WASM build compiles**

Run:
```bash
cd crates/vol-llm-ui && cargo check --features web --target wasm32-unknown-unknown
```

Expected: No compile errors.

- [ ] **Step 9: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/app.rs
git commit -m "refactor(web): replace Rc<RefCell> + Signal<u64> with Signal<UiState>"
```

---

### Task 2: Web Components — Replace borrow() with signal.read()

**Files:**
- Modify: `src/web/components/status_bar.rs`
- Modify: `src/web/components/conversation.rs`
- Modify: `src/web/components/input_area.rs`
- Modify: `src/web/components/skills.rs`
- Modify: `src/web/components/log_viewer.rs`
- Modify: `src/web/components/tools_panel.rs`
- Modify: `src/web/components/workspace.rs`

These components only **read** state — no mutations. The change is mechanical: `state.ui_state.borrow()` → `state.signal.read()`.

- [ ] **Step 1: Update status_bar.rs**

Replace:
```rust
let ui = state.ui_state.borrow();
```

With:
```rust
let ui = state.signal.read();
```

Remove `drop(ui)` at the end of the borrow block (not needed — `ReadGuard` is dropped at end of scope).

- [ ] **Step 2: Update conversation.rs**

Replace all occurrences of `state.ui_state.borrow()` with `state.signal.read()`:

In `ConversationView()`:
```rust
let count = state.signal.read().conversation.len();
```

In `MessageEntry()`:
```rust
let entry = state.signal.read().conversation.get(index).cloned();
```

- [ ] **Step 3: Update input_area.rs**

Replace:
```rust
let is_running = state.ui_state.borrow().is_running;
let has_approval = state.ui_state.borrow().approval_state.has_pending();
```

With:
```rust
let state_ref = state.signal.read();
let is_running = state_ref.is_running;
let has_approval = state_ref.approval_state.has_pending();
```

- [ ] **Step 4: Update skills.rs**

Replace:
```rust
let count = state.ui_state.borrow().skills.len();
```

With:
```rust
let count = state.signal.read().skills.len();
```

Replace in `SkillRow`:
```rust
let skill = state.signal.read().skills.get(index).cloned();
```

- [ ] **Step 5: Update log_viewer.rs**

Replace all `state.ui_state.borrow()` with `state.signal.read()` in:
- `LogViewer()`: the destructuring block
- `LogRunItem()`: `state.ui_state.borrow().log_viewer_run_logs.get(index).cloned()`
- `LogEntryItem()`: `state.ui_state.borrow().log_viewer_entries.get(index).cloned()`

- [ ] **Step 6: Update tools_panel.rs**

Replace all `state.ui_state.borrow()` with `state.signal.read()` in:
- `ToolsPanel()`: `state.ui_state.borrow().tool_calls.len()`
- `ToolItem()`: the destructuring block

- [ ] **Step 7: Update workspace.rs**

Replace all `state.ui_state.borrow()` with `state.signal.read()` in:
- `WorkspacePanel()`: `state.ui_state.borrow().workspace.entries.len()`
- `WorkspaceItem()`: the destructuring block

- [ ] **Step 8: Verify WASM build compiles**

Run:
```bash
cd crates/vol-llm-ui && cargo check --features web --target wasm32-unknown-unknown
```

Expected: No compile errors.

- [ ] **Step 9: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/status_bar.rs \
  crates/vol-llm-ui/src/web/components/conversation.rs \
  crates/vol-llm-ui/src/web/components/input_area.rs \
  crates/vol-llm-ui/src/web/components/skills.rs \
  crates/vol-llm-ui/src/web/components/log_viewer.rs \
  crates/vol-llm-ui/src/web/components/tools_panel.rs \
  crates/vol-llm-ui/src/web/components/workspace.rs
git commit -m "refactor(web): replace state.borrow() with signal.read() across components"
```

---

### Task 3: Web Components — Replace bump_version() with signal.with_mut()

**Files:**
- Modify: `src/web/components/approval_dialog.rs`
- Modify: `src/web/components/session_dialog.rs`
- Modify: `src/web/components/tools_tab.rs`
- Modify: `src/web/components/file_tree.rs`
- Modify: `src/web/components/file_content.rs`

These components **mutate** state. Replace `try_borrow_mut()` + `bump_ver()` pattern with `signal.with_mut()`.

- [ ] **Step 1: Update approval_dialog.rs**

Replace `on_approve` handler. Old:
```rust
let ui = state.ui_state.clone();
let mut ver = state.version;
let on_approve = move |_: Event<MouseData>| {
    if let Ok(mut s) = ui.try_borrow_mut() {
        s.apply(UiEvent::ApprovalResolved { approved: true });
    }
    let v = *ver.peek();
    let next = v.wrapping_add(1);
    ver.set(next);
};
```

New:
```rust
let sig = state.signal.clone();
let on_approve = move |_: Event<MouseData>| {
    sig.with_mut(|s| s.apply(UiEvent::ApprovalResolved { approved: true }));
};
```

Same pattern for `on_reject`.

For `on_stop`:
```rust
let sig = state.signal.clone();
let on_stop = move |_: Event<MouseData>| {
    sig.with_mut(|s| {
        s.apply(UiEvent::ApprovalResolved { approved: false });
        s.is_running = false;
        s.conversation.push(ConversationEntry::Error {
            message: "Agent stopped by user".to_string(),
        });
    });
};
```

Update the initial `has_pending` check:
```rust
let has_pending = state.signal.read().approval_state.has_pending();
```

Update the data reads before `rsx!`:
```rust
let tool_name = state.signal.read().approval_state.tool_name.clone().unwrap_or_default();
let reason = state.signal.read().approval_state.reason.clone().unwrap_or_default();
let arguments = state.signal.read().approval_state.arguments.clone().unwrap_or_default();
```

Remove `bump_version` import if present.

- [ ] **Step 2: Update session_dialog.rs**

Replace `on_new`:
```rust
let sig = state.signal.clone();
let on_new = move |_: Event<MouseData>| {
    let new_id = uuid_v4_stub();
    sig.with_mut(|s| {
        s.session_id = new_id;
        s.session_dialog_open = false;
    });
};
```

Replace `on_resume`:
```rust
let sig = state.signal.clone();
let on_resume = move |_: Event<MouseData>| {
    sig.with_mut(|s| {
        let sel = s.session_dialog_selected;
        let session_id = s.session_dialog_sessions.get(sel).map(|e| e.session_id.clone()).unwrap_or_default();
        if !session_id.is_empty() {
            s.session_id = session_id;
        }
        s.session_dialog_open = false;
    });
};
```

Replace `on_delete`:
```rust
let sig = state.signal.clone();
let on_delete = move |_: Event<MouseData>| {
    sig.with_mut(|s| {
        let sel = s.session_dialog_selected;
        let current = s.session_id.clone();
        if let Some(entry) = s.session_dialog_sessions.get(sel) {
            if entry.session_id != current {
                s.session_dialog_sessions.remove(sel);
                if !s.session_dialog_sessions.is_empty() {
                    s.session_dialog_selected = sel.min(s.session_dialog_sessions.len().saturating_sub(1));
                }
            }
        }
    });
};
```

Replace `onclick` for session selection:
```rust
let sig = state.signal.clone();
let idx = i;
rsx! {
    div {
        class: cls,
        onclick: move |_: Event<MouseData>| {
            sig.with_mut(|s| s.session_dialog_selected = idx);
        },
        // ...
    }
}
```

Replace overlay `onclick`:
```rust
let sig = state.signal.clone();
rsx! {
    div { class: "modal-overlay", onclick: move |_: Event<MouseData>| {
        sig.with_mut(|s| s.session_dialog_open = false);
    },
    // ...
}
```

Update reads:
```rust
let open = state.signal.read().session_dialog_open;
let sessions = state.signal.read().session_dialog_sessions.clone();
let selected = state.signal.read().session_dialog_selected;
```

- [ ] **Step 3: Update tools_tab.rs**

Update `ToolCallItem` expand/collapse handler. Old:
```rust
onclick: move |_: Event<MouseData>| {
    let ui = state.ui_state.clone();
    let mut ver = state.version;
    let idx = index;
    if let Ok(mut s) = ui.try_borrow_mut() {
        if s.expanded_tool_calls.contains(&idx) {
            s.expanded_tool_calls.remove(&idx);
        } else {
            s.expanded_tool_calls.insert(idx);
        }
    }
    let v = (*ver.peek()).wrapping_add(1);
    ver.set(v);
},
```

New:
```rust
onclick: move |_: Event<MouseData>| {
    let sig = state.signal.clone();
    let idx = index;
    sig.with_mut(move |s| {
        if s.expanded_tool_calls.contains(&idx) {
            s.expanded_tool_calls.remove(&idx);
        } else {
            s.expanded_tool_calls.insert(idx);
        }
    });
},
```

Update reads:
```rust
let count = state.signal.read().tool_calls.len();
```

In `ToolCallItem`:
```rust
let (seq, tool_name, arg_preview, status, duration_ms) = {
    let ui = state.signal.read();
    match ui.tool_calls.get(index) {
        // ...
    }
};

let is_expanded = state.signal.read().expanded_tool_calls.contains(&index);
```

- [ ] **Step 4: Update file_tree.rs**

Remove the `bump_version` function.

Replace directory toggle `onclick`:
```rust
let sig = state.signal.clone();
let dir_path = path.clone();
let dir_onclick = move |_: Event<MouseData>| {
    let p = dir_path.clone();
    sig.with_mut(move |s| {
        if s.collapsed_dirs.contains(&p) {
            s.collapsed_dirs.remove(&p);
        } else {
            s.collapsed_dirs.insert(p);
        }
    });
};
```

Replace file `onclick` (the complex one with file_read callback):
```rust
let sig = state.signal.clone();
let mut tab = state.active_tab;
let rpc = state.rpc_client.clone();
let file_path = path.clone();
let file_onclick = move |_: Event<MouseData>| {
    let p = file_path.clone();
    let rpc_clone = rpc.clone();
    let sig_clone = sig.clone();

    // Open or select existing tab, and determine if we need to fetch
    let needs_fetch = sig.with_mut(|s| {
        let existing = s.open_files.iter().position(|f| f.path == p);
        match existing {
            Some(idx) => {
                s.selected_file_tab = Some(idx);
                false // already loaded
            }
            None => {
                let new_idx = s.open_files.len();
                s.open_files.push(OpenFileTab {
                    path: p.clone(),
                    content: None,
                    error: None,
                });
                s.selected_file_tab = Some(new_idx);
                true // new tab, needs fetch
            }
        }
    });

    if needs_fetch {
        let sig_cb = sig_clone.clone();
        let read_path = p.clone();
        rpc_clone.file_read(&p, move |result| {
            sig_cb.with_mut(|s| {
                if let Some(idx) = s.open_files.iter().position(|f| f.path == read_path) {
                    match result {
                        Ok(c) => { s.open_files[idx].content = Some(c); }
                        Err(e) => { s.open_files[idx].error = Some(e); }
                    }
                }
            });
        });
    }

    tab.set(ActiveTab::Workspace);
};
```

Replace `collapsed` read in directory rendering:
```rust
let collapsed = state.signal.read().collapsed_dirs.contains(&path);
```

Replace tree read in `FileTree()`:
```rust
let tree = {
    let ui = state.signal.read();
    build_tree(&ui.workspace.entries)
};
```

- [ ] **Step 5: Update file_content.rs**

Remove the `bump_version` function.

Replace `FileContentView` reads:
```rust
let (open_files, selected) = {
    let ui = state.signal.read();
    (ui.open_files.clone(), ui.selected_file_tab)
};
```

Replace `render_tab` — tab selection:
```rust
let is_selected = {
    let ui = state.signal.read();
    Some(i) == ui.selected_file_tab
};
```

Replace tab select `onclick`:
```rust
let sig = state.signal.clone();
let select_onclick = move |_: Event<MouseData>| {
    sig.with_mut(|s| s.selected_file_tab = Some(i));
};
```

Replace tab close `onclick`:
```rust
let sig = state.signal.clone();
let close_path = path.clone();
let close_onclick = move |evt: Event<MouseData>| {
    evt.stop_propagation();
    sig.with_mut(|s| {
        if let Some(pos) = s.open_files.iter().position(|t| t.path == close_path) {
            s.open_files.remove(pos);
            if s.open_files.is_empty() {
                s.selected_file_tab = None;
            } else if s.selected_file_tab == Some(pos) {
                let new_len = s.open_files.len();
                s.selected_file_tab = Some(pos.min(new_len.saturating_sub(1)));
            } else if s.selected_file_tab.map(|s| s > pos).unwrap_or(false) {
                s.selected_file_tab = s.selected_file_tab.map(|s| s - 1);
            }
        }
    });
};
```

- [ ] **Step 6: Verify WASM build compiles**

Run:
```bash
cd crates/vol-llm-ui && cargo check --features web --target wasm32-unknown-unknown
```

Expected: No compile errors.

- [ ] **Step 7: Run tests**

Run:
```bash
cd crates/vol-llm-ui && cargo test
```

Expected: All tests pass.

- [ ] **Step 8: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/approval_dialog.rs \
  crates/vol-llm-ui/src/web/components/session_dialog.rs \
  crates/vol-llm-ui/src/web/components/tools_tab.rs \
  crates/vol-llm-ui/src/web/components/file_tree.rs \
  crates/vol-llm-ui/src/web/components/file_content.rs
git commit -m "refactor(web): replace bump_version() with signal.with_mut() for mutations"
```

---

### Task 4: TUI — RwLock + Event-Driven Render

**Files:**
- Modify: `src/connection/local.rs`
- Modify: `src/tui/bin/tui.rs`

This replaces `tokio::sync::Mutex` with `tokio::sync::RwLock` and adds a render trigger channel to `LocalEventObserver`.

- [ ] **Step 1: Update LocalConnection in local.rs**

Replace the `state` field type. Change:
```rust
pub struct LocalConnection {
    agent_config: CodingAgentConfig,
    state: Arc<tokio::sync::Mutex<UiState>>,
    connected: Arc<AtomicBool>,
    cancelled: Arc<AtomicBool>,
}
```

To:
```rust
pub struct LocalConnection {
    agent_config: CodingAgentConfig,
    state: Arc<tokio::sync::RwLock<UiState>>,
    connected: Arc<AtomicBool>,
    cancelled: Arc<AtomicBool>,
}
```

Update `new()`:
```rust
pub fn new(config: CodingAgentConfig, state: Arc<tokio::sync::RwLock<UiState>>) -> Self {
    Self {
        agent_config: config,
        state,
        connected: Arc::new(AtomicBool::new(true)),
        cancelled: Arc::new(AtomicBool::new(false)),
    }
}
```

Update `approve_tool` — change `.lock().await` to `.write().await`:
```rust
async fn approve_tool(&self, ...) -> anyhow::Result<()> {
    let mut state = self.state.write().await;
    state.approval_state.response = Some((approved, None));
    Ok(())
}
```

Update `cancel` — change `.lock().await` to `.write().await`:
```rust
async fn cancel(&self, _req_id: String) -> anyhow::Result<()> {
    self.cancelled.store(true, Ordering::Relaxed);
    let mut state = self.state.write().await;
    state.is_running = false;
    Ok(())
}
```

Update tests — change all `tokio::sync::Mutex::new(...)` to `tokio::sync::RwLock::new(...)`.

- [ ] **Step 2: Update LocalEventObserver in local.rs**

Change:
```rust
struct LocalEventObserver {
    state: Arc<tokio::sync::Mutex<UiState>>,
    event_tx: mpsc::Sender<UiEvent>,
}
```

To:
```rust
struct LocalEventObserver {
    state: Arc<tokio::sync::RwLock<UiState>>,
    event_tx: mpsc::Sender<UiEvent>,
    render_tx: mpsc::Sender<()>,
}
```

Update `on_event`:
```rust
async fn on_event(&self, event: &AgentStreamEvent) -> Result<(), ObserverError> {
    let mut buffer = EventBuffer::new();
    let mut state = self.state.write().await;
    buffer.apply_stream(event, &mut state);
    drop(state);
    let _ = self.render_tx.try_send(());
    Ok(())
}
```

Update `run_agent` — pass `render_tx` to the observer:
```rust
let observer = Arc::new(LocalEventObserver {
    state: state.clone(),
    event_tx: tx.clone(),
    render_tx: self.render_tx.clone(),
});
```

Add `render_tx` field to `LocalConnection`:
```rust
pub struct LocalConnection {
    agent_config: CodingAgentConfig,
    state: Arc<tokio::sync::RwLock<UiState>>,
    connected: Arc<AtomicBool>,
    cancelled: Arc<AtomicBool>,
    render_tx: mpsc::Sender<()>,
}
```

Update `new()`:
```rust
pub fn new(config: CodingAgentConfig, state: Arc<tokio::sync::RwLock<UiState>>, render_tx: mpsc::Sender<()>) -> Self {
    Self {
        agent_config: config,
        state,
        connected: Arc::new(AtomicBool::new(true)),
        cancelled: Arc::new(AtomicBool::new(false)),
        render_tx,
    }
}
```

Update `clone_for_run()`:
```rust
pub fn clone_for_run(&self) -> Self {
    Self {
        agent_config: self.agent_config.clone(),
        state: self.state.clone(),
        connected: self.connected.clone(),
        cancelled: Arc::new(AtomicBool::new(false)),
        render_tx: self.render_tx.clone(),
    }
}
```

- [ ] **Step 3: Update tui.rs main loop**

Replace state creation:
```rust
let (render_tx, mut render_rx) = tokio::sync::mpsc::channel(64);
let ui_state = Arc::new(tokio::sync::RwLock::new(UiState::new(
    session_id,
    working_dir.to_string_lossy().as_ref(),
    "local",
)));
```

Replace connection creation:
```rust
let connection = LocalConnection::new(agent_config, ui_state.clone(), render_tx.clone());
let connection = Arc::new(connection);
```

Remove:
```rust
let mut render_interval = tokio::time::interval(Duration::from_millis(33));
```

Replace the `tokio::select!` block. Old:
```rust
loop {
    tokio::select! {
        biased;
        maybe_event = events.next() => { /* input handling */ }
        _ = render_interval.tick() => {
            let state = ui_state.lock().await;
            terminal.draw(|f| render_ui(f, &state))?;
        }
    }
}
```

New:
```rust
loop {
    tokio::select! {
        biased;

        // Render — highest priority
        _ = render_rx.recv() => {
            let state = ui_state.read().await;
            terminal.draw(|f| render_ui(f, &state))?;
        }

        // Input
        maybe_event = events.next() => {
            match maybe_event {
                Some(Ok(Event::Key(key))) => {
                    let mut state = ui_state.write().await;
                    let action = handle_key(key, &mut state, &input_buf);
                    match action {
                        InputAction::Exit => break,
                        InputAction::Send(text) => {
                            input_buf.clear();
                            state.is_running = true;
                            let conn = connection.clone();
                            let state_clone = ui_state.clone();
                            let render_tx_clone = render_tx.clone();
                            tokio::spawn(async move {
                                match conn.submit(text).await {
                                    Ok(rx) => {
                                        let mut rx = rx;
                                        while let Some(_event) = rx.recv().await {
                                            // Events applied by observer
                                        }
                                        // Ensure is_running is cleared
                                        let mut s = state_clone.write().await;
                                        s.is_running = false;
                                        drop(s);
                                        let _ = render_tx_clone.try_send(());
                                    }
                                    Err(e) => {
                                        let mut s = state_clone.write().await;
                                        s.is_running = false;
                                        s.last_error = Some(format!("{}", e));
                                        drop(s);
                                        let _ = render_tx_clone.try_send(());
                                    }
                                }
                            });
                        }
                        InputAction::ResumeSession(_id) => {}
                        InputAction::None => {}
                    }
                    // Trigger render after input handling
                    let _ = render_tx.try_send(());
                }
                Some(Ok(Event::Resize(_, _))) => {}
                _ => {}
            }
        }
    }
}
```

Remove unused imports: `tokio::time::Duration` if no longer needed elsewhere.

- [ ] **Step 4: Verify TUI build compiles**

Run:
```bash
cd crates/vol-llm-ui && cargo check --features tui
```

Expected: No compile errors.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-ui/src/connection/local.rs \
  crates/vol-llm-ui/src/tui/bin/tui.rs
git commit -m "refactor(tui): replace Mutex with RwLock, tick with event-driven render"
```

---

### Task 5: Cleanup and Verification

**Files:**
- All modified files above
- `src/state/mod.rs` (verify unchanged)

- [ ] **Step 1: Run full test suite**

Run:
```bash
cargo test
```

Expected: All tests pass, including `test_ui_event_*`, `test_ui_state_*`, `test_active_tab_toggle`, `test_local_connection_*`.

- [ ] **Step 2: Verify WASM build**

Run:
```bash
cargo check --features web --target wasm32-unknown-unknown
```

Expected: No errors.

- [ ] **Step 3: Verify TUI build**

Run:
```bash
cargo check --features tui
```

Expected: No errors.

- [ ] **Step 4: Check for stale code**

Search for any remaining `bump_ver`, `bump_version`, `Signal<u64>`, `version` signal references in web components:
```bash
grep -rn 'bump_ver\|bump_version\|Signal<u64>' crates/vol-llm-ui/src/web/
```

Expected: No results.

Search for any remaining `Mutex<UiState>` in TUI code:
```bash
grep -rn 'Mutex.*UiState' crates/vol-llm-ui/src/
```

Expected: No results (only `RwLock<UiState>` should remain).

- [ ] **Step 5: Commit**

```bash
git add .
git commit -m "chore: final cleanup for signal-first state management"
```
