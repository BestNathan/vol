# Debug Panel — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a debug button to the status bar that opens a debug panel with a WS messages tab showing all sent/received JSON-RPC messages.

**Architecture:** `DebugState` signal holds message buffer and panel state. `JsonRpcClient` gets an optional debug callback for pushing messages. `DebugPanel` component renders as a modal overlay with tab navigation. Recording starts when panel opens, stops when closed; messages persist until page refresh.

**Tech Stack:** Rust, Dioxus 0.6, web-sys, serde_json

---

### Task 1: Add DebugState and WsMessage types

**Files:**
- Modify: `crates/vol-llm-ui/src/state/mod.rs`

- [ ] **Step 1: Add debug types after the existing `GlobalState` struct**

At the end of the file (before `#[cfg(test)]`), add:

```rust
/// Whether a WS message is inbound or outbound.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WsDirection { In, Out }

/// A captured WebSocket message.
#[derive(Debug, Clone)]
pub struct WsMessage {
    pub direction: WsDirection,
    pub method: String,
    pub payload: String,
    pub elapsed_ms: u64,
}

/// Debug panel state.
#[derive(Debug, Clone)]
pub struct DebugState {
    pub open: bool,
    pub active_tab: DebugTab,
    pub ws_messages: Vec<WsMessage>,
    start_time: Option<web_time::Instant>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DebugTab { Ws }

impl DebugState {
    pub fn new() -> Self {
        Self { open: false, active_tab: DebugTab::Ws, ws_messages: Vec::new(), start_time: None }
    }

    pub fn toggle(&mut self) {
        self.open = !self.open;
        if !self.open {
            self.active_tab = DebugTab::Ws;
        }
    }

    pub fn push_ws(&mut self, direction: WsDirection, method: String, payload: String) {
        if self.open {
            if self.start_time.is_none() {
                self.start_time = Some(web_time::Instant::now());
            }
            let elapsed_ms = self.start_time.unwrap().elapsed().as_millis() as u64;
            self.ws_messages.push(WsMessage { direction, method, payload, elapsed_ms });
        }
    }
}
```

- [ ] **Step 2: Verify compilation**

```bash
cargo check -p vol-llm-ui --no-default-features --features web
```

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-ui/src/state/mod.rs
git commit -m "feat(debug): add DebugState, WsMessage, DebugTab types"
```

---

### Task 2: Wire debug collection into JsonRpcClient

**Files:**
- Modify: `crates/vol-llm-ui/src/web/client.rs`

- [ ] **Step 1: Add `debug_state` field to `ClientInner`**

After the `send_queue` field:

```rust
debug_state: RefCell<Option<Signal<crate::state::DebugState>>>,
```

Initialize in `JsonRpcClient::new()`:

```rust
send_queue: RefCell::new(Vec::new()),
debug_state: RefCell::new(None),
```

Also in `reconnect()` — preserve the debug_state reference (it's not recreated there, it's on the existing `inner`).

- [ ] **Step 2: Add setter method to `JsonRpcClient`**

```rust
/// Attach debug state for WS message capture.
pub fn set_debug_state(&self, debug_state: Signal<crate::state::DebugState>) {
    *self.inner.debug_state.borrow_mut() = Some(debug_state);
}
```

- [ ] **Step 3: Push outgoing messages in `send_raw`**

After the ready state check, if the message is being sent or queued successfully, extract method from JSON and push to debug:

```rust
fn push_debug_out(&self, msg: &str) {
    if let Some(ref ds) = *self.inner.debug_state.borrow() {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(msg) {
            let method = val.get("method").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
            ds.write_unchecked().push_ws(crate::state::WsDirection::Out, method, msg.to_string());
        }
    }
}
```

Call `self.push_debug_out(msg)` at the beginning of `send_raw`, before the send/queue logic (so failed sends are also captured, which is useful for debugging).

- [ ] **Step 4: Push incoming messages in `handle_message`**

At the top of `handle_message`:

```rust
fn push_debug_in(inner: &Rc<ClientInner>, data: &str) {
    if let Some(ref ds) = *inner.debug_state.borrow() {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(data) {
            let method = val.get("method").and_then(|v| v.as_str())
                .or_else(|| {
                    // It's a response — extract method from the result/error wrapper
                    val.get("id").map(|_| "<response>")
                })
                .unwrap_or("unknown")
                .to_string();
            ds.write_unchecked().push_ws(crate::state::WsDirection::In, method, data.to_string());
        }
    }
}
```

Call `Self::push_debug_in(inner, data)` as the first line in `handle_message`.

- [ ] **Step 5: Verify compilation**

```bash
cargo check -p vol-llm-ui --no-default-features --features web
```

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-ui/src/web/client.rs
git commit -m "feat(debug): wire WS message capture into JsonRpcClient"
```

