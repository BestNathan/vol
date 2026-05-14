---
type: concept
category: pattern
tags: [mcp, tools, agent, executable-tool]
created: 2026-05-11
---

# MCP Client Integration

**Category:** Integration pattern
**Related:** [[vol-llm-mcp-crate]], [[tool-registry]], [[vol-llm-tool-crate]], [[tool-trait]], [[agent-builder-pattern]], [[react-agent-mcp-integration]], [[mcp-manager-lifecycle]], [[vol-mcp-servers-crate]]

## Definition

Pattern for bridging MCP (Model Context Protocol) server tools into the existing `ExecutableTool` trait system, allowing the ReAct agent to call external MCP tools alongside native Rust tools.

## Key Points

- **McpTool** struct implements `ExecutableTool`, proxying execution to `McpManager`
- **Naming convention:** `mcp__{server_name}_{tool_name}` — double underscore prefix distinguishes MCP tools from native tools
- **Name sanitization:** server names are sanitized to `[a-zA-Z0-9_-]` (regex `[^a-zA-Z0-9_-]` → `_`, consecutive underscores merged, trailing removed)
- **String leaking:** `display_name` and `description` use `Box::leak` to satisfy `ExecutableTool::name() -> &'static str`. Acceptable because tools are registered once at startup.
- **Shared manager:** `Arc<McpManager>` shared across all `McpTool` instances

## How It Works

```
LLM: calls mcp__weather_get_temperature({"location": "Tokyo"})
  ↓
ReActAgent: config.tools.execute(call, ctx)
  ↓
ToolRegistry: lookup("mcp__weather_get_temperature") → McpTool
  ↓
McpTool.execute(args, ctx)
  ↓
McpManager.call_tool("weather", "get_temperature", args)
  ↓
rmcp transport (STDIO stdin/stdout) → child process responds
  ↓
ToolResult::success(content) → injected into ReAct context
```

## ToolRegistry Integration

New method: `register_from_mcp(manager: Arc<McpManager>) -> usize`
- Iterates all connected servers via `manager.list_all_tools().await` (async)
- Creates `McpTool` for each discovered tool
- Registers via `register_boxed()`
- Returns count of tools registered
- Disconnected servers' tools are automatically excluded

## Builder Integration

New method: `AgentConfigBuilder::with_mcp_from_config(working_dir) -> Self`
- Loads and merges `.mcp.json` + `~/.mcp.json` configs
- Creates `McpManager` and connects all MCP servers (failure isolation: skips failed servers)
- Registers all discovered tools into the tool registry
- Stores `Arc<McpManager>` in builder for later cleanup

## Lifecycle

1. **Build time:** `with_mcp_from_config()` creates `McpManager`, connects servers, registers tools
2. **Run time:** Agent calls MCP tools transparently via `ToolRegistry`; `McpManager` handles auto-reconnect in background
3. **Cleanup:** `run()` calls `manager.disconnect().await` after agent loop completes

## Error Handling

| Stage | Error | Handling |
|-------|-------|----------|
| Config parse | JSON invalid | Log error, return empty config, agent continues |
| Server connect | command not found | Log error, skip server, agent continues |
| Tool call | server crashed/disconnected | Returns `McpError::ServerDisconnected`, converted to `ToolError::ExecutionFailed` |

## Timeline

- **2026-05-11**: MCP client integration implemented — McpTool, register_from_mcp, with_mcp_from_config [[react-agent-mcp-integration]]
- **2026-05-11**: Runnable example added demonstrating full flow [[docs-rs-mcp-example]]
- **2026-05-13**: Migrated from `McpSession` to `McpManager` — connection state tracking, auto-reconnect, full MCP protocol [[mcp-manager-impl]]
