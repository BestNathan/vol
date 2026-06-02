---
type: entity
category: product
tags: [crate, mcp, client, rust, rmcp, multi-transport]
created: 2026-05-11
updated: 2026-05-15
source_count: 3
---

# vol-llm-mcp Crate

**Category:** Rust crate — MCP Client protocol layer
**Related:** [[vol-llm-tool-crate]], [[vol-llm-agent-crate]], [[rmcp-sdk]], [[mcp-transport-pattern]], [[mcp-client-integration]], [[mcp-manager-lifecycle]], [[react-agent-mcp-integration]], [[mcp-multi-transport-config]]

## Overview

The `vol-llm-mcp` crate provides the MCP (Model Context Protocol) Client protocol layer for the ReAct Agent system. It enables agents to discover, connect to, and invoke tools, resources, and prompts from external MCP servers configured via `~/.mcp.json` and `.mcp.json` (Claude Desktop schema).

## Architecture

This crate is the **bottom layer** in the dependency graph — it has no dependency on tool or agent crates:

```
vol-llm-mcp → rmcp (MCP protocol)
vol-llm-tool → vol-llm-mcp (McpTool uses McpManager)
vol-llm-agent → vol-llm-mcp (McpManager lifecycle)
```

## Key Modules

| Module | Purpose |
|--------|---------|
| `config.rs` | Parse and merge `.mcp.json` (project) + `~/.mcp.json` (user); `McpTransport` enum (Stdio/Http) with required `type` field |
| `error.rs` | `McpError` enum: ConfigParse, ConnectionFailed, ToolCallFailed, ResourceReadFailed, PromptGetFailed, ServerDisconnected, TransportError |
| `manager.rs` | `McpManager`: connection lifecycle, state tracking, auto-reconnect, full MCP protocol; dispatches on `McpTransport` for stdio vs HTTP connection |
| `session.rs` | `McpSession`: legacy connection management (retained, stdio-only, no longer used by downstream code) |

## McpManager

`McpManager` is the central MCP client coordinator (replaces `McpSession` for downstream code):
- `McpManager::new(configs)` — creates manager with per-server configs
- `manager.connect()` — connects all servers, caches capabilities, spawns background reconnect on failure
- `manager.disconnect()` / `disconnect_server(name)` — graceful shutdown
- `manager.reconnect(name)` — manual reconnect, resets retry counter
- `manager.server_status()` / `server_status_async()` — per-server `ServerStatus` map
- `manager.list_all_tools()` — returns `(server_name, McpToolInfo)` pairs from Connected servers only
- `manager.call_tool(server, tool, args)` — invokes tool; errors if server not Connected
- `manager.list_all_resources()` / `read_resource(uri)` / `list_all_resource_templates()` — resource protocol
- `manager.list_all_prompts()` / `get_prompt(name, args)` / `complete_prompt(name, arg, value)` — prompt protocol

## ServerStatus & ServerState

`ServerStatus` enum: `Connected`, `Disconnected`, `Connecting`, `Error(String)`
`ServerState` tracks per-server: config, status, retry_count, running_service, cancel_token, cached tools/resources/templates/prompts, reconnect handle

## McpTransport Enum

`McpTransport` replaces the flat `command`/`args`/`env` fields on `McpServerConfig`:

```rust
pub enum McpTransport {
    Stdio { command: String, args: Vec<String>, env: HashMap<String, String> },
    Http { url: String, headers: Option<HashMap<String, String>> },
}
```

Parsing uses serde's internally-tagged enum (`#[serde(tag = "type")]`). The `type` field is required — values `"stdio"` and `"http"` are recognized; others are skipped with a warning.

## Timeline

- **2026-05-11**: Crate created with config parsing, session management, tool discovery/execution [[react-agent-mcp-integration]]
- **2026-05-13**: `McpManager` added — connection lifecycle, auto-reconnect, full MCP protocol (resources, prompts) [[mcp-manager-impl]]
- **2026-05-15**: Multi-transport config — `McpTransport` enum (Stdio/Http) with required `type` field; HTTP via `StreamableHttpClientTransport` [[mcp-multi-transport-config]]
