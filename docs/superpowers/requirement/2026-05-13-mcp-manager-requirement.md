# Requirements: MCP Manager for Connection Lifecycle

## Background

The current `vol-llm-mcp` crate provides `McpSession` which connects to MCP servers at startup, discovers tools, and holds connections until the agent run ends. Issues:
- No connection state tracking — if a server disconnects mid-run, the agent has no way to know or recover
- No automatic reconnection — a crashed child process leaves the agent with dead tool references
- Only supports tools, not the full MCP protocol (resources, prompts)
- `McpSession` is a flat data structure with no lifecycle management

## Goals

1. **`McpManager` replaces `McpSession`** — all downstream code references `McpManager` instead of `McpSession`
2. **Connection state tracking** — each MCP server has a visible state (`Connected`, `Disconnected`, `Connecting`, `Error`)
3. **Automatic reconnection with limits** — disconnected servers auto-retry with configurable max attempts; after exhaustion, stops auto-reconnecting but remains available for manual reconnect
4. **Agent-transparent tool/resource/prompt discovery** — disconnected servers' tools are automatically excluded from discovery results; the agent only sees what's available
5. **Full MCP protocol support** — tools (list + call), resources (list + read + templates), prompts (list + get + complete)
6. **Clean disconnect API** — `McpManager::disconnect()` for graceful shutdown

## Non-Goals

- Do NOT add sampling, completions, or roots protocol support (these are server-facing, not client-facing capabilities we need)
- Do NOT change the existing `ExecutableTool` / `ToolRegistry` interface — adapt internally only
- Do NOT add explicit OTel/Loki integration for connection state — tracing::info! logging is sufficient and captured by the existing observability layer
- Do NOT support dynamic server add/remove at runtime (servers are configured at startup only)

## Scope

**Included:**
- New `McpManager` struct in `vol-llm-mcp/src/manager.rs`
- Per-server connection state enum with atomic tracking
- Background reconnect loop per server (with configurable max retries: default 5, interval: exponential backoff 1s-30s)
- `McpManager` public API:
  - `connect(configs)` — connect all servers
  - `disconnect()` — graceful shutdown
  - `server_status()` — get all server states
  - `reconnect(server_name)` — manual reconnect trigger
  - `list_tools()` — only return tools from connected servers
  - `call_tool(server, tool, args)` — call tool; error if server not connected
  - `list_resources()` / `read_resource(uri)` / `list_resource_templates()` — only from servers that support it
  - `list_prompts()` / `get_prompt(name, args)` / `complete_prompt(name, argument, value)` — only from servers that support it
- Deprecate/remove `McpSession` — update `lib.rs` re-exports
- Update `McpTool` in `vol-llm-tool` to hold `Arc<McpManager>` instead of `Arc<McpSession>`
- Update `ToolRegistry::register_from_mcp()` to accept `Arc<McpManager>`
- Update `AgentConfig` and `AgentConfigBuilder` to use `McpManager` instead of `McpSession`

**Excluded:**
- No HTTP/SSE transport for McpManager (only stdio child process, matching current behavior)
- No dynamic server add/remove after initial connect
- No persistent state (connection state is in-memory only)

### McpError variants

```rust
pub enum McpError {
    ConfigParse { path: String, detail: String },
    ServerNotFound(String),
    ConnectionFailed { server: String, detail: String },
    InitializeTimeout { server: String },
    ToolCallFailed { server: String, tool: String, detail: String },
    ResourceReadFailed { server: String, uri: String, detail: String },
    PromptGetFailed { server: String, name: String, detail: String },
    TransportError(String),
    ServerDisconnected(String), // server_name
}
```

## Cache and Discovery Behavior

- `list_tools()` / `list_resources()` / `list_prompts()` cache results at connect time
- When a server transitions from `Connected` to `Disconnected`, its cached tools/resources/prompts are immediately excluded from discovery results (cache persists in memory but is filtered by server state)
- On successful reconnect, the cache is invalidated and re-fetched
- `call_tool()` / `read_resource()` / `get_prompt()` always query the live server connection — no caching for execution

## Connection Timeout

- Individual connection attempts timeout after 10 seconds per server
- Timeout counts toward the max retry limit (same backoff as connection failure)

## Observability

- Connection state changes emit `tracing::info!` / `tracing::warn!` / `tracing::error!` log events
- Events are structured with server name, state transition, and retry count
- No explicit OTel/Loki integration required — the existing tracing layer captures logs automatically

## Constraints

- Must use existing `rmcp 1.6` crate for MCP protocol (already a dependency)
- Must remain backwards-compatible at the agent config level (same `.with_mcp_from_config()` builder method)
- Background reconnect tasks must be spawned within the `connect()` call, not requiring external event loops
- Must be `Send + Sync` for use across async contexts

## Success Criteria

1. `McpSession` is fully replaced by `McpManager` — no references remain in non-test code
2. `cargo check -p vol-llm-mcp` and `cargo check -p vol-llm-agent` compile without errors
3. All existing `vol-llm-mcp` tests pass (11 tests), plus new tests for:
   - Connection state transitions (Connected → Disconnected → Reconnecting → Connected)
   - Max retry exhaustion (after N failures, no more auto-reconnects)
   - Manual reconnect succeeds after exhaustion
   - Disconnected server tools excluded from `list_tools()`
4. The docs-rs MCP example (`docs_rs_mcp_example.rs`) still works — 4 tools discovered and callable
5. Background reconnect loop spawns within `connect()`, requires no external polling

## Edge Cases

- **Empty config** — `connect([])` should return a valid `McpManager` with zero servers (no-op)
- **Server already connected** — `reconnect()` on a connected server gracefully closes the current connection (same as `disconnect()`) then reconnects
- **Server never connected** — `reconnect()` on a never-connected (initial error) server should retry
- **Concurrent reconnects** — calling `reconnect()` while auto-reconnect is in progress should cancel the in-progress attempt and start fresh
- **Disconnect after exhaustion** — a server that has exhausted retries and been manually disconnected should show `Disconnected` state with retry count at max
- **Partial server failure** — if 3 of 5 servers fail, the manager still operates normally for the 2 connected ones
- **Tool call on disconnected server** — if a server disconnects between tool discovery and tool call, `call_tool()` should return a clear error (not panic or hang)

## Open Questions

- Should the auto-reconnect interval be configurable per-server, or global? → **Default: global config with per-server override support (but not required for v1)**
- Should `list_tools()` cache results or re-query the server each time? → **Default: cache at connect time, invalidate on reconnect**
