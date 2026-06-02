---
type: concept
category: pattern
tags: [connection, status, dashboard, event-bus, dioxus, vol-llm-ui]
created: 2026-05-14
updated: 2026-05-14
source_count: 1
---

# Connection State Dashboard Pattern

**Category:** UI pattern / real-time status display
**Related:** [[vol-llm-ui-crate]], [[event-bus-pattern]], [[dioxus-web-pattern]], [[remote-agent-connection]]

## Definition

A Dioxus component (`ConnectionStatePanel`) that subscribes to WebSocket connection events via the [[event-bus-pattern]] and renders a color-coded connection status indicator in the [[dioxus-web-pattern]] `StatusBar`. Provides real-time visual feedback about the state of the [[remote-agent-connection]] WebSocket.

## Key Points

- `ConnectionStatePanel` is a Dioxus `#[component]` embedded in `StatusBar`
- Uses `use_hook` + `SubscriptionSet` for EventBus subscription lifecycle (auto-cleanup on unmount)
- Subscribes to `UiEventKind::WsConnected`, `UiEventKind::WsConnecting`, `UiEventKind::WsDisconnected`
- Maintains a local `Signal<ConnectionStatus>` for the current state
- Color-coded indicators using Tailwind arbitrary values consistent with the established color palette:
  - **Connected** — green (`#80c080`): active WebSocket connection
  - **Connecting** — yellow (`#f0c040`): reconnection in progress
  - **Disconnected** — red (`#ff6060`): no active connection

## How It Works

### Event Flow

1. [[remote-agent-connection]] detects WebSocket state changes (connect, disconnect, reconnect attempts)
2. It publishes `UiEvent` with the appropriate `UiEventKind` via the [[event-bus-pattern]]
3. `ConnectionStatePanel` receives the event in its handler, updates its local `Signal<ConnectionStatus>`
4. Dioxus re-renders the component with the appropriate color and label

### Component Pattern

```rust
#[component]
fn ConnectionStatePanel() -> Element {
    let app_state: AppState = use_context();
    let connection_status = use_signal(|| ConnectionStatus::Disconnected);

    use_hook(move || {
        let bus = app_state.event_bus.clone();
        let mut set = SubscriptionSet::new(bus.clone());
        for kind in [
            UiEventKind::WsConnected,
            UiEventKind::WsConnecting,
            UiEventKind::WsDisconnected,
        ] {
            set.subscribe(&bus, kind, {
                let connection_status = connection_status.clone();
                move |event| {
                    // Map UiEvent to ConnectionStatus, update signal
                }
            });
        }
        Arc::new(set)
    });

    // Render color-coded indicator based on connection_status.read()
}
```

### ConnectionStatus Type

```rust
pub enum ConnectionStatus {
    Connected,
    Connecting { retry_count: u32 },
    Disconnected,
}
```

## Examples

The component renders inline in the `StatusBar`:
- `StatusBar` renders `ConnectionStatePanel {}` alongside agent status and duration info
- Uses Tailwind classes for background color matching status (green/yellow/red)
- Shows text label: "Connected", "Connecting (attempt N)", "Disconnected"

## Test Coverage

3 tests in vol-llm-ui:
1. `test_connection_state_connected` — WsConnected event triggers green indicator
2. `test_connection_state_disconnected` — WsDisconnected event triggers red indicator
3. `test_connection_state_connecting` — WsConnecting event triggers yellow indicator

## Related Concepts
- [[event-bus-pattern]]: EventBus subscription for WsConnected/WsConnecting/WsDisconnected events
- [[dioxus-web-pattern]]: Component architecture and StatusBar placement
- [[vol-llm-ui-crate]]: Crate containing the component
- [[remote-agent-connection]]: Source of connection status events
- [[connection-state-dashboard]]: Source documenting the implementation
