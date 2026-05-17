# Frontend Auto-Reconnect Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add WebSocket auto-reconnect with exponential backoff and automatic session restoration on reconnect success.

**Architecture:** When the WebSocket disconnects, `client.rs` launches a `spawn_local` reconnection loop with exponential backoff (3s → 6s → 12s, max 30s). New connections replace the old WebSocket in `ClientInner`. Reconnection state (attempts, delay, countdown) is exposed via `GlobalState`. On reconnect success, `app.rs` calls `session.list` → picks the most recent session → `session.resume` → `session.entries` → rebuilds `conversation_signal`.

**Tech Stack:** Rust, Dioxus web (WASM), web_sys WebSocket, futures_util

---

### Task 1: Add reconnect state to GlobalState and UiEvent

**Files:**
- Modify: `crates/vol-llm-ui/src/state/mod.rs`

- [x] **Step 1: Add reconnection fields to GlobalState**

Add these fields to `GlobalState`:
```rust
    pub reconnect_attempts: u32,
    pub reconnecting: bool,
    pub reconnect_delay_secs: u32,
    pub reconnect_maxed: bool,
```

Update `GlobalState::new()`:
```rust
            reconnect_attempts: 0, reconnecting: false,
            reconnect_delay_secs: 0, reconnect_maxed: false,
```

- [x] **Step 2: Add new UiEvent variants**

Add to `UiEvent` enum:
```rust
    // Reconnection state
    WsReconnecting { attempt: u32, delay_secs: u32 },
    WsReconnectFailed,
    WsReconnected,
```

- [x] **Step 3: Add new UiEventKind variants**

Add to `UiEventKind` enum:
```rust
    WsReconnecting, WsReconnectFailed, WsReconnected,
```

- [x] **Step 4: Update UiEvent::kind() mapping**

Add arms:
```rust
            UiEvent::WsReconnecting { .. } => UiEventKind::WsReconnecting,
            UiEvent::WsReconnectFailed => UiEventKind::WsReconnectFailed,
            UiEvent::WsReconnected => UiEventKind::WsReconnected,
```

- [x] **Step 5: cargo check**

Run: `cd crates/vol-llm-ui && cargo check --features web 2>&1 | head -30`
Expected: PASS

---

### Task 2: Add reconnect() method to JsonRpcClient

**Files:**
- Modify: `crates/vol-llm-ui/src/web/client.rs`

- [x] **Step 1: Add reconnect() method**

Add after the `subscribe()` method in `JsonRpcClient`:

```rust
    /// Attempt to reconnect by creating a new WebSocket.
    /// On success, swaps the inner WebSocket and re-subscribes to agent events.
    /// Returns Ok(()) if the new connection was established, Err if it failed.
    pub fn reconnect(&self) -> Result<(), String> {
        let new_ws = web_sys::WebSocket::new(&self.inner.url)
            .map_err(|e| format!("failed to create WebSocket: {e:?}"))?;

        // Clone inner Rc so we can set up callbacks on the new WS.
        let inner = self.inner.clone();
        let client = self.clone();

        // Message handler — same as original
        let inner_msg = inner.clone();
        let on_msg = Closure::wrap(Box::new(move |e: web_sys::MessageEvent| {
            if let Ok(data) = e.data().dyn_into::<js_sys::JsString>() {
                let data = data.as_string().unwrap();
                Self::handle_message(&inner_msg, &data);
            }
        }) as Box<dyn FnMut(_)>);
        new_ws.set_onmessage(Some(on_msg.as_ref().unchecked_ref()));
        on_msg.forget();

        // Close handler — will trigger another reconnect cycle from app layer
        let inner_close = inner.clone();
        let on_close = Closure::wrap(Box::new(move |_e: web_sys::CloseEvent| {
            inner_close.state.set(ConnectionState::Disconnected);
            if let Some(cb) = inner_close.on_state_change.take() {
                cb(ConnectionState::Disconnected);
                inner_close.on_state_change.set(Some(cb));
            }
        }) as Box<dyn FnMut(_)>);
        new_ws.set_onclose(Some(on_close.as_ref().unchecked_ref()));
        on_close.forget();

        // Open handler
        let inner_open = inner.clone();
        let client_for_open = client.clone();
        let on_open = Closure::wrap(Box::new(move |_e: web_sys::Event| {
            inner_open.state.set(ConnectionState::Connected);
            if let Some(cb) = inner_open.on_state_change.take() {
                cb(ConnectionState::Connected);
                inner_open.on_state_change.set(Some(cb));
            }
            let _ = client_for_open.subscribe();
        }) as Box<dyn FnMut(_)>);
        new_ws.set_onopen(Some(on_open.as_ref().unchecked_ref()));
        on_open.forget();

        // Atomically swap the WebSocket. This is safe because the old WS's
        // callbacks are already forgotten and the new ones are set up.
        inner.ws.set_onmessage(None);
        inner.ws.set_onclose(None);
        inner.ws.set_onopen(None);
        // We can't swap through Cell since ws is not Copy. Instead, use the
        // web_sys WebSocket which can be reassigned via JS. The simplest
        // approach: store ws in a Cell and replace it.
        // Actually, web_sys::WebSocket is not Copy/Clone but can be put in Cell.
        // Current design stores it directly in ClientInner. To swap, we need
        // to replace the field. Since ws is in Rc<ClientInner>, we need Cell<WebSocket>.
        //
        // CHANGE: Make ws: Cell<web_sys::WebSocket> in ClientInner.
        inner.state.set(ConnectionState::Connecting);

        Ok(())
    }
```

