# Signal-First State Management Design

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the manual version-counter state management in web with Dioxus `Signal<UiState>`, and replace tick-based TUI rendering with event-driven rendering, while sharing `state/mod.rs` as the only common code.

**Architecture:** Web uses `Signal<UiState>` for automatic re-render subscriptions. TUI uses `Arc<RwLock<UiState>>` with event-driven render triggers via `tokio::sync::mpsc::channel`. No abstraction layer between frontends — each uses its natural reactive primitive.

**Tech Stack:** Dioxus 0.6 `Signal<T>` (web), `Arc<RwLock<T>>` (TUI), `tokio::sync::mpsc` (TUI render trigger).

---

## 1. Shared Layer (No Changes)

`state/mod.rs` — `UiState` struct, `UiEvent` enum, and `UiState::apply()` method remain unchanged. This is the only code shared between web and TUI frontends. The `apply()` method already works in-place (`&mut self`), mapping directly to both `Signal::with_mut()` and `RwLock::write()`.

## 2. Web Frontend: `Signal<UiState>`

### 2.1 Root Component (`app.rs`)

Replace the current `Rc<RefCell<UiState>>` + `Signal<u64>` version counter with a single `Signal<UiState>`:

```rust
#[component]
pub fn App() -> Element {
    let ws_url = derive_ws_url();
    let signal = use_signal(|| UiState::new("web-session".into(), "/workspace", &ws_url));
    let active_tab = use_signal(|| ActiveTab::Conversation);

    let client = use_hook(|| {
        let c = JsonRpcClient::new(&ws_url);
        let signal_for_loop = signal.clone();
        spawn(async move {
            loop {
                if let Some(event) = signal_for_loop.read().event_rx.next().await {
                    // Convert AgentEvent → UiEvent, apply to state
                    signal_for_loop.with_mut(|s| s.apply(ui_event));
                }
            }
        });
        c
    });

    use_context_provider(|| AppState { signal, active_tab, rpc_client: client.clone() });
    // ... rsx!
}
```

### 2.2 Component Read Pattern

Every component replaces `state.borrow()` with `signal.read()`:

```rust
#[component]
fn StatusBar() -> Element {
    let signal = use_context::<Signal<UiState>>();
    let state = signal.read();  // creates subscription
    rsx! {
        div { "Session: {state.session_id}" }
    }
}
```

### 2.3 Event-Driven Mutation

`Signal::with_mut()` handles mutation and automatically triggers re-renders for all subscribed components:

```rust
signal.with_mut(|s| s.apply(ui_event));
// No bump_version() needed — Dioxus handles this
```

### 2.4 File Read Callbacks

When `file_read` completes, mutate the signal directly:

```rust
let sig = signal.clone();
client.file_read(&path, move |result| {
    sig.with_mut(|s| {
        if let Some(tab) = s.open_files.get_mut(idx) {
            match result {
                Ok(content) => { tab.content = Some(content); tab.error = None; }
                Err(e) => { tab.error = Some(e); tab.content = None; }
            }
        }
    });
});
```

### 2.5 Removed

- `Signal<u64>` version counter
- `bump_version()` / `bump_ver()` helper functions
- `try_borrow_mut()` — replaced by `with_mut()` (panic-safe via Dioxus internals)
- Manual `version.read()` in render for subscription

## 3. TUI Frontend: Event-Driven Rendering

### 3.1 `RwLock` over `Mutex`

Replace `Arc<tokio::sync::Mutex<UiState>>` with `Arc<tokio::sync::RwLock<UiState>>`. The render path is read-only and should not block on the observer's write. `RwLock` allows concurrent reads (render + input handling) while serializing writes.

### 3.2 Render Trigger Channel

Add a `render_tx: tokio::sync::mpsc::Sender<()>` to both the main loop and `LocalEventObserver`. After every state mutation, send `()` to trigger a render:

```rust
let (render_tx, mut render_rx) = tokio::sync::mpsc::channel(64);
```

The observer sends after each `apply_stream()`:
```rust
async fn on_event(&self, event: &AgentStreamEvent) -> Result<(), ObserverError> {
    let mut state = self.state.write().await;
    self.buffer.apply_stream(event, &mut state);
    let _ = self.render_tx.try_send(());
    Ok(())
}
```

The main loop's `select!` merges render triggers with input events:
```rust
loop {
    tokio::select! {
        biased;

        // Render — highest priority, runs whenever triggered
        _ = render_rx.recv() => {
            let state = ui_state.read().await;
            terminal.draw(|f| render_ui(f, &state))?;
        }

        // Input
        maybe_event = events.next() => {
            // handle_key mutates state, then triggers render
            let mut state = ui_state.write().await;
            let action = handle_key(key, &mut state, &input_buf);
            match action { ... }
            let _ = render_tx.try_send(());
        }
    }
}
```

Key: `biased` means render always gets priority. If a render trigger arrives while processing input, render happens immediately after input handling completes.

### 3.3 Removed

- `std::time::Duration::from_millis(33)` render interval
- `tokio::time::Interval` tick
- Unnecessary renders when state hasn't changed
- `Mutex` contention between observer, main loop, and render

## 4. File Impact

| File | Change |
|------|--------|
| `src/web/components/app.rs` | Replace `Rc<RefCell<UiState>>` + `Signal<u64>` with `Signal<UiState>`. Remove version counter. |
| `src/web/components/status_bar.rs` | Replace `state.borrow()` with `signal.read()`. |
| `src/web/components/conversation.rs` | Replace `state.borrow()` with `signal.read()`. |
| `src/web/components/input_area.rs` | Replace `state.borrow()` with `signal.read()`. |
| `src/web/components/approval_dialog.rs` | Replace `state.borrow()` with `signal.read()`. |
| `src/web/components/session_dialog.rs` | Replace `state.borrow()` with `signal.read()`. |
| `src/web/components/skills.rs` | Replace `state.borrow()` with `signal.read()`. |
| `src/web/components/log_viewer.rs` | Replace `state.borrow()` with `signal.read()`. |
| `src/web/components/tools_panel.rs` | Replace `state.borrow()` with `signal.read()`. |
| `src/web/components/workspace.rs` | Replace `state.borrow()` with `signal.read()`. |
| `src/web/components/file_tree.rs` | Replace `bump_version()` with `signal.with_mut()`. Remove `bump_version()` helper. |
| `src/web/components/file_content.rs` | Replace `bump_version()` with `signal.with_mut()`. Remove `bump_version()` helper. |
| `src/web/components/tools_tab.rs` | Replace `state.borrow()` with `signal.read()`. |
| `src/web/components/file_tree.rs` | Replace `state.borrow()` with `signal.read()`. |
| `src/connection/local.rs` | Replace `Mutex` → `RwLock`. Add `render_tx` to `LocalEventObserver`. Send render trigger after each `on_event()`. |
| `src/tui/bin/tui.rs` | Replace `Mutex` → `RwLock`. Add `render_rx` channel. Remove tick interval. Render via `select!` on channel. |
| `src/state/mod.rs` | No changes. |
| `src/state/event_buffer.rs` | No changes — `EventBuffer::apply_stream()` logic unchanged. |

## 5. Error Handling

- `Signal::with_mut()` is safe during render — Dioxus handles borrow ordering internally
- `RwLock::write()` in TUI event loop will not block renders since renders use `read()`
- If `render_tx.try_send()` fails (channel full), it's a no-op — the render is already pending

## 6. Backward Compatibility

- `UiState` fields unchanged — components read the same data
- `UiEvent` enum unchanged — event deserialization works identically
- `ActiveTab::Workspace` enum variant kept (not rendered as tab button, but structurally present)
