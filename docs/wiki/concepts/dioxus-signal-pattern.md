---
type: concept
category: pattern
tags: [dioxus, signal, state-management, web, frontend]
created: 2026-05-08
updated: 2026-05-10
source_count: 2
---

# Dioxus Signal Pattern

**Category:** State management
**Related:** [[vol-llm-ui-crate]], [[dioxus-web-pattern]], [[agent-event-stream]]

## Definition

Using Dioxus `Signal<T>` with `use_context_provider` to share mutable state across a component tree, with `write_silent()` for interior mutability and `peek()` for reads within closures.

## Key Points

- `Signal<UiState>` created in root `App` component via `use_signal(|| UiState::new(...))`
- Wrapped in `AppState` struct (implements `Clone + PartialEq`) for context sharing
- `use_context_provider` in `App` provides `AppState` to all descendants
- Child components consume via `use_context::<AppState>()`
- `write_silent()` mutates without triggering reactive subscriptions -- appropriate when the component will re-render anyway on the next frame
- `peek()` reads current value without creating a reactive subscription -- used in closure bodies where `&mut self` would otherwise be required
- `AppState::apply_event(UiEvent)` centralizes all mutations: `self.ui_state.write_silent().apply(event)`

## How It Works

```rust
// Root component
let ui_state = use_signal(|| UiState::new("web-session".into(), "/workspace"));
use_context_provider(|| AppState { ui_state });

// AppState struct
#[derive(Clone, PartialEq)]
pub struct AppState {
    pub ui_state: Signal<UiState>,
}

impl AppState {
    pub fn apply_event(&self, event: UiEvent) {
        self.ui_state.write_silent().apply(event);
    }
}

// Child component
let state: AppState = use_context();
let active = state.ui_state.peek().active_tab;
```

## Comparison with TUI

The TUI frontend uses `Arc<Mutex<UiState>>` for shared state, with explicit locking. The web frontend uses `Signal<UiState>`, which integrates with Dioxus's reactive rendering system. Both frontends share the same `UiState` / `UiEvent` types defined in `vol-llm-ui::state`, enabling the same connection abstractions (`LocalConnection`, `RemoteConnection`) to work with either.

| Aspect | TUI (ratatui) | Web (Dioxus) |
|--------|---------------|--------------|
| State wrapper | `Arc<Mutex<UiState>>` | `Signal<UiState>` |
| Mutation | `state.lock().unwrap().apply(e)` | `state.ui_state.write_silent().apply(e)` |
| Read | `state.lock().unwrap()` | `state.ui_state.peek()` |
| Distribution | Passed explicitly | `use_context_provider` / `use_context` |

## Related Concepts
- [[dioxus-web-pattern]]: Component architecture built on top of this state pattern
- [[agent-event-stream]]: `UiEvent` derived from agent stream events
- [[vol-llm-ui-crate]]: Crate defining `UiState` and `UiEvent`
- [[file-tab-pattern]]: Uses `bump_version()` helper and `peek()/set()` pattern for tab interactions
