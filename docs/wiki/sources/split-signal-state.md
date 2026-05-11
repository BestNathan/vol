---
type: source
source_type: report
date: 2026-05-11
ingested: 2026-05-11
tags: [refactor, state-management, event-bus, dioxus, web, frontend]
---

# Split Signal State — EventBus Architecture

**Authors/Creators:** Claude Code (vol-llm-ui team)
**Date:** 2026-05-11
**Link:** Plan at `docs/superpowers/plans/2026-05-11-split-signal-state-impl.md`

## TL;DR

Replaced centralized `Signal<UiState>` with per-component local signals and a typed `EventBus` with `UiEventKind` routing. Components now own their own state — some create local `Signal<T>` and subscribe to specific event kinds, others read shared signals (`GlobalState`, `ApprovalUiState`) via `use_context`. `AppState` simplified to hold only `EventBus`, `JsonRpcClient`, and `Signal<ActiveTab>`.

## Key Takeaways

- `EventBus` routes events by `UiEventKind` enum — `publish()` only invokes handlers matching the event's kind
- `SubscriptionSet` tracks subscriptions per component, auto-cleans via `Drop` impl
- `UiEvent::kind()` maps each variant to its coarse-grained `UiEventKind`
- Shared signals (`GlobalState`, `ApprovalUiState`) created in `App()` and provided via `use_context_provider` for cross-component reads
- Local signals (`ConversationState`, `ToolState`, `WorkspaceState`, etc.) created per-component with `use_signal`
- EventBus handlers in `App()` update shared signals using `write_unchecked()` (takes `&self`, works from `Fn` closures)
- Component reducers (`reduce_conversation`, `reduce_tool_state`) pattern: match on `UiEvent` variants, mutate local signal
- `EventHandler` type changed from `Box<dyn Fn(&UiEvent) + Send + Sync>` to `Box<dyn Fn(&UiEvent) + 'static>` — WASM is single-threaded, `Send+Sync` unnecessary
- `ConversationEntry` gained `PartialEq` derive for Dioxus component macro
- `dioxus-signals` added as explicit dependency in Cargo.toml
- All builds pass: web (`--features web`), TUI (`--features tui`), 43 tests green

## Detailed Summary

### EventBus Architecture

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
```

`EventBus` stores `HashMap<UiEventKind, Vec<Subscriber>>` behind `Arc<Mutex>`. `publish(event)` locks the map, looks up `event.kind()`, and calls matching handlers.

`SubscriptionSet` tracks `(UiEventKind, SubscriptionId)` pairs. On `Drop`, it removes all tracked subscriptions from the bus — no manual cleanup needed.

### AppState Simplified

**Before:** `AppState { ui_state: Signal<UiState> }` — all state in one big signal
**After:** `AppState { event_bus: EventBus, rpc_client: JsonRpcClient, active_tab: Signal<ActiveTab> }`

### Component State Ownership

| Component | State Source | Subscriptions |
|-----------|-------------|---------------|
| `App()` | Creates `EventBus`, `Signal<GlobalState>`, `Signal<ApprovalUiState>` | Updates shared signals via EventBus handlers |
| `StatusBar` | `use_context::<Signal<GlobalState>>()` | None — reads shared signal |
| `InputArea` | `use_context::<Signal<GlobalState>>()` + `use_context::<Signal<ApprovalUiState>>()` | None — reads shared signals |
| `ApprovalDialog` | `use_context::<Signal<ApprovalUiState>>()` | None — reads shared signal |
| `ConversationView` | `use_signal(|| ConversationState::new())` | 13 event kinds (agent lifecycle, thinking, content, iteration) |
| `ToolsPanel` | `use_signal(|| ToolState::new())` | 4 event kinds (ToolCallBegin/Complete/Error/Skipped) |
| `ToolsTabContent` | `use_signal(|| ToolState::new())` | 4 event kinds (same as ToolsPanel) |
| `FileTree` | `use_signal(|| WorkspaceState::new())` + `use_context_provider` | None — local state only |
| `FileContentView` | `use_context::<Signal<WorkspaceState>>()` | None — reads from FileTree's context |
| `SkillsPanel` | `use_signal(|| SkillsState::new())` | None — local state |
| `SessionDialog` | `use_signal(|| SessionDialogState::new())` | None — local state |
| `LogViewer` | `use_signal(|| LogViewerState::new())` | None — local state |

### File Changes

- `state/mod.rs`: Added `UiEventKind` enum, `UiEvent::kind()`, `EventBus`, `SubscriptionSet`, `HasReducer` trait, per-component state structs (`GlobalState`, `ConversationState`, `ToolState`, `WorkspaceState`, `SkillsState`, `LogViewerState`, `SessionDialogState`, `ApprovalUiState`)
- `web/client.rs`: Added `url()` method to `JsonRpcClient`
- `web/components/app.rs`: Rewritten — creates EventBus + shared signals, WS event loop publishes to bus
- `web/components/conversation.rs`: Local signal + `reduce_conversation` reducer
- `web/components/tools_panel.rs`: Local signal + `reduce_tool_state` reducer
- `web/components/tools_tab.rs`: Local signal + expandable items, `reduce_tool_state` reducer
- `web/components/status_bar.rs`: Reads `Signal<GlobalState>` from context
- `web/components/input_area.rs`: Reads shared signals from context
- `web/components/approval_dialog.rs`: Reads `Signal<ApprovalUiState>` from context
- `web/components/file_tree.rs`: Creates local `WorkspaceState`, provides via context
- `web/components/file_content.rs`: Reads `WorkspaceState` from context
- `web/components/skills.rs`, `session_dialog.rs`, `log_viewer.rs`: Local signals

### Build Verification

- `cargo check -p vol-llm-ui --no-default-features --features web --bin vol-llm-ui-web` — passes
- `cargo check -p vol-llm-ui --features tui --bin vol-llm-tui` — passes
- `cargo test -p vol-llm-ui` — 43 passed, 0 failed

## Entities Mentioned
- [[vol-llm-ui-crate]]: The crate receiving the EventBus refactoring

## Concepts Covered
- [[event-bus-pattern]]: EventBus with UiEventKind routing and SubscriptionSet auto-cleanup
- [[dioxus-signal-pattern]]: Updated from centralized Signal<UiState> to per-component signals
- [[dioxus-web-pattern]]: Updated component architecture
- [[split-signal-state]]: This source — full refactoring documentation
