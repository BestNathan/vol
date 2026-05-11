---
type: source
tags: [mcp, client, agent, tools, react]
created: 2026-05-11
---

# React Agent MCP Integration

**Date:** 2026-05-11
**Status:** Implemented
**Related:** [[vol-llm-mcp-crate]], [[mcp-client-integration]], [[vol-llm-tool-crate]], [[vol-llm-agent-crate]]

## Summary

Implemented MCP (Model Context Protocol) client integration for the ReAct agent system. A new `vol-llm-mcp` crate provides the MCP Client protocol layer, enabling agents to discover and invoke external MCP servers configured via `~/.mcp.json` and `.mcp.json` (Claude Desktop schema).

## Key Design Decisions

- **Dependency direction:** `vol-llm-mcp` is the bottom layer with no dependency on tool/agent crates. `vol-llm-tool` depends on `vol-llm-mcp` (via `McpTool`), and `vol-llm-agent` depends on both.
- **McpTool** implements `ExecutableTool` trait, proxying execution to `McpSession`. Name format: `mcp__{server_name}_{tool_name}` with server name sanitization.
- **McpSession** manages connections to multiple MCP servers via `rmcp` crate (STDIO transport). Created at agent build time, disconnected in `run()` cleanup.
- **Config merge:** `.mcp.json` (project-level) overrides `~/.mcp.json` (user-level) per-key. Missing configs are not errors — agent continues without MCP tools.
- **Connection failure isolation:** If one MCP server fails to connect, it's logged and skipped; other servers and native tools continue working.

## Files Changed

| File | Change |
|------|--------|
| `crates/vol-llm-mcp/` | New crate: config, error, session modules |
| `crates/vol-llm-tool/src/mcp_tool.rs` | McpTool implementing ExecutableTool |
| `crates/vol-llm-tool/src/registry.rs` | Clone impl + `register_from_mcp()` method |
| `crates/vol-llm-agent/src/react/agent.rs` | `mcp_session` field + disconnect in `run()` |
| `crates/vol-llm-agent/src/react/config_builder.rs` | `with_mcp_from_config()` builder method |
| `crates/vol-llm-agent-channel/` | Test fix: added `mcp_session` field |
| `crates/vol-llm-observability/` | Test fix: added `mcp_session` field |

## Implementation Notes

- Uses `rmcp` 1.6 crate with `client` + `transport-io` + `transport-child-process` features
- `ClientInfo.serve_with_ct(transport, cancel_token)` creates the MCP client service
- `Peer<RoleClient>.list_all_tools()` and `Peer<RoleClient>.call_tool()` for tool operations
- `Box::leak` used for `&'static str` in `McpTool` — acceptable because tools are registered once at startup
- `Arc::try_unwrap` used for session disconnect — gracefully skips if Arc is shared across runs
