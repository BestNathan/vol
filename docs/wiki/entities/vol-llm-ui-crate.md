---
type: entity
category: product
tags: [crate, ui, tui, web, rust, frontend]
created: 2026-05-08
updated: 2026-05-10 (lazy-load-dir-tree)
source_count: 3
---

# vol-llm-ui Crate

**Category:** Rust crate — Shared UI state model and connection abstraction

**Related:** [[vol-llm-agent-crate]], [[vol-llm-agent-channel-crate]], [[connection-trait]], [[ratatui-tui-pattern]], [[ui-event-loop-pattern]], [[dioxus-signal-pattern]], [[dioxus-web-pattern]], [[file-tab-pattern]], [[workspace-tree-pattern]]

## Overview

The `vol-llm-ui` crate provides a shared state model (`UiState`, `UiEvent`) and connection abstractions (`AgentConnection`, `FileOperations`) for UI frontends. It supports two connection modes:

- **Local** — in-process `ReActAgent` with `EventObserver` (`LocalConnection`)
- **Remote** — JSON-RPC 2.0 over WebSocket via jsonrpsee (`RemoteConnection`)

Both modes implement the same trait interfaces, so TUI (ratatui) and Web (Dioxus WASM) frontends can switch between local and remote transparently.

## Key Facts
- `UiState` — shared state model for all UI frontends [[remote-connection-impl]]
- `UiEvent` — event enum for agent lifecycle and tool approval [[remote-connection-impl]]
- `AgentConnection` trait — abstracts local vs remote agent interaction [[remote-connection-impl]]
- `FileOperations` trait — abstracts file/log/session access [[remote-connection-impl]]
- `LocalConnection` — in-process agent connection [[remote-connection-impl]]
- `RemoteConnection` — JSON-RPC WebSocket connection with auto-reconnect [[remote-connection-impl]]
- Features: `tui` (default, ratatui + crossterm), `web` (Dioxus WASM)
- TUI binary: `vol-llm-tui` — ratatui 0.30 rendering at 30fps with crossterm event stream [[tui-frontend-ratatui]]
- TUI modules: `render` (9 panel renderers), `input` (keyboard handling with approval/session support) [[ratatui-tui-pattern]]
- Event loop: `tokio::select!` with biased mode prioritizing input over render ticks [[ui-event-loop-pattern]]
- Web binary: `vol-llm-ui-web` — Dioxus 0.6 WASM with Signal<UiState> context [[dioxus-web-pattern]]
- Web components: `App`, `StatusBar`, `ConversationView`, `ToolsPanel`, `InputArea`, `WorkspacePanel`, `SkillsPanel`, `LogViewer`, `SessionDialog`, `ApprovalDialog`, `FileTree`, `ToolsTabContent`, `FileContentView`, `TreeNode` [[task-8-dioxus-web-frontend]], [[task-5-file-content-view]], [[lazy-load-dir-tree]]
- Web state: `Signal<UiState>` via `use_context_provider`, `write_silent()` for interior mutability, `with_mut()` for tree mutations [[dioxus-signal-pattern]]
- Workspace: `WorkspaceTreeNode` tree with lazy-loaded directory children via JSON-RPC `file.list` [[workspace-tree-pattern]]

## Architecture

```
Frontend (TUI/Web)
    ↓
AgentConnection trait ──┬── LocalConnection (ReActAgent + EventObserver)
                        └── RemoteConnection (JSON-RPC WebSocket)
    ↓
FileOperations trait ───┬── LocalConnection (direct filesystem)
                        └── RemoteConnection (JSON-RPC endpoints)
```

## Timeline
- **2026-05-07**: Crate created with state model, hooks, and `LocalConnection`
- **2026-05-08**: `RemoteConnection` added with JSON-RPC 2.0 over WebSocket [[remote-connection-impl]]
- **2026-05-08**: TUI frontend added — ratatui rendering, crossterm event loop, 9 render functions migrated from vol-llm-tui [[tui-frontend-ratatui]]
- **2026-05-08**: Web frontend added — Dioxus 0.6 WASM, 10 components, Signal-based state management [[task-8-dioxus-web-frontend]]
- **2026-05-08**: Final verification passed — 39 tests, all feature builds (tui, web, both) green [[task-10-final-verification]]
- **2026-05-10**: `FileContentView` added — file tab bar with content preview, `OpenFileTab` state, `render_tab` non-component pattern [[task-5-file-content-view]]
- **2026-05-10**: Lazy-loading directory tree — `WorkspaceTreeNode` replaces flat entries, directories fetch children on-demand via `file.list`, every expand re-fetches fresh data, refresh button on each directory, `TreeNode` reactive component pattern [[lazy-load-dir-tree]]
