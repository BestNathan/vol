---
type: concept
category: pattern
tags: [websocket, reconnect, dioxus, wasm, exponential-backoff, session-restoration]
created: 2026-05-17
updated: 2026-05-17
source_count: 1
---

# Frontend Auto-Reconnect

**Category:** Web frontend reliability pattern

**Related:** [[vol-llm-ui-crate]], [[dioxus-web-pattern]], [[json-rpc-websocket]], [[event-bus-pattern]], [[sessions-ui-pattern]]

## Definition

WebSocket auto-reconnect with exponential backoff and automatic session restoration on the Dioxus web frontend. When the WebSocket connection drops, a spawned async task drives reconnection attempts with increasing delays (3s → 6s → 12s, capped at 30s). On success, the most recent persisted session is automatically restored via `session.list` → `session.resume` → `session.entries`, rebuilding the conversation view without requiring a page refresh.

## Key Points

- **Max retries:** 10 attempts, then stops and displays "Connection lost. Please refresh."
- **Backoff formula:** `min(3 * 2^(attempt-1), 30)` seconds — delays: 3, 6, 12, 16, 20, 24, 28, 30, 30, 30
- **Countdown display:** StatusBar shows "Reconnecting... (Xs)" updating every second
- **Session restoration:** Always picks the first (most recent) session from `session.list`
- **Reusable converter:** Uses `session_entries_to_conversation()` from [[sessions-ui-pattern]]

## How It Works

### Client Layer (`client.rs`)

The `JsonRpcClient` stores its WebSocket in a `RefCell<web_sys::WebSocket>` inside `ClientInner` (shared via `Rc`). This allows swapping the WebSocket at runtime without breaking existing borrows.

```rust
struct ClientInner {
    ws: RefCell<web_sys::WebSocket>,
    // ... other fields
}
```

The `reconnect()` method:
1. Creates a new `WebSocket` to the same URL
2. Sets up identical `onmessage`, `onclose`, `onopen` handlers
3. Swaps the new WebSocket into `ClientInner` via `*inner.ws.borrow_mut() = new_ws`
4. Sets state to `Connecting`
5. The `onopen` handler auto-subscribes to agent events (existing pattern preserved)

### Reconnect Watcher (`app.rs`)

A `spawn_local` async task watches the `GlobalState.reconnecting` flag:

```rust
loop {
    // Wait until reconnecting flag is set
    while !global.read().reconnecting { sleep(200ms); }

    for attempt in 1..=10 {
        let delay = exponential_backoff(attempt);

        // Countdown: update delay_secs each second
        for remaining in (1..=delay).rev() {
            global.write().reconnect_delay_secs = remaining;
            bus.publish(WsReconnecting { attempt, delay_secs: remaining });
            sleep(1000ms);
            if global.read().ws_connected { return; }  // restored externally
        }

        client.reconnect()?;

        // Wait up to 5s for connection to establish
        for _ in 0..50 {
            sleep(100ms);
            if global.read().ws_connected { return; }
        }
    }

    // All attempts exhausted
    global.write().reconnect_maxed = true;
    bus.publish(WsReconnectFailed);
    break;
}
```

### Session Restoration (`app.rs`)

A second `spawn_local` task watches for reconnection success:

```rust
loop {
    // Wait for ws_connected && reconnect_attempts > 0
    while !(global.read().ws_connected && global.read().reconnect_attempts > 0) {
        sleep(200ms);
    }

    // session.list → pick first → session.resume → session.entries
    let sessions = client.session_list().await?;
    let latest = &sessions[0];
    client.session_resume(&latest.id).await?;
    let entries = client.session_entries(&latest.id).await?;

    // Convert and rebuild conversation signal
    let conv = session_entries_to_conversation(entries);
    conversation_signal.write().entries = conv;
}
```

### UI States

| State | StatusBar Display | Indicator |
|-------|-------------------|-----------|
| Connected | "Connected" | Green dot |
| Connecting | "Connecting" | Yellow pulse |
| Disconnected | "Error" | Red blink |
| Reconnecting | "Reconnecting... (Xs)" | Yellow pulse |
| Maxed out | "No connection" | Red solid |

### State Fields

`GlobalState` additions:
- `reconnecting: bool` — true when reconnect loop is active
- `reconnect_attempts: u32` — current attempt number (0 = not reconnecting)
- `reconnect_delay_secs: u32` — seconds until next attempt (counts down)
- `reconnect_maxed: bool` — true when all 10 attempts exhausted

### New Event Variants

```rust
UiEvent::WsReconnecting { attempt: u32, delay_secs: u32 }
UiEvent::WsReconnectFailed
UiEvent::WsReconnected
```

### ConnectionIndicator Component

Priority order in `ConnectionIndicator`:
1. `reconnect_maxed` → red "No connection"
2. `reconnecting` → yellow "Reconnecting... (Xs)" with countdown
3. `connected` → green "Connected"
4. `error` → red blinking "Error"
5. default → yellow pulsing "Connecting"

## Dependencies

- `gloo-timers` with `futures` feature — provides `TimeoutFuture` for WASM-compatible async sleep
- `web-sys` features: `CloseEvent`, `Event` — needed for WebSocket lifecycle callbacks

## Related Concepts

- [[json-rpc-websocket]]: WebSocket protocol that this reconnects
- [[event-bus-pattern]]: New event kinds published during reconnect lifecycle
- [[dioxus-web-pattern]]: App component spawns reconnect and restoration tasks
- [[sessions-ui-pattern]]: Session restoration reuses entry converter
- [[mcp-manager-lifecycle]]: Server-side MCP manager has similar auto-reconnect with backoff
- [[remote-agent-connection]]: TUI's `RemoteConnection` has similar auto-reconnect (jsonrpsee-based)
