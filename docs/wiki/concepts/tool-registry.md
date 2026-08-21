---
type: concept
category: framework
tags: [tools, registry, execution]
created: 2026-05-04
updated: 2026-08-21
source_count: 3
---

# Tool Registry

**Category:** Tool management framework
**Related:** [[react-pattern]], [[agent-plugin-system]], [[vol-llm-tool-crate]], [[mcp-client-integration]], [[tool-trait]], [[cli-style-tool-pattern]]

## Definition

A registry that manages tool definitions and executes tool calls during the Act phase of the ReAct loop.

## Key Points
- Tools are identified by unique string names [[react-agent-docs]]
- Registry provides tool definitions to the LLM for function calling [[react-agent-docs]]
- Execution returns structured results back to the agent [[react-agent-docs]]

## How It Works

The `ToolRegistry` is a `HashMap<String, Arc<dyn ExecutableTool>>` that supports:
1. **Registration**: Tools implement the `ExecutableTool` trait and are registered by name
2. **Definition export**: `definitions()` returns all tool schemas for LLM function calling
3. **Execution**: `execute(call, context)` dispatches to the appropriate tool
4. **MCP registration**: `register_from_mcp(session)` discovers and registers tools from MCP servers [[mcp-client-integration]]
5. **Clone**: Registry implements `Clone` (cheap Arc reference count bumps)
6. **CLI-style tools**: `task` ([[vol-llm-task-crate]]) and `fs` ([[vol-llm-fs-crate]]) register via their `register_cli(registry)` helpers from the runtime builder and coexist with the tools they wrap [[cli-style-tool-pattern]]

The original four built-in tools (`market_data`, `alert_history`, `iv_curve`, `rule_info`),
backed by TDengine market-data tables, were designed in [[agent-tool-design]] but removed
with the volatility pipeline on 2026-08-21 — the registry is populated dynamically (builtin
tools, CLI-style `fs`/`task`, MCP tools) instead.

## Related Concepts
- [[react-pattern]]: Tools are called during the Act phase
- [[agent-plugin-system]]: Plugins can intercept tool call events
- [[tool-trait]]: The trait tools must implement
- [[tool-context]]: Execution context passed to each tool
- [[vol-llm-tool-crate]]: Where the Tool trait and registry are defined