Wait — the `ws` field is a plain `web_sys::WebSocket`, not in a `Cell`. Since `web_sys::WebSocket` is not `Copy` but can be moved, we need to change the field type. Let me revise.

**REVISED Step 1: Change ws field to Cell**

Change `ClientInner`:
```rust
struct ClientInner {
    ws: Cell<web_sys::WebSocket>,
    // ... rest unchanged
}
```

Update all accesses from `self.inner.ws` to `self.inner.ws.get()` and `self.inner.ws.set(new_ws)`.

Specifically in:
- `new()`: `Cell::new(ws)` instead of just `ws`
- `send_raw()`: `self.inner.ws.get().send_with_str(msg)`
- Close handler: `inner.ws.set(new_ws)` in reconnect
- `alloc_id`, `pending`, etc. remain unchanged

- [x] **Step 2: Update existing ws accesses in new()**

In `new()`, change:
```rust
        let inner = Rc::new(ClientInner {
            ws: Cell::new(ws),  // was: ws
            url: url.to_string(),
            // ...
        });
```

And in the closure setup:
```rust
        inner.ws.get().set_onmessage(Some(on_msg.as_ref().unchecked_ref()));
        inner.ws.get().set_onclose(Some(on_close.as_ref().unchecked_ref()));
        inner.ws.get().set_onopen(Some(on_open.as_ref().unchecked_ref()));
```

- [x] **Step 3: Update send_raw()**

Change:
```rust
    fn send_raw(&self, msg: &str) -> Result<(), String> {
        self.inner.ws.get().send_with_str(msg).map_err(|e| format!("send failed: {e:?}"))
    }
```

- [x] **Step 4: Implement reconnect() with backoff loop**

Add the reconnect method that spawns an async loop:

