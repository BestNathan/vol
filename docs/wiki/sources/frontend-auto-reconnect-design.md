---
type: source
source_type: design
date: 2026-05-17
ingested: 2026-05-17
tags: [websocket, reconnect, frontend, dioxus, session-restoration]
---

# Frontend Auto-Reconnect Implementation

**Authors/Creators:** BestNathan
**Date:** 2026-05-17
**Link:** `docs/superpowers/plans/2026-05-17-frontend-auto-reconnect-plan.md`

## TL;DR
Added WebSocket auto-reconnect with exponential backoff (3s → 6s → 12s, max 30s) to the Dioxus web frontend. On reconnect success, the most recent persisted session is automatically restored via `session.list` → `session.resume` → `session.entries`, rebuilding the conversation view without requiring a page refresh.

## Key Takeaways
- `JsonRpcClient` gains `reconnect()` method that swaps the internal WebSocket while preserving pending callbacks and event channels
- `ws` field changed from bare `WebSocket` to `RefCell<WebSocket>` to allow mutation through shared `Rc<ClientInner>`
- Reconnect loop runs as a separate `spawn_local` task watching `GlobalState.reconnecting` flag
- 10 max retry attempts with exponential backoff; countdown displayed in StatusBar
- Session restoration uses existing `session_entries_to_conversation()` converter from SessionsPanel
- Three new UI states: WsReconnecting, WsReconnectFailed, WsReconnected
- `GlobalState` gains `reconnecting`, `reconnect_attempts`, `reconnect_delay_secs`, `reconnect_maxed` fields
- StatusBar shows "Reconnecting... (Xs)" with countdown, "No connection" when exhausted
- `session_entries_to_conversation()` made `pub(crate)` for use from `app.rs`

## Detailed Summary

### Client Layer (`client.rs`)
- `ClientInner.ws` changed to `RefCell<web_sys::WebSocket>` — enables swapping the WebSocket at runtime without breaking `Rc` borrows
- `reconnect()` method creates a new WebSocket, sets up identical onmessage/onclose/onopen handlers, swaps it into `ClientInner`, and sets state to `Connecting`
- The onopen handler auto-subscribes to agent events (existing pattern preserved)

### App Layer (`app.rs`)
- `on_state_change` callback now manages `reconnecting` flag on disconnect
- Reconnect watcher `spawn_local` task: polls `GlobalState.reconnecting`, drives exponential backoff loop (3s, 6s, 12s, 16s, 20s, 24s, 28s, 30s, 30s, 30s), publishes `WsReconnecting` events each second with countdown
- Session restoration `spawn_local` task: waits for `ws_connected && reconnect_attempts > 0`, calls `session.list` → picks first (most recent) → `session.resume` → `session.entries` → rebuilds `conversation_signal`
- Both tasks run in infinite loops, enabling multiple reconnect cycles

### UI Layer (`status_bar.rs`)
- `ConnectionIndicator` component gains `reconnecting`, `reconnect_delay`, `reconnect_maxed` props
- Priority: reconnect_maxed > reconnecting > connected > error > connecting
- "Reconnecting... (Xs)" with animated yellow indicator; "No connection" with solid red

### State Layer (`state/mod.rs`)
- `GlobalState`: 4 new reconnect fields
- `UiEvent`: 3 new variants (`WsReconnecting { attempt, delay_secs }`, `WsReconnectFailed`, `WsReconnected`)
- `UiEventKind`: 3 new variants with corresponding `kind()` mapping
- `UiEvent::apply()` extended to handle new variants (no-op, same as other WS events)

### Dependencies
- `gloo-timers` with `futures` feature added for `TimeoutFuture` (WASM-compatible async sleep)
- `web-sys` features `CloseEvent`, `Event` added

## Entities Mentioned
- [[vol-llm-ui-crate]]: Main crate modified — `client.rs`, `app.rs`, `status_bar.rs`, `state/mod.rs`, `sessions_panel.rs`

## Concepts Covered
- [[dioxus-web-pattern]]: App component gains two new `spawn_local` tasks
- [[event-bus-pattern]]: New WsReconnecting/WsReconnectFailed/WsReconnected event kinds
- [[json-rpc-websocket]]: WebSocket reconnect mechanism via `RefCell` swap
- [[sessions-ui-pattern]]: `session_entries_to_conversation()` reused for session restoration

## Notes
- The `RemoteConnection` in the TUI already has auto-reconnect logic; this adds the same capability to the web `JsonRpcClient`
- Unlike the TUI's jsonrpsee-based reconnect (which reconnects the jsonrpsee `Client`), the web client uses raw `web_sys::WebSocket` with manual callback management
- The session restoration approach is deliberately simple — always restores the first session from `session.list` (most recent by server-side ordering). Multi-session restore could be a future enhancement.
