---
type: concept
category: pattern
tags: [dioxus, web, frontend, component, wasm]
created: 2026-05-08
updated: 2026-05-11 (split-signal-state)
source_count: 4
---

# Dioxus Web Pattern

**Category:** Web frontend architecture
**Related:** [[vol-llm-ui-crate]], [[dioxus-signal-pattern]], [[ratatui-tui-pattern]], [[human-in-the-loop]], [[workspace-tree-pattern]], [[event-bus-pattern]]

## Definition

Component architecture for a browser-based UI built with Dioxus 0.6, compiled to WASM, using RSX macros for declarative rendering. **As of 2026-05-11**, state management uses per-component local signals with an [[event-bus-pattern]] for cross-component event routing, replacing the previous centralized `Signal<UiState>`.

## Key Points

- Dioxus 0.6 via `dioxus::launch(App)` in binary entry point
- Feature gated: `#[cfg(feature = "web")]` in `lib.rs`, binary requires `--features web`
- Components: `App`, `StatusBar`, `ToolsPanel`, `ConversationView`, `InputArea`, `WorkspacePanel`, `SkillsPanel`, `LogViewer`, `SessionDialog`, `ApprovalDialog`, `FileTree`, `TreeNode`, `ToolsTabContent`, `FileContentView`, `TabBar`, `TabContent`
- Global CSS embedded as `const GLOBAL_CSS: &str`, injected via `<style>` element
- Dark theme with flexbox layout: status bar (top), tools panel (left), tab content (right), input area (bottom)
- Tab routing: `TabContent` matches on `ActiveTab` enum to render the active panel
- Modal dialogs (`SessionDialog`, `ApprovalDialog`) rendered at root level, internally guard on state
- **State management:** `App()` creates `EventBus` + shared `Signal<GlobalState>` / `Signal<ApprovalUiState>`. Components create local `Signal<T>` and subscribe to specific `UiEventKind`s via `SubscriptionSet` [[event-bus-pattern]]
- **Shared signals** provided via `use_context_provider` — `StatusBar`, `InputArea`, `ApprovalDialog` read them directly
- **Local signals** — `ConversationView`, `ToolsPanel`, `ToolsTabContent` create their own and subscribe to EventBus
- **Context-passed signals** — `FileTree` creates `Signal<WorkspaceState>` and provides via `use_context_provider`; `FileContentView` reads from that context

## AppState Structure

**Current** (post-2026-05-11):
```rust
#[derive(Clone)]
pub struct AppState {
    pub event_bus: EventBus,
    pub rpc_client: JsonRpcClient,
    pub active_tab: Signal<ActiveTab>,
}
```

`App()` also creates and provides via `use_context_provider`:
- `Signal<GlobalState>` — shared run/session/connection info
- `Signal<ApprovalUiState>` — shared HITL approval state

Components that need shared state read these via `use_context::<Signal<T>>()`. Components that own local state create their own `Signal<T>` via `use_signal`.

**Previous** (pre-2026-05-11):
```rust
pub struct AppState {
    pub ui_state: Signal<UiState>,
}
```
All state was centralized in one big signal. Replaced by EventBus pattern [[event-bus-pattern]].

## Component Layout

```
App
├── StatusBar          (agent status, duration, mode)
├── main-layout
│   ├── ToolsPanel     (tool call history, left sidebar)
│   └── right-panel
│       ├── TabBar     (Conversation | Workspace | Skills | Logs)
│       ├── TabContent (routed by ActiveTab)
│       │   ├── ConversationView
│       │   ├── WorkspacePanel
│       │   ├── SkillsPanel
│       │   └── LogViewer
│       └── InputArea  (text input + send button)
├── SessionDialog      (modal overlay)
└── ApprovalDialog     (modal overlay)
```

## FileTree Component

The `FileTree` component renders a `WorkspaceTreeNode` tree in the left sidebar. Each node is a reactive `#[component] TreeNode` — not a plain function — enabling Dioxus reactivity when children are populated via `Signal::with_mut()`. Directories fetch children on-demand via JSON-RPC `file.list`, with a refresh button (⟳) for re-fetching. See [[workspace-tree-pattern]] for the full pattern.

## Build Command

```bash
cargo check -p vol-llm-ui --features web --bin vol-llm-ui-web
```

## Styling Approach

Global CSS is defined as a const string and rendered inline. This avoids external stylesheet dependencies in the WASM build. The dark theme uses a consistent color palette:

- Background: `#1a1a2e`
- Panels: `#252540`, `#2d2d44`
- Borders: `#333355`, `#444466`
- Accent: `#80a0ff` (blue), `#4080ff` (user), `#f0c040` (warning/running)
- Status: `#80c080` (success), `#ff6060` (error), `#888` (skipped)

## Comparison with TUI

Both frontends share `UiState` / `UiEvent` / `ActiveTab` types and the same connection abstractions. The TUI uses ratatui widgets with terminal-specific rendering; the web uses Dioxus RSX with HTML/CSS rendering. The web frontend has 14+ components vs. the TUI's 9 render functions + 1 input handler.

| Aspect | TUI (ratatui) | Web (Dioxus) |
|--------|---------------|--------------|
| Framework | ratatui 0.30 + crossterm 0.29 | Dioxus 0.6 (WASM) |
| Entry point | `main()` with terminal setup | `dioxus::launch(App)` |
| Rendering | Imperative `Frame` drawing | Declarative `rsx!` macros |
| State | `Arc<Mutex<UiState>>` | Per-component `Signal<T>` + EventBus |
| Input | crossterm `EventStream` + `tokio::select!` | HTML events (`onclick`, `oninput`) |
| Feature flag | `#[cfg(feature = "tui")]` | `#[cfg(feature = "web")]` |

## Related Concepts
- [[dioxus-signal-pattern]]: State management used by all components — updated for per-component signals
- [[event-bus-pattern]]: EventBus with UiEventKind routing replacing centralized state
- [[ratatui-tui-pattern]]: Terminal frontend counterpart
- [[human-in-the-loop]]: Approval dialog component implements HITL workflow
- [[vol-llm-ui-crate]]: Shared crate defining state types and connection traits
- [[file-tab-pattern]]: Tabbed file viewer rendered in Workspace tab
- [[workspace-tree-pattern]]: WorkspaceTreeNode tree structure and lazy-loading pattern
- [[lazy-load-dir-tree]]: Source documenting the directory tree implementation
- [[split-signal-state]]: Source documenting the EventBus refactoring