```rust
    /// Start automatic reconnection with exponential backoff.
    /// Called when the connection is lost. Does nothing if already reconnecting.
    pub fn start_reconnect(&self, on_attempt: impl Fn(u32, u32) + 'static, on_success: impl Fn() + 'static, on_failed: impl Fn() + 'static) {
        // Already reconnecting — don't spawn another loop.
        if matches!(self.inner.state.get(), ConnectionState::Disconnected) {
            // state is already Disconnected, proceed
        } else {
            return;
        }

        let inner = self.inner.clone();
        let client = self.clone();

        wasm_bindgen_futures::spawn_local(async move {
            // Backoff: 3s, 6s, 12s, 16s, 20s, 24s, 28s, 30s, 30s, 30s
            const MAX_ATTEMPTS: u32 = 10;
            const MIN_DELAY: u64 = 3;
            const MAX_DELAY: u64 = 30;

            for attempt in 1..=MAX_ATTEMPTS {
                let delay = (MIN_DELAY * 2u64.pow(attempt - 1)).min(MAX_DELAY);
                on_attempt(attempt, delay as u32);

                // Wait for the delay
                gloo_timers::future::TimeoutFuture::new(delay * 1000).await;

                // Try to reconnect
                let new_ws = match web_sys::WebSocket::new(&inner.url) {
                    Ok(ws) => ws,
                    Err(e) => {
                        log::warn!("reconnect attempt {attempt} failed to create WS: {e:?}");
                        continue;
                    }
                };

                // Set up callbacks on the new WebSocket
                let inner_cb = inner.clone();
                let client_cb = client.clone();
                let on_msg = Closure::wrap(Box::new(move |e: web_sys::MessageEvent| {
                    if let Ok(data) = e.data().dyn_into::<js_sys::JsString>() {
                        let data = data.as_string().unwrap();
                        Self::handle_message(&inner_cb, &data);
                    }
                }) as Box<dyn FnMut(_)>);
                new_ws.set_onmessage(Some(on_msg.as_ref().unchecked_ref()));
                on_msg.forget();

                let inner_c = inner.clone();
                let on_close = Closure::wrap(Box::new(move |_e: web_sys::CloseEvent| {
                    inner_c.state.set(ConnectionState::Disconnected);
                    if let Some(cb) = inner_c.on_state_change.take() {
                        cb(ConnectionState::Disconnected);
                        inner_c.on_state_change.set(Some(cb));
                    }
                }) as Box<dyn FnMut(_)>);
                new_ws.set_onclose(Some(on_close.as_ref().unchecked_ref()));
                on_close.forget();

                // Wait for open or error — use a oneshot channel
                let (tx, rx) = futures_channel::oneshot::channel::<bool>();
                let inner_o = inner.clone();
                let client_o = client.clone();
                let mut tx_cell = std::cell::Cell::new(Some(tx));
                let on_open = Closure::wrap(Box::new(move |_e: web_sys::Event| {
                    inner_o.state.set(ConnectionState::Connected);
                    if let Some(cb) = inner_o.on_state_change.take() {
                        cb(ConnectionState::Connected);
                        inner_o.on_state_change.set(Some(cb));
                    }
                    let _ = client_o.subscribe();
                    if let Some(sender) = tx_cell.take() {
                        let _ = sender.send(true);
                    }
                }) as Box<dyn FnMut(_)>);
                new_ws.set_onopen(Some(on_open.as_ref().unchecked_ref()));
                on_open.forget();

                let on_err = Closure::wrap(Box::new(move |_e: web_sys::Event| {
                    if let Some(sender) = tx_cell.take() {
                        let _ = sender.send(false);
                    }
                }) as Box<dyn FnMut(_)>);
                new_ws.set_onerror(Some(on_err.as_ref().unchecked_ref()));
                on_err.forget();

                // Wait for result
                match rx.await {
                    Ok(true) => {
                        // Swap the WebSocket
                        inner.ws.set(new_ws);
                        on_success();
                        return;
                    }
                    _ => {
                        log::warn!("reconnect attempt {attempt} failed");
                        continue;
                    }
                }
            }

            // All attempts exhausted
            on_failed();
        });
    }
```

Actually, this is getting complex with the oneshot channel. Let me simplify — instead of waiting for open in the reconnect loop, just swap the WS and let the existing `on_state_change` callback detect success/failure. The simpler approach:

**SIMPLIFIED Step 4: reconnect() just swaps WS, app handles retry loop**

Add to `JsonRpcClient`:

```rust
    /// Reconnect by creating a new WebSocket and swapping the old one.
    /// Returns Ok(()) immediately — success is signaled via on_state_change callback.
    pub fn reconnect(&self) -> Result<(), String> {
        let new_ws = web_sys::WebSocket::new(&self.inner.url)
            .map_err(|e| format!("failed to create WebSocket: {e:?}"))?;

        let inner = self.inner.clone();
        let client = self.clone();

        let inner_msg = inner.clone();
        let on_msg = Closure::wrap(Box::new(move |e: web_sys::MessageEvent| {
            if let Ok(data) = e.data().dyn_into::<js_sys::JsString>() {
                let data = data.as_string().unwrap();
                Self::handle_message(&inner_msg, &data);
            }
        }) as Box<dyn FnMut(_)>);
        new_ws.set_onmessage(Some(on_msg.as_ref().unchecked_ref()));
        on_msg.forget();

        let inner_c = inner.clone();
        let on_close = Closure::wrap(Box::new(move |_e: web_sys::CloseEvent| {
            inner_c.state.set(ConnectionState::Disconnected);
            if let Some(cb) = inner_c.on_state_change.take() {
                cb(ConnectionState::Disconnected);
                inner_c.on_state_change.set(Some(cb));
            }
        }) as Box<dyn FnMut(_)>);
        new_ws.set_onclose(Some(on_close.as_ref().unchecked_ref()));
        on_close.forget();

        let inner_o = inner.clone();
        let client_o = client.clone();
        let on_open = Closure::wrap(Box::new(move |_e: web_sys::Event| {
            inner_o.state.set(ConnectionState::Connected);
            if let Some(cb) = inner_o.on_state_change.take() {
                cb(ConnectionState::Connected);
                inner_o.on_state_change.set(Some(cb));
            }
            let _ = client_o.subscribe();
        }) as Box<dyn FnMut(_)>);
        new_ws.set_onopen(Some(on_open.as_ref().unchecked_ref()));
        on_open.forget();

        // Swap WebSocket — old WS callbacks are cleared, new ones take over
        inner.ws.set(new_ws);
        inner.state.set(ConnectionState::Connecting);

        Ok(())
    }
```