---

### Task 3: Create DebugPanel component

**Files:**
- Create: `crates/vol-llm-ui/src/web/components/debug_panel.rs`
- Modify: `crates/vol-llm-ui/src/web/components/mod.rs`

- [ ] **Step 1: Create `debug_panel.rs`**

```rust
//! Debug panel — WS message inspector and development tools.

use dioxus::prelude::*;
use crate::state::{DebugState, DebugTab, WsDirection};

fn format_elapsed(ms: u64) -> String {
    let secs = ms / 1000;
    let mins = secs / 60;
    let hours = mins / 60;
    format!("{:02}:{:02}:{:02}.{:03}", hours, mins % 60, secs % 60, ms % 1000)
}

fn format_json_pretty(raw: &str) -> String {
    if raw.is_empty() { return String::new(); }
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(raw) {
        serde_json::to_string_pretty(&val).unwrap_or_else(|_| raw.to_string())
    } else {
        raw.to_string()
    }
}

fn tab_label(tab: DebugTab) -> &'static str {
    match tab {
        DebugTab::Ws => "WS",
    }
}

#[component]
pub fn DebugPanel() -> Element {
    let mut debug = use_context::<Signal<DebugState>>();
    let guard = debug.read();
    let messages = guard.ws_messages.clone();
    let open = guard.open;
    let active_tab = guard.active_tab;
    drop(guard);

    if !open { return rsx! { div {} }; }

    rsx! {
        div { class: "fixed inset-0 bg-black/50 z-50 flex items-center justify-center p-4",
            div { class: "bg-[#1a1a2e] border border-[#444] rounded-lg flex flex-col shadow-2xl",
                style: "width: 80vw; height: 80vh;",
                div { class: "flex items-center justify-between px-4 py-2 border-b border-[#333] shrink-0",
                    div { class: "flex items-center gap-3",
                        span { class: "text-[#e0e0e0] font-bold text-sm", "Debug Panel" }
                        div { class: "flex gap-1",
                            for tab in [DebugTab::Ws].iter() {
                                let is_active = *tab == active_tab;
                                let tab_cls = if is_active {
                                    "px-3 py-1 text-[12px] font-semibold cursor-pointer border-b-2 border-[#80a0ff] text-[#e0e0e0]"
                                } else {
                                    "px-3 py-1 text-[12px] cursor-pointer text-[#888] hover:text-[#ccc] border-b-2 border-transparent"
                                };
                                button {
                                    class: "{tab_cls}",
                                    onclick: {
                                        let mut d = debug;
                                        let t = *tab;
                                        move |_| { d.write_unchecked().active_tab = t; }
                                    },
                                    {tab_label(*tab)}
                                }
                            }
                        }
                    }
                    button {
                        class: "text-[#888] hover:text-white text-lg leading-none px-1",
                        onclick: {
                            let mut d = debug;
                            move |_| { d.write_unchecked().open = false; }
                        },
                        "×"
                    }
                }
                div { class: "flex-1 overflow-hidden",
                    match active_tab {
                        DebugTab::Ws => rsx! { WsTab { messages } },
                    }
                }
            }
        }
    }
}

#[component]
fn WsTab(messages: Vec<crate::state::WsMessage>) -> Element {
    let mut expanded = use_signal(|| None::<usize>);

    rsx! {
        div { class: "flex flex-col h-full",
            div { class: "flex-1 overflow-y-auto font-mono text-xs",
                if messages.is_empty() {
                    div { class: "flex items-center justify-center h-full text-[#666] text-sm",
                        "No messages yet. Open the panel while the agent is active to capture WS traffic."
                    }
                } else {
                    for (i, msg) in messages.iter().enumerate() {
                        WsMessageRow { index: i, message: msg.clone(), expanded }
                    }
                }
            }
            div { class: "px-3 py-1.5 border-t border-[#333] text-[10px] text-[#666] shrink-0 flex items-center justify-between",
                span { "{messages.len()} messages" }
                span { "Recording is active while panel is open" }
            }
        }
    }
}

#[component]
fn WsMessageRow(index: usize, message: crate::state::WsMessage, expanded: Signal<Option<usize>>) -> Element {
    let is_expanded = *expanded.read() == Some(index);
    let arrow = match message.direction {
        WsDirection::In => "←",
        WsDirection::Out => "→",
    };
    let arrow_color = match message.direction {
        WsDirection::In => "#40c040",
        WsDirection::Out => "#80a0ff",
    };
    let stamp = format_elapsed(message.elapsed_ms);

    rsx! {
        div {
            class: "border-b border-[#222] hover:bg-[#222240] cursor-pointer",
            onclick: {
                let mut e = expanded;
                move |_| { e.with_mut(|s| if *s == Some(index) { *s = None } else { *s = Some(index) }); }
            },
            div { class: "flex items-center gap-2 px-3 py-1.5",
                span { class: "text-[#555] w-[100px] shrink-0", "{stamp}" }
                span { style: "color: {arrow_color}; font-weight: bold;", "{arrow}" }
                span { class: "text-[#ccc] truncate", "{message.method}" }
            }
            if is_expanded {
                div { class: "px-3 pb-2 pl-[120px]",
                    pre { class: "text-[#888] text-[11px] bg-[#111128] rounded p-2 whitespace-pre-wrap break-all max-h-[300px] overflow-y-auto",
                        "{format_json_pretty(&message.payload)}"
                    }
                }
            }
        }
    }
}
```
fn format_elapsed(ms: u64) -> String {
    let secs = ms / 1000;
    let mins = secs / 60;
    let hours = mins / 60;
    format!("{:02}:{:02}:{:02}.{:03}", hours, mins % 60, secs % 60, ms % 1000)
}
```

- [ ] **Step 2: Register `DebugPanel` in `mod.rs`**

Add to `crates/vol-llm-ui/src/web/components/mod.rs`:

```rust
pub mod debug_panel;

