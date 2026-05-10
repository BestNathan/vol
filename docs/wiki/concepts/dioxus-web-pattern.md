---
type: concept
category: pattern
tags: [dioxus, web, frontend, component, wasm]
created: 2026-05-08
updated: 2026-05-10
source_count: 2
---

# Dioxus Web Pattern

**Category:** Web frontend architecture
**Related:** [[vol-llm-ui-crate]], [[dioxus-signal-pattern]], [[ratatui-tui-pattern]], [[human-in-the-loop]]

## Definition

Component architecture for a browser-based UI built with Dioxus 0.6, compiled to WASM, using RSX macros for declarative rendering and context-based state sharing.

## Key Points

- Dioxus 0.6 via `dioxus::launch(App)` in binary entry point
- Feature gated: `#[cfg(feature = "web")]` in `lib.rs`, binary requires `--features web`
- Components: `App`, `StatusBar`, `ToolsPanel`, `ConversationView`, `InputArea`, `WorkspacePanel`, `SkillsPanel`, `LogViewer`, `SessionDialog`, `ApprovalDialog`, `FileTree`, `ToolsTabContent`, `FileContentView`
- Global CSS embedded as `const GLOBAL_CSS: &str`, injected via `<style>` element
- Dark theme with flexbox layout: status bar (top), tools panel (left), tab content (right), input area (bottom)
- Tab routing: `TabContent` matches on `ActiveTab` enum to render the active panel
- Modal dialogs (`SessionDialog`, `ApprovalDialog`) rendered at root level, internally guard on state
- All components consume shared state via `use_context::<AppState>()`

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

Both frontends share `UiState` / `UiEvent` / `ActiveTab` types and the same connection abstractions. The TUI uses ratatui widgets with terminal-specific rendering; the web uses Dioxus RSX with HTML/CSS rendering. The web frontend has 10 components vs. the TUI's 9 render functions + 1 input handler.

| Aspect | TUI (ratatui) | Web (Dioxus) |
|--------|---------------|--------------|
| Framework | ratatui 0.30 + crossterm 0.29 | Dioxus 0.6 (WASM) |
| Entry point | `main()` with terminal setup | `dioxus::launch(App)` |
| Rendering | Imperative `Frame` drawing | Declarative `rsx!` macros |
| State | `Arc<Mutex<UiState>>` | `Signal<UiState>` |
| Input | crossterm `EventStream` + `tokio::select!` | HTML events (`onclick`, `oninput`) |
| Feature flag | `#[cfg(feature = "tui")]` | `#[cfg(feature = "web")]` |

## Related Concepts
- [[dioxus-signal-pattern]]: State management used by all components
- [[ratatui-tui-pattern]]: Terminal frontend counterpart
- [[human-in-the-loop]]: Approval dialog component implements HITL workflow
- [[vol-llm-ui-crate]]: Shared crate defining state types and connection traits
- [[file-tab-pattern]]: Tabbed file viewer rendered in Workspace tab