- [x] **Step 5: cargo check**

Run: `cd crates/vol-llm-ui && cargo check --features web 2>&1 | head -30`
Expected: PASS

---

### Task 3: Wire reconnect loop into app.rs on disconnect

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/app.rs`

- [x] **Step 1: Update disconnect handler to start reconnect loop**

In the `use_hook` where `c.on_state_change` is set, update the `Disconnected` branch to start a reconnect loop:

Replace the `on_state_change` callback in the client `use_hook` with:

```rust
        c.on_state_change(move |cs| {
            let event = match cs {
                crate::web::client::ConnectionState::Connected => UiEvent::WsConnected,
                crate::web::client::ConnectionState::Connecting => UiEvent::WsConnecting,
                crate::web::client::ConnectionState::Disconnected =>
                    UiEvent::WsDisconnected { reason: Some("Disconnected".to_string()) },
            };
            bus_conn.publish(&event);
            match cs {
                crate::web::client::ConnectionState::Connected => {
                    let mut g = global_conn.write_unchecked();
                    g.ws_connected = true;
                    g.ws_last_error = None;
                    g.is_running = false;
                    g.run_start = None;
                    g.reconnecting = false;
                    g.reconnect_attempts = 0;
                    g.reconnect_maxed = false;
                }
                crate::web::client::ConnectionState::Connecting => {
                    global_conn.write_unchecked().ws_connected = false;
                }
                crate::web::client::ConnectionState::Disconnected => {
                    let mut g = global_conn.write_unchecked();
                    g.ws_connected = false;
                    g.ws_last_error = Some("Disconnected".to_string());
                    g.is_running = false;
                    // Start reconnect loop if not already reconnecting
                    if !g.reconnecting && !g.reconnect_maxed {
                        g.reconnecting = true;
                        g.reconnect_attempts = 0;
                    }
                }
            }
        });
```

- [x] **Step 2: Add reconnect loop spawn after the client use_hook**

After the client `use_hook` (after `c` is created), spawn a reconnect watcher:

```rust
    // Reconnect loop: watches GlobalState.reconnecting and drives client.reconnect()
    let reconnect_client = client.clone();
    let reconnect_global = global_signal.clone();
    let reconnect_bus = event_bus.with(|eb| eb.clone());
    wasm_bindgen_futures::spawn_local(async move {
        const MAX_ATTEMPTS: u32 = 10;
        const MIN_DELAY: u64 = 3;
        const MAX_DELAY: u64 = 30;

        loop {
            // Wait until reconnecting flag is set
            loop {
                let g = reconnect_global.read();
                if g.reconnecting && !g.reconnect_maxed {
                    break;
                }
                // If reconnect succeeded (reconnecting=false, ws_connected=true), reset
                if g.ws_connected {
                    // Reset in case something set reconnecting
                    let mut gw = reconnect_global.write_unchecked();
                    gw.reconnect_attempts = 0;
                    gw.reconnect_maxed = false;
                }
                gloo_timers::future::TimeoutFuture::new(200).await;
            }

            for attempt in 1..=MAX_ATTEMPTS {
                let delay = (MIN_DELAY * 2u64.pow(attempt - 1)).min(MAX_DELAY);

                // Update state with countdown
                {
                    let mut g = reconnect_global.write_unchecked();
                    g.reconnect_attempts = attempt;
                    g.reconnect_delay_secs = delay as u32;
                }
                reconnect_bus.publish(&UiEvent::WsReconnecting {
                    attempt,
                    delay_secs: delay as u32,
                });

                // Countdown timer — update delay_secs each second
                for remaining in (1..=delay).rev() {
                    {
                        let mut g = reconnect_global.write_unchecked();
                        g.reconnect_delay_secs = remaining as u32;
                    }
                    gloo_timers::future::TimeoutFuture::new(1000).await;

                    // Check if connection was restored externally
                    if reconnect_global.read().ws_connected {
                        return;
                    }
                }

                // Attempt reconnection
                match reconnect_client.reconnect() {
                    Ok(()) => {
                        log::info!("Reconnect attempt {attempt} initiated");
                    }
                    Err(e) => {
                        log::warn!("Reconnect attempt {attempt} failed: {e}");
                    }
                }

                // Wait up to 5 seconds for the connection to establish
                for _ in 0..50 {
                    gloo_timers::future::TimeoutFuture::new(100).await;
                    if reconnect_global.read().ws_connected {
                        // Reconnection succeeded — restore session
                        return;
                    }
                }
            }

            // All attempts exhausted
            {
                let mut g = reconnect_global.write_unchecked();
                g.reconnecting = false;
                g.reconnect_maxed = true;
                g.ws_last_error = Some("Connection lost. Please refresh.".to_string());
            }
            reconnect_bus.publish(&UiEvent::WsReconnectFailed);

            // Exit the outer loop — no more reconnect attempts
            break;
        }
    });
