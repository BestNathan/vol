---
type: concept
category: pattern
tags: [mcp, tools, agent, executable-tool]
created: 2026-05-11
---

# MCP Client Integration

**Category:** Integration pattern
**Related:** [[vol-llm-mcp-crate]], [[tool-registry]], [[vol-llm-tool-crate]], [[tool-trait]], [[agent-builder-pattern]], [[react-agent-mcp-integration]], [[vol-mcp-servers-crate]]

## Definition

Pattern for bridging MCP (Model Context Protocol) server tools into the existing `ExecutableTool` trait system, allowing the ReAct agent to call external MCP tools alongside native Rust tools.

## Key Points

- **McpTool** struct implements `ExecutableTool`, proxying execution to `McpSession`
- **Naming convention:** `mcp__{server_name}_{tool_name}` — double underscore prefix distinguishes MCP tools from native tools
- **Name sanitization:** server names are sanitized to `[a-zA-Z0-9_-]` (regex `[^a-zA-Z0-9_-]` → `_`, consecutive underscores merged, trailing removed)
- **String leaking:** `display_name` and `description` use `Box::leak` to satisfy `ExecutableTool::name() -> &'static str`. Acceptable because tools are registered once at startup.
- **Shared session:** `Arc<McpSession>` shared across all `McpTool` instances

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
McpSession.call_tool("weather", "get_temperature", args)
  ↓
rmcp transport (STDIO stdin/stdout) → child process responds
  ↓
ToolResult::success(content) → injected into ReAct context
```

## ToolRegistry Integration

New method: `register_from_mcp(session: Arc<McpSession>) -> usize`
- Iterates all connected servers via `session.list_all_tools()`
- Creates `McpTool` for each discovered tool
- Registers via `register_boxed()`
- Returns count of tools registered

## Builder Integration

New method: `AgentConfigBuilder::with_mcp_from_config(working_dir) -> Self`
- Loads and merges `.mcp.json` + `~/.mcp.json` configs
- Connects all MCP servers (failure isolation: skips failed servers)
- Registers all discovered tools into the tool registry
- Stores `Arc<McpSession>` in builder for later cleanup

## Lifecycle

1. **Build time:** `with_mcp_from_config()` connects servers, registers tools
2. **Run time:** Agent calls MCP tools transparently via `ToolRegistry`
3. **Cleanup:** `run()` calls `session.disconnect()` after agent loop completes
   - Uses `Arc::try_unwrap` — if Arc is shared (cloned config across runs), skips gracefully

## Error Handling

| Stage | Error | Handling |
|-------|-------|----------|
| Config parse | JSON invalid | Log error, return empty config, agent continues |
| Server connect | command not found | Log error, skip server, agent continues |
| Tool call | server crashed | Return `ToolResult::failure`, error injected into ReAct context |

## Timeline

- **2026-05-11**: MCP client integration implemented — McpTool, register_from_mcp, with_mcp_from_config [[react-agent-mcp-integration]]
