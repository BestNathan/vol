---
type: source
tags: [mcp, manager, connection, lifecycle, reconnect, rmcp]
created: 2026-05-13
---

# McpManager Implementation

**Date:** 2026-05-13
**Status:** Implemented
**Related:** [[vol-llm-mcp-crate]], [[mcp-manager-lifecycle]], [[mcp-client-integration]], [[vol-llm-tool-crate]], [[vol-llm-agent-crate]]

## Summary

Replaced `McpSession` with `McpManager` in the `vol-llm-mcp` crate. `McpManager` adds per-server connection state tracking (`Connected`, `Disconnected`, `Connecting`, `Error`), automatic reconnection with exponential backoff (1s-30s, configurable max retries), and full MCP protocol support beyond tools — now includes resources (list + read + templates) and prompts (list + get + complete).

## Key Design Decisions

- **`McpManager` replaces `McpSession`** as the central MCP client coordinator — all downstream code (`vol-llm-tool`, `vol-llm-agent`) now references `McpManager`
- **Internal mutability via `Arc<RwLock<>>`** — all public methods take `&self`, no `&mut self` needed
- **Background reconnect tasks** spawned within `connect()`, each owning a per-server `CancellationToken`. No external event loop required
- **Agent-transparent discovery** — disconnected servers' tools/resources/prompts are automatically excluded from `list_*` results; the agent only sees what's available
- **Full MCP protocol** — tools (list + call), resources (list + read + templates), prompts (list + get + complete)
- **No HTTP/SSE transport** for `McpManager` — only stdio child process, matching existing behavior
- **No explicit OTel/Loki integration** — `tracing::info!`/`tracing::warn!`/`tracing::error!` sufficient, captured by existing observability layer

## Connection State Machine

```
Connecting → Connected → Disconnected (graceful)
Connecting → Error(detail) → (auto-reconnect) → Connecting → ... → Error("max retries exceeded")
Connected → (manual reconnect) → Connecting → Connected
Error("max retries exceeded") → (manual reconnect) → Connecting → ...
```

## Files Changed

| File | Change |
|------|--------|
| `crates/vol-llm-mcp/src/manager.rs` | New file: `McpManager`, `ServerStatus`, `ServerState`, protocol methods, reconnect logic |
| `crates/vol-llm-mcp/src/error.rs` | Added `ResourceReadFailed`, `PromptGetFailed`, `ServerDisconnected` variants |
| `crates/vol-llm-mcp/src/lib.rs` | Exported `McpManager`, `ServerStatus` |
| `crates/vol-llm-tool/src/mcp_tool.rs` | `Arc<McpSession>` → `Arc<McpManager>` |
| `crates/vol-llm-tool/src/registry.rs` | `register_from_mcp(Arc<McpManager>)` |
| `crates/vol-llm-agent/src/react/agent.rs` | `mcp_session` → `mcp_manager` field |
| `crates/vol-llm-agent/src/react/config_builder.rs` | `with_mcp_from_config()` creates `McpManager` |

## Testing

- 14 tests pass in `vol-llm-mcp` (12 existing + 2 new: retry exhaustion, manual reconnect after exhaustion)
- 6 tests pass in `vol-llm-tool`
- 142 tests pass in `vol-llm-agent`
- `cargo check` passes for all three crates

## Implementation Notes

- `connect()` is idempotent — calling twice on already-connected servers is a no-op
- `reconnect()` cancels in-progress attempt via `CancellationToken` before starting fresh
- Retry counter resets on successful reconnect
- `call_tool()` checks live service before calling — returns `ServerDisconnected` if dead, never panics or hangs
- Caches populated at connect time, cleared on disconnect, filtered by state for discovery
- `McpSession` remains in `session.rs` for backwards compatibility but is no longer used by downstream code