```

- [x] **Step 3: Add session restoration on reconnect success**

Add a separate spawned task that watches for reconnection success and restores the most recent session:

```rust
    // Session restoration on reconnect
    let restore_client = client.clone();
    let restore_global = global_signal.clone();
    let restore_bus = event_bus.with(|eb| eb.clone());
    let restore_conv = conversation_signal.clone();
    wasm_bindgen_futures::spawn_local(async move {
        loop {
            // Wait for reconnection to succeed
            loop {
                let g = restore_global.read();
                if g.ws_connected && g.reconnect_attempts > 0 {
                    // Was reconnecting, now connected — restore session
                    break;
                }
                gloo_timers::future::TimeoutFuture::new(200).await;
            }

            log::info!("Reconnected — restoring most recent session");

            // Fetch session list
            let (tx, rx) = futures_channel::oneshot::channel();
            restore_client.session_list(move |result| {
                let _ = tx.send(result);
            });
            let sessions = match rx.await {
                Ok(Ok(s)) => s,
                _ => {
                    log::warn!("Failed to fetch session list after reconnect");
                    // Reset reconnect state
                    restore_global.write_unchecked().reconnect_attempts = 0;
                    continue;
                }
            };

            if sessions.is_empty() {
                log::info!("No persisted sessions — nothing to restore");
                restore_global.write_unchecked().reconnect_attempts = 0;
                continue;
            }

            // Pick the most recent session (already sorted by time from backend)
            let latest = &sessions[0];
            log::info!("Restoring session: {}", latest.id);

            // Resume the session
            let (tx2, rx2) = futures_channel::oneshot::channel();
            restore_client.session_resume(&latest.id, move |result| {
                let _ = tx2.send(result);
            });
            match rx2.await {
                Ok(Ok(resp)) => {
                    log::info!("Session resumed: {} entries", resp.entry_count);
                }
                _ => {
                    log::warn!("Failed to resume session");
                    restore_global.write_unchecked().reconnect_attempts = 0;
                    continue;
                }
            }

            // Fetch entries and rebuild conversation
            let (tx3, rx3) = futures_channel::oneshot::channel();
            restore_client.session_entries(&latest.id, move |result| {
                let _ = tx3.send(result);
            });
            match rx3.await {
                Ok(Ok(entries)) => {
                    // Convert session entries to conversation entries
                    let conv_entries = crate::web::components::sessions_panel::session_entries_to_conversation(entries);
                    // Rebuild conversation signal
                    {
                        let mut conv = restore_conv.write_unchecked();
                        conv.entries = conv_entries;
                        if conv.auto_scroll {
                            conv.conversation_scroll = 0;
                        }
                    }
                    log::info!("Conversation restored from session");
                }
                _ => {
                    log::warn!("Failed to fetch session entries");
                }
            }

            // Reset reconnect state
            restore_global.write_unchecked().reconnect_attempts = 0;

            // Wait for next disconnect
            loop {
                if !restore_global.read().ws_connected {
                    break;
                }
                gloo_timers::future::TimeoutFuture::new(200).await;
            }
        }
    });
