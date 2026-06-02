---
type: source
source_type: code
date: 2026-05-14
ingested: 2026-05-14
tags: [vol-llm-ui, connection, dashboard, dioxus, event-bus]
---

# Connection State Dashboard

**Authors/Creators:** team
**Date:** 2026-05-14
**Link:** vol-llm-ui crate, web components

## TL;DR
A new `ConnectionStatePanel` Dioxus component was added to the vol-llm-ui web frontend. It subscribes to the [[event-bus-pattern]] to listen for WebSocket connection status events (`WsConnected`, `WsConnecting`, `WsDisconnected`) published by [[remote-connection-impl]] and renders a color-coded status indicator in the top [[dioxus-web-pattern]] `StatusBar`. Three tests were added to vol-llm-ui.

## Key Takeaways
- `ConnectionStatePanel` is a new Dioxus component that displays real-time connection status
- Uses [[event-bus-pattern]] with `SubscriptionSet` to subscribe to `WsConnected`, `WsConnecting`, `WsDisconnected` event kinds
- Renders a color-coded indicator (green/yellow/red) in the `StatusBar` at the top of the UI
- Integrates with [[remote-connection-impl]] which publishes connection events via the EventBus
- 3 unit tests added covering connected, disconnected, and connecting states
- Adds `ConnectionStatus` to the shared `GlobalState` signal

## Detailed Summary

### Component Architecture

`ConnectionStatePanel` follows the standard Dioxus component pattern established in [[dioxus-web-pattern]]:
- Uses `use_hook` with `SubscriptionSet` for EventBus subscription lifecycle management
- Maintains a local `Signal<ConnectionStatus>` for the current connection state
- Renders inline in the `StatusBar` component alongside agent status and duration

### Event Flow

1. [[remote-connection-impl]] detects WebSocket state changes (connect, disconnect, reconnect attempts)
2. It publishes `UiEvent` with appropriate `UiEventKind` (`WsConnected`, `WsConnecting`, `WsDisconnected`) via the [[event-bus-pattern]]
3. `ConnectionStatePanel` subscribes to these event kinds and updates its local signal
4. The component re-renders with the appropriate color-coded indicator

### Status Indicators
- **Connected** (green, `#80c080`): Active WebSocket connection, showing connected label
- **Connecting** (yellow, `#f0c040`): Reconnection attempt in progress, showing reconnecting label with retry count
- **Disconnected** (red, `#ff6060`): No active connection, showing disconnected label

### Test Coverage

3 tests added to vol-llm-ui:
1. `test_connection_state_connected` — verifies green indicator renders when WsConnected event received
2. `test_connection_state_disconnected` — verifies red indicator renders when WsDisconnected event received
3. `test_connection_state_connecting` — verifies yellow indicator renders when WsConnecting event received

## Entities Mentioned
- [[vol-llm-ui-crate]]: New ConnectionStatePanel component added to web components, GlobalState extended with ConnectionStatus
- [[remote-connection-impl]]: Source of connection status events consumed by the dashboard

## Concepts Covered
- [[event-bus-pattern]]: ConnectionStatePanel subscribes to WsConnected/WsConnecting/WsDisconnected event kinds via SubscriptionSet
- [[dioxus-web-pattern]]: New component follows established Dioxus component patterns with use_hook + Signal + SubscriptionSet
- [[connection-state-dashboard]]: New concept — real-time connection status display via EventBus subscription

## Notes
- The WsConnected, WsConnecting, WsDisconnected event kinds were already defined in UiEventKind as part of the [[split-signal-state]] refactor
- ConnectionStatePanel integrates into StatusBar — the existing StatusBar component was updated to include the new panel
- Connection status is also tracked in GlobalState signal for components that need to read connection state without subscribing to events