// Add to the existing re-exports:
pub use debug_panel::DebugPanel;
```

- [ ] **Step 3: Verify compilation**

```bash
cargo check -p vol-llm-ui --no-default-features --features web
```

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/debug_panel.rs crates/vol-llm-ui/src/web/components/mod.rs
git commit -m "feat(debug): add DebugPanel component with WS tab"
```

---

### Task 4: Add debug button to StatusBar

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/status_bar.rs`

- [ ] **Step 1: Add debug toggle button to StatusBar**

Add `let mut debug = use_context::<Signal<crate::state::DebugState>>();` at the top of the component.

In the right side div (line 73, after the version info), add the debug button:

```rust
div { class: "flex-shrink-0 ml-2",
    button {
        class: {
            let d = debug.read();
            if d.open {
                "text-[11px] px-1.5 py-0.5 rounded-[3px] font-bold bg-[#2a2a44] text-[#c0c040] hover:bg-[#3a3a55] cursor-pointer"
            } else {
                "text-[11px] px-1.5 py-0.5 rounded-[3px] font-bold bg-transparent text-[#555] hover:text-[#888] hover:bg-[#2a2a44] cursor-pointer"
            }
        },
        onclick: move |_| { debug.write_unchecked().toggle(); },
        title: "Debug Panel",
        "🐛"
    }
}
```

- [ ] **Step 2: Verify compilation**

```bash
cargo check -p vol-llm-ui --no-default-features --features web
```

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/status_bar.rs
git commit -m "feat(debug): add debug toggle button to status bar"
```

---

### Task 5: Wire DebugState signal and DebugPanel into App

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/app.rs`

- [ ] **Step 1: Create and provide `DebugState` signal**

After the existing signal declarations (around line 136), add:

```rust
let debug_signal = use_signal(|| DebugState::new());
```

After the existing `use_context_provider` calls (around line 511), add:

```rust
use_context_provider(|| debug_signal);
```

- [ ] **Step 2: Wire debug_state into JsonRpcClient**

After the `JsonRpcClient` is created in `use_hook` (around line 183, after `c` is created), add:

```rust
c.set_debug_state(debug_signal);
```

- [ ] **Step 3: Render DebugPanel**

In the main rsx! block, after the main layout div but before the closing tag, add:

```rust
DebugPanel {}
```

Add the import at the top of the file if not already there:

```rust
use super::debug_panel::DebugPanel;
```

- [ ] **Step 4: Verify compilation**

```bash
cargo check -p vol-llm-ui --no-default-features --features web
```

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/app.rs
git commit -m "feat(debug): wire DebugState and DebugPanel into App"
```

---

### Task 6: Integration verification

- [ ] **Step 1: Full cargo check**

```bash
cargo check -p vol-llm-ui --no-default-features --features web
```

- [ ] **Step 2: Manual test**

1. Start dev server, open in browser
2. Verify 🐛 button visible in status bar
3. Click 🐛 → debug panel opens, recording starts
4. Select an agent and send a message → WS messages appear in panel
5. Click a message row → JSON expands inline
6. Click again → collapses
7. Click × → panel closes (messages preserved)
8. Reopen panel → old messages still visible, new ones appear as they arrive

- [ ] **Step 3: Commit any fixes**

```bash
git add -A && git commit -m "chore: final integration fixes for debug panel"
```