```

- [x] **Step 4: Add gloo-timers dependency**

The crate needs `gloo-timers` for async sleep in WASM. Check `crates/vol-llm-ui/Cargo.toml`:

Run: `grep "gloo-timers" crates/vol-llm-ui/Cargo.toml`

If not present, add to `[dependencies]`:
```toml
gloo-timers = { version = "0.3", features = ["futures"] }
```

Also need `futures-channel` with oneshot:
Run: `grep "futures-channel" crates/vol-llm-ui/Cargo.toml`
If missing `oneshot` feature, update:
```toml
futures-channel = "0.3"
```

- [x] **Step 5: cargo check**

Run: `cd crates/vol-llm-ui && cargo check --features web 2>&1 | head -40`
Expected: PASS or minor borrow/ownership fixes

---

### Task 4: Update StatusBar to show reconnection countdown

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/status_bar.rs`

- [x] **Step 1: Update ConnectionIndicator**

Change the `ConnectionIndicator` component to accept more props and show reconnect state:

```rust
#[component]
fn ConnectionIndicator(connected: bool, error: Option<String>, reconnecting: bool, reconnect_delay: u32, reconnect_maxed: bool) -> Element {
    if reconnect_maxed {
        rsx! {
            span { class: "flex items-center gap-1 mr-1", title: "Connection lost. Please refresh.",
                span { class: "w-2 h-2 rounded-full inline-block flex-shrink-0", style: "background-color: #ff4040;" }
                span { class: "text-[10px] text-[#ff8080]", "No connection" }
            }
        }
    } else if reconnecting {
        rsx! {
            span { class: "flex items-center gap-1 mr-1", title: "Reconnecting...",
                span { class: "w-2 h-2 rounded-full inline-block flex-shrink-0 animate-pulse", style: "background-color: #f0c040;" }
                span { class: "text-[10px] text-[#f0c040]", "Reconnecting... ({reconnect_delay}s)" }
            }
        }
    } else if connected {
        rsx! {
            span { class: "flex items-center gap-1 mr-1", title: "Connected",
                span { class: "w-2 h-2 rounded-full inline-block flex-shrink-0", style: "background-color: #40c040; box-shadow: 0 0 4px #40c040;" }
                span { class: "text-[10px] text-[#888] max-w-[80px] overflow-hidden text-ellipsis whitespace-nowrap", "Connected" }
            }
        }
    } else if let Some(ref err) = error {
        rsx! {
            span { class: "flex items-center gap-1 mr-1", title: "{err}",
                span { class: "w-2 h-2 rounded-full inline-block flex-shrink-0", style: "background-color: #ff4040; animation: conn-blink 1s ease-in-out infinite;" }
                span { class: "text-[10px] text-[#888] max-w-[80px] overflow-hidden text-ellipsis whitespace-nowrap", "Error" }
            }
        }
    } else {
        rsx! {
            span { class: "flex items-center gap-1 mr-1", title: "Connecting...",
                span { class: "w-2 h-2 rounded-full inline-block flex-shrink-0 animate-pulse", style: "background-color: #f0c040;" }
                span { class: "text-[10px] text-[#888] max-w-[80px] overflow-hidden text-ellipsis whitespace-nowrap", "Connecting" }
            }
        }
    }
}
```

- [x] **Step 2: Update StatusBar to pass new props**

In `StatusBar`, read the new fields from GlobalState:

```rust
    let reconnecting = gs.reconnecting;
    let reconnect_delay = gs.reconnect_delay_secs;
    let reconnect_maxed = gs.reconnect_maxed;
```

And update the `ConnectionIndicator` call:
```rust
                ConnectionIndicator { connected: ws_connected, error: ws_error.clone(), reconnecting, reconnect_delay, reconnect_maxed }
```

- [x] **Step 3: cargo check**

Run: `cd crates/vol-llm-ui && cargo check --features web 2>&1 | head -20`
Expected: PASS

---

### Task 5: Build and verify

**Files:** No changes

- [x] **Step 1: Full build**

Run: `make web-build 2>&1 | tail -20`
Expected: WASM build succeeds

- [x] **Step 2: Run clippy**

Run: `make web-clippy 2>&1 | tail -20`
Expected: No warnings

- [x] **Step 3: Verify the existing session panel function is accessible**

The `session_entries_to_conversation` function in `sessions_panel.rs` is currently private (`fn`). Make it `pub(crate)` so `app.rs` can use it:

Modify `crates/vol-llm-ui/src/web/components/sessions_panel.rs`:
Change:
```rust
fn session_entries_to_conversation(entries: Vec<SessionEntry>) -> Vec<ConversationEntry> {
```
To:
```rust
pub(crate) fn session_entries_to_conversation(entries: Vec<SessionEntry>) -> Vec<ConversationEntry> {
```

- [x] **Step 4: Final build verification**

Run: `make web-build 2>&1 | tail -10`
Expected: clean build
