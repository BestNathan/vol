---
type: entity
category: product
tags: [crate, ui, tui, web, rust, frontend]
created: 2026-05-08
updated: 2026-05-23 (per-agent-conversation)
source_count: 21
---

# vol-llm-ui Crate

**Category:** Rust crate — Shared UI state model and connection abstraction, with TUI and Web frontends including FileContentView file tabs

**Related:** [[vol-llm-agent-crate]], [[vol-llm-agent-channel-crate]], [[connection-trait]], [[ratatui-tui-pattern]], [[ui-event-loop-pattern]], [[dioxus-signal-pattern]], [[dioxus-web-pattern]], [[file-tab-pattern]], [[workspace-tree-pattern]], [[event-bus-pattern]], [[sessions-ui-pattern]], [[tailwind-css-migration]], [[connection-state-dashboard]], [[mcp-state-types]], [[schema-form-pattern]], [[skills-panel-json-rpc]], [[drawer-ui-pattern]], [[file-tree-sidebar-scroll-fix]], [[mobile-file-tree-rail]], [[mobile-ui-refinements]], [[file-tree-single-click-expand-fix]], [[file-tree-collapsed-state-follow-up]], [[file-tree-chevron-glyph-refinement]]

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
- Web binary: `vol-llm-ui-web` — Dioxus 0.6 WASM with per-component signals + EventBus [[dioxus-web-pattern]]
- Web components: `App`, `StatusBar`, `ConversationView`, `ToolsPanel`, `InputArea`, `WorkspacePanel`, `SkillsPanel`, `LogViewer`, `ApprovalDialog`, `FileTree`, `ToolsTabContent`, `FileContentView`, `TreeNode`, `TabBar`, `TabContent`, `SessionsPanel`, `AgentsPanel`, `ConnectionStatePanel`, `McpPanel`, `ToolCallDialog`, `SkillDetailDialog` [[task-8-dioxus-web-frontend]], [[task-5-file-content-view]], [[lazy-load-dir-tree]], [[task-6-sessions-tab-wiring]], [[connection-state-dashboard]], [[tool-call-dialog-component]], [[skills-panel-content]]
- `JsonRpcClient` gains `reconnect()` method — swaps internal WebSocket at runtime while preserving pending callbacks and event channels [[frontend-auto-reconnect]]
- Web state: `EventBus` with `UiEventKind` routing, per-component local `Signal<T>`, shared `Signal<GlobalState>` / `Signal<ApprovalUiState>` via `use_context_provider` [[dioxus-signal-pattern]], [[event-bus-pattern]], [[split-signal-state]]
- `SubscriptionSet` with `Drop` impl for automatic cleanup on component unmount [[event-bus-pattern]]
- `ConnectionStatePanel` — real-time connection status indicator in StatusBar, subscribes to WsConnected/WsConnecting/WsDisconnected via EventBus, color-coded (green/yellow/red) [[connection-state-dashboard]]
- `GlobalState` extended with `ConnectionStatus` field for cross-component connection state reads
- `GlobalState` extended with `reconnecting`, `reconnect_attempts`, `reconnect_delay_secs`, `reconnect_maxed` fields for reconnect state [[frontend-auto-reconnect]]
- `ActiveTab` extended with `Mcp` variant (between Skills and Logs), `McpSubtab` enum for server/tools/resources/prompts sub-tabs
- MCP wire types: `McpServerInfo`, `McpToolInfo`, `McpResourceInfo`, `McpResourceTemplateInfo`, `McpPromptInfo`, `McpPromptArgInfo` — all serializable for JSON-RPC
- MCP local state: `McpState` (panel state), `McpServerRowState` (display row), `McpToolCallState` (tool call dialog), `McpResourceViewerState`, `McpPromptViewerState`
- `EventHandler` type: `Box<dyn Fn(&UiEvent) + 'static>` — no `Send + Sync` bounds for WASM [[split-signal-state]]
- Workspace: `WorkspaceTreeNode` tree with lazy-loaded directory children via JSON-RPC `file.list`; discovered child directories remain `loaded: false` until their own children are fetched, render visually collapsed while unloaded, first click loads/expands them, and directory controls use a CSS-drawn chevron affordance [[workspace-tree-pattern]], [[file-tree-single-click-expand-fix]], [[file-tree-collapsed-state-follow-up]], [[file-tree-chevron-glyph-refinement]]

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
- **2026-05-10**: Lazy-loading directory tree — `WorkspaceTreeNode` replaces flat entries, directories fetch children on-demand via `file.list`, every expand re-fetches fresh data, refresh button on each directory, `TreeNode` reactive component pattern [[lazy-load-dir-tree]]
- **2026-05-10**: `FileContentView` added — file tab bar with content preview, `OpenFileTab` state, `render_tab` non-component pattern [[task-5-file-content-view]]
- **2026-05-10**: Lazy-loading directory tree — `WorkspaceTreeNode` replaces flat entries, directories fetch children on-demand via `file.list`, every expand re-fetches fresh data, refresh button on each directory, `TreeNode` reactive component pattern [[lazy-load-dir-tree]]
- **2026-05-11**: Split signal state refactor — centralized `Signal<UiState>` replaced with per-component local signals + `EventBus` with `UiEventKind` routing; `SubscriptionSet` auto-cleanup via `Drop`; shared `GlobalState`/`ApprovalUiState` signals for cross-component reads; `AppState` simplified to `EventBus` + `JsonRpcClient` + `Signal<ActiveTab>`; 43 tests passing [[split-signal-state]]
- **2026-05-11**: Sessions tab wired into App — `SessionsState` signal, `SessionsPanel` replaces `SessionDialog`, Sessions tab button in TabBar, checkpoint CSS added [[task-6-sessions-tab-wiring]]
- **2026-05-12**: Tailwind CSS migration completed — all 16 web component files migrated from `GLOBAL_CSS` to Tailwind v4 utility classes; `GLOBAL_CSS` const (~215 lines) deleted; `input.css` created with custom breakpoints and animations; `rebuild-web.sh` integrates Tailwind CLI; Rust wasm32 build verified; responsive breakpoints added for sidebar and tab bar [[tailwind-css-full-migration]]
- **2026-05-14**: `ConnectionStatePanel` added — EventBus subscriber component rendering color-coded WebSocket connection status (green/yellow/red) in StatusBar; listens to WsConnected/WsConnecting/WsDisconnected event kinds from [[remote-connection-impl]]; `GlobalState` extended with `ConnectionStatus`; 3 tests added [[connection-state-dashboard]]
- **2026-05-14**: `ActiveTab::Mcp` variant added between Skills and Logs; `McpSubtab` enum (Servers, Tools, Resources, Prompts); MCP wire types (`McpServerInfo`, `McpToolInfo`, `McpResourceInfo`, `McpResourceTemplateInfo`, `McpPromptInfo`, `McpPromptArgInfo`) and local state structs (`McpState`, `McpServerRowState`, `McpToolCallState`, `McpResourceViewerState`, `McpPromptViewerState`) added to state module
- **2026-05-15**: `ToolCallDialog` component added — modal dialog for invoking MCP tools with editable JSON arguments, JSON validation, async `mcp_call_tool` RPC call, inline result/error display; uses `let Some(...) else { return rsx!{}; }` early-return pattern for optional dialog state [[tool-call-dialog-component]]
- **2026-05-16**: `McpToolCallState` gained `input_schema: Option<serde_json::Value>` field to carry tool JSON Schema to dialog for future SchemaForm component; debug `console.log` removed from `ToolCard` onclick [[mcp-toolcall-input-schema]]
- **2026-05-16**: `ToolCallDialog` rewritten to use `SchemaForm` component — raw JSON textarea replaced with auto-generated form fields from tool JSON Schema; form state via `use_signal` with `build_form_defaults()` initialization; `use_effect` re-initializes on schema change [[schemaform-toolcall-dialog]]
- **2026-05-16**: Skills panel populated — `SkillsState` gained `error` field, `SkillDialogState` / `SkillDetail` types added, `SkillsPanel` fetches skills on mount via `rpc_client.skill_list()` with error/retry UI, row click opens `SkillDetailDialog` modal showing name/version/scope/triggers/content/file_listing; `SkillDetailDialog` rendered at App root level, dialog signal passed via context [[skills-panel-content]]
- **2026-05-17**: `JsonRpcClient` gains `reconnect()` method — internal WebSocket swapped via `RefCell<WebSocket>`, auto-subscribe preserved; App spawns two `spawn_local` tasks (reconnect watcher with exponential backoff 3s→30s, 10 max retries; session restoration via session.list→session.resume→session.entries); `GlobalState` gains `reconnecting`/`reconnect_attempts`/`reconnect_delay_secs`/`reconnect_maxed` fields; StatusBar shows "Reconnecting... (Xs)" countdown; `UiEvent` gains `WsReconnecting`/`WsReconnectFailed`/`WsReconnected` variants; `gloo-timers` dependency added [[frontend-auto-reconnect]]
- **2026-05-18**: Mobile layout support added — `WorkspaceState` gains `file_tree_drawer_open: bool`; file tree becomes slide-out drawer with backdrop and close button on mobile (`sm:hidden`); StatusBar hides verbose fields; TabBar uses `flex-nowrap overflow-x-auto` with smaller text; dialogs use `w-[95vw]` on mobile; conversation and input area get tighter padding; `file_tree_outer_class()` function and `DESKTOP_SIDEBAR_CLASSES` constant for drawer state management [[mobile-layout-design]]
- **2026-05-18**: FileTree desktop scroll fix — `DESKTOP_SIDEBAR_CLASSES` now uses bounded flex-column layout (`sm:flex sm:h-full sm:min-h-0`) and the tree body uses `min-h-0 flex-1 overflow-y-auto`; directory chevron/refresh controls restyled as compact icon affordances; regression test added [[file-tree-sidebar-scroll-fix]]
- **2026-05-18**: Mobile FileTree rail refinement — mobile closed state now renders an inline `w-10` rail owned by `FileTree`; `App` no longer renders a floating hamburger button; right-side content uses `min-w-0 flex-1` so tabs reserve the rail width; regression tests added [[mobile-file-tree-rail]]
- **2026-05-18**: Mobile UI refinements — FileTree drawer/backdrop are scoped below `StatusBar`, `InputArea` textarea uses mobile-safe `text-[16px]`, and `SkillsPanel` renders mobile cards while keeping the desktop table [[mobile-ui-refinements]]
- **2026-05-18**: FileTree single-click expansion fix — `WorkspaceTreeNode::replace_dir_children()` now inserts discovered child directories with `loaded: false` so their first click loads and expands them instead of first collapsing an empty node; regression test added [[file-tree-single-click-expand-fix]]
- **2026-05-18**: FileTree collapsed-state follow-up — `TreeNode` now treats unloaded empty directories as visually collapsed, first click loads without inserting into `collapsed_dirs`, and directory chevrons use a larger `w-6 h-6 text-[16px]` affordance [[file-tree-collapsed-state-follow-up]]
- **2026-05-18**: FileTree chevron glyph refinement — directory expand/collapse control now uses a CSS-drawn chevron, points right when collapsed, and rotates downward when expanded [[file-tree-chevron-glyph-refinement]]
