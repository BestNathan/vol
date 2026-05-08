---
type: entity
category: product
tags: [crate, ui, tui, web, rust, frontend]
created: 2026-05-08
updated: 2026-05-08
source_count: 1
---

# vol-llm-ui Crate

**Category:** Rust crate — Shared UI state model and connection abstraction

**Related:** [[vol-llm-agent-crate]], [[vol-llm-agent-channel-crate]], [[connection-trait]]

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
