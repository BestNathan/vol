---
name: signal-state-redesign
description: Redesign vol-llm-ui state management to use Dioxus Signal-first architecture for both web and TUI
type: project
---

# Requirements: Signal-First State Management

## Background

The current vol-llm-ui project uses two different state management approaches:
- **Web (WASM)**: `Rc<RefCell<UiState>>` + `Signal<u64>` version counter, manually bumped
- **TUI (native)**: `Arc<Mutex<UiState>>` with 33ms tick-based render

This hybrid approach caused bugs (infinite reconnect loops, missed re-renders) and makes the codebase harder to maintain. The user selected a **Signal-first** architecture where `Signal<UiState>` is the single source of truth for both web and TUI.

## Goals

1. **Single `UiState` type** shared between web and TUI with identical mutation API
2. **Web**: WS events trigger re-renders natively via Dioxus signal subscriptions — no manual version counter
3. **TUI**: subscribes to the same signal, replacing tick-based rendering with event-driven rendering
4. **No panics** from borrow conflicts during concurrent read/write cycles
5. **Existing features work identically** after refactor: file tree, tools tab, conversation, logs, skills, session dialog, approval flow

## Non-Goals

- Multi-agent conversation state (e.g., `HashMap<AgentId, AgentState>`) — out of scope for this redesign
- Changes to the JSON-RPC protocol or server-side code
- New features or UI changes
- TUI visual redesign — same UI, different render trigger

## Scope

**Included:**
- `UiState` struct remains the same fields, just wrapped in `Signal<UiState>`
- Web `App` component: replace `Rc<RefCell<UiState>>` + `Signal<u64>` with `Signal<UiState>`
- TUI main loop: replace `Arc<Mutex<UiState>>` with signal subscription
- Event handler code paths that mutate state (currently `try_borrow_mut().apply(event)`)
- `bump_version()` calls removed — signal mutation handles re-render triggers

**Excluded:**
- Changes to `UiEvent` enum or `UiState::apply()` logic
- JSON-RPC client (`client.rs`) protocol changes
- CSS or visual changes
- File tree, tools tab, or file content view components (they read state, don't mutate it)

## Constraints

1. **WASM compatibility**: All code must compile for `wasm32-unknown-unknown` (web) and native (TUI)
2. **Dioxus 0.6**: Signal API is from `dioxus-signals` crate, available in both contexts
3. **No tokio in WASM**: TUI uses tokio runtime; web uses WASM event loop. Signal must work in both
4. **Thread safety**: TUI runs on native threads; `Signal<T>` in Dioxus uses interior mutability. Need to verify `Signal<UiState>` is `Send + Sync` for TUI use
5. **`UiState` contains `Instant`**: Uses `web_time::Instant` for web, `std::time::Instant` for TUI (cfg-gated). Signal wrapping must preserve this

## Success Criteria

1. Both TUI and Web use `Signal<UiState>` as the single source of truth
2. Web: removing `version` signal and `bump_version()` — all re-renders happen via signal subscriptions automatically
3. TUI: render cycle reads state via `Signal::read()` or `Signal::with()` without blocking
4. No panics from borrow conflicts during concurrent read/write (verified by running web + TUI builds)
5. `cargo build --target wasm32-unknown-unknown` succeeds
6. `cargo build --features tui` succeeds
7. All existing tests pass: `cargo test`
8. Existing features (file tree, tools tab, conversation, logs) work identically

## Edge Cases

1. **Signal read during render cycle**: Dioxus's `Signal::read()` creates a subscription. If the event loop mutates the signal while Dioxus is rendering, the subscription triggers a new render. This is the desired behavior but must not cause infinite loops. Solution: mutations batch into the signal, Dioxus deduplicates re-renders.

2. **TUI without Dioxus runtime**: The TUI binary doesn't use Dioxus's reactive runtime by default. `Signal<T>` from `dioxus-signals` requires a reactive context. Options: (a) run TUI within a minimal Dioxus `VirtualDom` context even without UI, (b) use a standalone signal-like wrapper. This is the key technical decision for the design phase.

3. **Large state updates on every event**: Each WS event mutates `UiState`. With `Signal<UiState>`, field-level updates via `Signal::with_mut()` are preferred over replacing the whole struct. The current `UiState::apply()` already works in-place (`&mut self`), so this maps directly to `signal.with_mut(|state| state.apply(event))`.

4. **Event loop holding signal borrow across await**: The JSON-RPC event loop does `loop { let event = rx.next().await; signal.with_mut(|s| s.apply(event)); }`. This is safe — the borrow is released before the next await point.

5. **File read callbacks**: The `file_read()` callback pattern needs access to the signal to update `open_files` tabs. The callback closure captures the signal clone, calls `signal.with_mut()` to set content/error. Same pattern as event loop.

## Open Questions

1. **TUI reactive context**: How does TUI get a Dioxus signal without a full `VirtualDom`? This will be resolved during the design/brainstorming phase.
2. **Feature flag structure**: Currently `#[cfg(feature = "tui")]` gates TUI-specific code. The signal wrapper may need similar cfg gates if web and TUI use different signal backends.
