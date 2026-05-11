---
type: concept
category: pattern
tags: [dioxus, signal, state-management, web, frontend]
created: 2026-05-08
updated: 2026-05-11 (split-signal-state)
source_count: 4
---

# Dioxus Signal Pattern

**Category:** State management
**Related:** [[vol-llm-ui-crate]], [[dioxus-web-pattern]], [[agent-event-stream]], [[event-bus-pattern]]

## Definition

Using Dioxus `Signal<T>` with `use_context_provider` to share mutable state across a component tree. **As of 2026-05-11**, the pattern evolved from a single centralized `Signal<UiState>` to per-component local signals with an [[event-bus-pattern]] for cross-component event routing. Shared signals (`GlobalState`, `ApprovalUiState`) are still provided via `use_context_provider` for cross-component reads.

## Key Points

- **Current architecture:** Per-component local `Signal<T>` created via `use_signal(|| ...)` in each component
- Shared signals (`Signal<GlobalState>`, `Signal<ApprovalUiState>`) created in `App()` and provided via `use_context_provider`
- `write_unchecked()` mutates from `Fn` closures (takes `&self`), used in EventBus handlers
- `with_mut(|s| ...)` mutates state and auto-triggers re-renders — used for tree mutations in `FileTree` [[workspace-tree-pattern]]
- `use_hook` returns `Arc::new(SubscriptionSet)` for automatic cleanup on component unmount
- Components that don't need EventBus subscriptions simply `use_context::<Signal<T>>()` to read shared state

## How It Works — Current Architecture

```rust
// Root component — creates EventBus + shared signals
let event_bus = use_signal(|| EventBus::new());
let global_signal = use_signal(|| GlobalState::new(ws_url.clone()));
let approval_signal = use_signal(|| ApprovalUiState::new());
use_context_provider(|| global_signal);
use_context_provider(|| approval_signal);

// Child component with local signal + EventBus subscriptions
let app_state: AppState = use_context();
let signal = use_signal(|| ConversationState::new());
use_hook(move || {
    let bus = app_state.event_bus.clone();
    let mut set = SubscriptionSet::new(bus.clone());
    for kind in [UiEventKind::AgentStart, ...] {
        set.subscribe(&bus, kind, {
            let signal = signal.clone();
            move |event| { reduce_conversation(&mut *signal.write_unchecked(), event); }
        });
    }
    Arc::new(set)
});

// Child component reading shared signal
let g: Signal<GlobalState> = use_context();
let gs = g.read();
```

## How It Worked — Previous Architecture (pre-2026-05-11)

```rust
// Root component — single centralized signal
let ui_state = use_signal(|| UiState::new("web-session".into(), "/workspace"));
use_context_provider(|| AppState { ui_state });

// AppState struct provided apply_event for centralized mutations
impl AppState {
    pub fn apply_event(&self, event: UiEvent) {
        self.ui_state.write_silent().apply(event);
    }
}

// Child component consumed via use_context
let state: AppState = use_context();
let active = state.ui_state.peek().active_tab;
```

This approach was replaced by the EventBus pattern [[event-bus-pattern]] to eliminate borrow conflicts, reduce unnecessary re-renders, and give each component clear ownership of its state.

## Comparison with TUI

The TUI frontend uses `Arc<Mutex<UiState>>` for shared state, with explicit locking. The web frontend now uses per-component `Signal<T>` with EventBus routing. Both frontends share the same `UiState` / `UiEvent` types defined in `vol-llm-ui::state`, enabling the same connection abstractions (`LocalConnection`, `RemoteConnection`) to work with either.

| Aspect | TUI (ratatui) | Web (Dioxus) |
|--------|---------------|--------------|
| State wrapper | `Arc<Mutex<UiState>>` | Per-component `Signal<T>` + EventBus |
| Mutation | `state.lock().unwrap().apply(e)` | `signal.write_unchecked()` in EventBus handlers |
| Read | `state.lock().unwrap()` | `signal.read()` / `use_context::<Signal<T>>()` |
| Distribution | Passed explicitly | `use_context_provider` / `use_context` |

## Related Concepts
- [[event-bus-pattern]]: EventBus with UiEventKind routing replacing centralized state
- [[dioxus-web-pattern]]: Component architecture built on top of this state pattern
- [[agent-event-stream]]: `UiEvent` derived from agent stream events
- [[vol-llm-ui-crate]]: Crate defining `UiState` and `UiEvent`
- [[split-signal-state]]: Source documenting the full refactoring
