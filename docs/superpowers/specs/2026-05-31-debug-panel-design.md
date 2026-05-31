# Debug Panel — Design Spec

**Date:** 2026-05-31
**Status:** draft

## Problem

No visibility into WebSocket traffic during development. When something goes wrong (messages not sending, unexpected responses), the developer has to open browser DevTools to inspect WS frames.

## Design

### Debug Button

A debug toggle button in the StatusBar right section:

```
[🐛]  或  [Debug]
```

- Inactive state: subtle icon, clicks to open
- Active state (panel open): highlighted, recording indicator

### Debug Panel

A full-size overlay modal with tab navigation:

```
┌──────────────────────────────────────────────┐
│  Debug Panel              [WS] [MCP] [...]  ×│
├──────────────────────────────────────────────┤
│  Tab content area                            │
│                                              │
└──────────────────────────────────────────────┘
```

- 80vw × 80vh, centered, z-50 overlay
- Left tab bar or top tab bar
- Close button (×) in top-right
- Panel open → recording starts; panel close → recording stops, messages preserved

### Tab 1: WebSocket Messages

Shows all WS messages sent and received while debug mode is active.

Each row:
```
HH:MM:SS.mmm  →  agent.submit       (outgoing, blue arrow)
HH:MM:SS.mmm  ←  agent.event        (incoming, green arrow)
```

- Monospace font, 12px
- Click row → expands inline to show full JSON (pretty-printed)
- Scrollable list, newest at bottom, auto-scroll toggle

### Data Model

```rust
struct WsMessage {
    direction: WsDirection,  // In or Out
    method: String,          // e.g. "agent.submit", "agent.event"
    payload: String,         // full JSON string
    timestamp: Instant,
}

enum WsDirection { In, Out }

struct DebugState {
    enabled: bool,           // true when panel is open
    active_tab: DebugTab,
    ws_messages: Vec<WsMessage>,
}

enum DebugTab { Ws }
```

### Collection Points

`JsonRpcClient`:
- `send_raw(msg)` → push `Out` message (parse method from JSON)
- `handle_message(data)` → push `In` message (parse method/type from JSON)

### Lifetime

- Messages accumulated while `DebugState.enabled` is true
- Panel close → recording stops, messages preserved in memory
- Panel reopen → recording resumes, old messages still visible
- Page refresh → everything cleared

### Files

| File | Change |
|------|--------|
| `state/mod.rs` | Add `DebugState`, `WsMessage`, `WsDirection`, `DebugTab` |
| `web/client.rs` | In `send_raw` and `handle_message`: push to debug state if enabled |
| `web/components/status_bar.rs` | Add debug toggle button |
| `web/components/debug_panel.rs` | New file — panel overlay, tab navigation, WS message list |
| `web/components/app.rs` | Render `DebugPanel` when open, wire `DebugState` signal |

### Edge Cases

- **High message rate**: Vec may grow large. Accept for now — typical dev session < 10k messages.
- **Message JSON parse fails**: Store raw string as payload, method = "unknown".
- **Panel open during reconnect**: New WS messages captured normally; old ones from before reconnect still visible.
