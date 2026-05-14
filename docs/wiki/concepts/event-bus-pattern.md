---
type: concept
category: pattern
tags: [event-bus, pub-sub, state-management, dioxus, web, frontend]
created: 2026-05-11
updated: 2026-05-14 (connection-state-dashboard)
source_count: 2
---

# EventBus Pattern

**Category:** State management / event routing
**Related:** [[vol-llm-ui-crate]], [[dioxus-signal-pattern]], [[dioxus-web-pattern]], [[split-signal-state]]

## Definition

A publish-subscribe event bus that routes events by coarse-grained `UiEventKind` using a `HashMap<UiEventKind, Vec<Subscriber>>`. Only handlers subscribed to the matching kind are invoked on `publish()`. `SubscriptionSet` tracks subscriptions per component and auto-cleans via `Drop`.

## Key Points

- `EventBus` stores subscribers behind `Arc<Mutex<HashMap<UiEventKind, Vec<Subscriber>>>>`
- `UiEventKind` is a `Copy + Hash` enum — one variant per `UiEvent` variant family
- `UiEvent::kind()` method maps each concrete event to its kind
- `publish(event)` locks the map, looks up `event.kind()`, calls matching handlers
- `subscribe(kind, handler)` returns a `SubscriptionId`
- `SubscriptionSet` stores `(UiEventKind, SubscriptionId)` pairs; `Drop` removes them from the bus
- `EventHandler` type: `Box<dyn Fn(&UiEvent) + 'static>` — no `Send + Sync` bounds (WASM is single-threaded)
- Component pattern: create `SubscriptionSet` in `use_hook`, return `Arc::new(set)` for Drop-on-unmount cleanup

## EventBus Implementation

```rust
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

#[derive(Clone)]
pub struct EventBus { inner: Arc<EventBusInner> }

impl EventBus {
    pub fn new() -> Self { ... }
    pub fn subscribe<F>(&self, kind: UiEventKind, handler: F) -> SubscriptionId
    where F: Fn(&UiEvent) + 'static { ... }
    pub fn publish(&self, event: &UiEvent) { ... }
}
```

## SubscriptionSet Pattern

```rust
pub struct SubscriptionSet {
    ids: Vec<(UiEventKind, SubscriptionId)>,
    bus: Arc<EventBusInner>,
}

impl SubscriptionSet {
    pub fn new(bus: EventBus) -> Self { ... }
    pub fn subscribe<F>(&mut self, _bus: &EventBus, kind: UiEventKind, handler: F)
    where F: Fn(&UiEvent) + 'static { ... }
}

impl Drop for SubscriptionSet {
    fn drop(&mut self) {
        // Remove all tracked subscriptions
    }
}
```

## Component Usage

```rust
#[component]
fn MyComponent() -> Element {
    let app_state: AppState = use_context();
    let signal = use_signal(|| MyState::new());

    use_hook(move || {
        let bus = app_state.event_bus.clone();
        let mut set = SubscriptionSet::new(bus.clone());
        for kind in [UiEventKind::Foo, UiEventKind::Bar] {
            set.subscribe(&bus, kind, {
                let signal = signal.clone();
                move |event| {
                    // Handle event, mutate signal
                }
            });
        }
        Arc::new(set) // Dropped when component unmounts
    });

    // ... render
}
```

## Why Fn not FnMut

`Fn` closures are simpler — they don't require mutable access to captured variables. `Signal` is `Copy`, so it can be cloned inside the closure. `write_unchecked()` takes `&self`, so it works from `Fn` closures.

## Comparison with Previous Approach

**Before:** Single `Signal<UiState>` in `App`, all components read/borrow from it, `AppState::apply_event(UiEvent)` centralizes all mutations.
**After:** Each component owns its own `Signal<T>`, subscribes to specific event kinds, mutates via local reducer. Shared state (`GlobalState`, `ApprovalUiState`) provided as separate signals via `use_context_provider`.

Benefits:
- No more `borrow()` / `borrow_mut()` conflicts from nested reads
- Components only re-render when their specific signal changes
- Clear ownership: each component manages its own state lifecycle
- TUI continues using `Arc<Mutex<UiState>>` unchanged

## Connection Status Events

The `UiEventKind` enum includes `WsConnected`, `WsConnecting`, and `WsDisconnected` variants for tracking WebSocket connection state. [[remote-connection-impl]] publishes these events when the connection state changes. Components like `ConnectionStatePanel` subscribe to these kinds to display real-time connection status indicators. See [[connection-state-dashboard]] for the full pattern.

## Related Concepts
- [[dioxus-signal-pattern]]: Per-component signals replacing centralized Signal<UiState>
- [[dioxus-web-pattern]]: Component architecture updated for EventBus
- [[split-signal-state]]: Source documenting the full refactoring
- [[agent-event-stream]]: UiEvent derived from agent stream events
- [[connection-state-dashboard-pattern]]: Real-time connection status display via EventBus subscription
