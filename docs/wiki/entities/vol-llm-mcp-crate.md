---
type: entity
category: product
tags: [crate, mcp, client, rust, rmcp]
created: 2026-05-11
---

# vol-llm-mcp Crate

**Category:** Rust crate — MCP Client protocol layer
**Related:** [[vol-llm-tool-crate]], [[vol-llm-agent-crate]], [[rmcp-sdk]], [[mcp-transport-pattern]], [[mcp-client-integration]], [[react-agent-mcp-integration]]

## Overview

The `vol-llm-mcp` crate provides the MCP (Model Context Protocol) Client protocol layer for the ReAct Agent system. It enables agents to discover, connect to, and invoke tools from external MCP servers configured via `~/.mcp.json` and `.mcp.json` (Claude Desktop schema).

## Architecture

This crate is the **bottom layer** in the dependency graph — it has no dependency on tool or agent crates:

```
vol-llm-mcp → rmcp (MCP protocol)
vol-llm-tool → vol-llm-mcp (McpTool uses McpSession)
vol-llm-agent → vol-llm-mcp (McpSession lifecycle)
```

## Key Modules

| Module | Purpose |
|--------|---------|
| `config.rs` | Parse and merge `.mcp.json` (project) + `~/.mcp.json` (user) |
| `error.rs` | `McpError` enum: ConfigParse, ConnectionFailed, ToolCallFailed, etc. |
| `session.rs` | `McpSession`: connect/disconnect to multiple MCP servers, list_tools, call_tool |

## McpConfig

- `McpConfig::load(working_dir)` — loads from both sources, merges with project-level priority
- Schema: `{"mcpServers": {"name": {"command": "...", "args": [...], "env": {...}}}}`
- Missing config files are not errors — returns empty config

## McpSession

- `McpSession::connect(configs)` — connects to all configured servers, caches tool lists
- Per-server isolation: if one fails, others continue
- `list_all_tools()` — returns `(server_name, McpToolInfo)` pairs from all servers
- `call_tool(server, tool_name, args)` — invokes a tool via rmcp STDIO transport
- `disconnect()` — graceful shutdown of all child processes

## ServerConnection

- Wraps `RunningService<RoleClient, ClientInfo>` from rmcp
- `peer()` returns `&Peer<RoleClient>` for tool operations
- `close()` performs graceful shutdown

## Dependencies

- `rmcp = "1.6"` with features: `client`, `transport-io`, `transport-child-process`
- `tokio`, `serde`, `serde_json`, `thiserror`, `tracing`, `tokio-util`, `dirs`

## Timeline

- **2026-05-11**: Crate created with config parsing, session management, tool discovery/execution [[react-agent-mcp-integration]]
