---
type: concept
category: framework
tags: [agent-connection, trait, ui, remote, file-operations]
created: 2026-05-08
updated: 2026-05-08
source_count: 1
---

# AgentConnection and FileOperations Traits

**Category:** UI abstraction layer

**Related:** [[vol-llm-ui-crate]], [[json-rpc-websocket]], [[connection-trait]], [[tui-frontend-ratatui]], [[dioxus-web-pattern]], [[jsonrpc-server-handler]]

## Definition

Two traits in `vol-llm-ui` that abstract agent interaction and file operations for UI frontends. Both traits are implemented by the same concrete type (local or remote), so callers can use a single struct for all operations.

## AgentConnection Trait

Abstract connection to an agent — works identically whether the agent runs in-process (local) or on a remote server (JSON-RPC over WS).

```rust
#[async_trait]
pub trait AgentConnection: Send + Sync {
    async fn submit(&self, input: String) -> anyhow::Result<mpsc::Receiver<UiEvent>>;
    async fn approve_tool(&self, req_id: String, approved: bool, reason: Option<String>) -> anyhow::Result<()>;
    async fn cancel(&self, req_id: String) -> anyhow::Result<()>;
    fn is_connected(&self) -> bool;
}
```

- `submit()` — sends user input, returns a receiver for streaming `UiEvent`s
- `approve_tool()` — responds to tool approval requests
- `cancel()` — stops the current agent run
- `is_connected()` — connection health check

## FileOperations Trait

File, log, and session operations — direct filesystem in local mode, JSON-RPC endpoints in remote mode.

```rust
#[async_trait]
pub trait FileOperations: Send + Sync {
    async fn list_files(&self, path: &str) -> anyhow::Result<Vec<FileEntry>>;
    async fn read_file(&self, path: &str) -> anyhow::Result<String>;
    async fn list_logs(&self) -> anyhow::Result<Vec<LogRunInfo>>;
    async fn list_sessions(&self) -> anyhow::Result<Vec<SessionInfo>>;
}
```

## Implementations

| Type | AgentConnection | FileOperations | Backend |
|------|-----------------|----------------|---------|
| `LocalConnection` | In-process `ReActAgent` via `EventObserver` | Direct filesystem | [[vol-llm-agent-crate]] |
| `RemoteConnection` | JSON-RPC over WebSocket via jsonrpsee | JSON-RPC endpoints (`file.list`, `file.read`, `log.list`, `session.list`) | [[vol-llm-agent-channel-crate]] |

## Supporting Types
- `FileEntry` — file/directory entry: `name`, `is_dir`, `size`
- `LogRunInfo` — log run summary: `run_id`, `timestamp`, `event_count`
- `SessionInfo` — session summary: `session_id`, `entry_count`, `created_at`

## Design Rationale

The trait separation allows UI frontends (TUI [[ratatui-tui-pattern]], Web [[dioxus-web-pattern]]) to work with any connection mode without knowing implementation details. A single `RemoteConnection` struct implements both traits, making it easy to pass around a unified handle.
