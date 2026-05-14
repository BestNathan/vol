---
type: source
source_type: code
date: 2026-05-11
ingested: 2026-05-11
tags: [mcp, example, docs-rs, react-agent, vol-llm-agents]
---

# Docs-RS MCP Integration Example

**Authors/Creators:** vol-llm-agents project
**Date:** 2026-05-11
**Link:** `crates/vol-llm-agents/examples/docs_rs_mcp_example.rs`

## TL;DR

Example demonstrating ReActAgent connecting to the docs-rs MCP server via `with_mcp_from_config()`, searching for the "dioxus" crate and returning its README summary.

## Key Takeaways

- The example shows the complete MCP integration flow: temp directory with `.mcp.json` config, agent build with `with_mcp_from_config()`, tool discovery inspection, and agent execution
- Uses `AnthropicProvider` with `qwen3.6-plus` model via DashScope coding endpoint
- Prints discovered MCP tools (prefixed `mcp__`) before running for visibility
- Agent runs with a Chinese-language prompt: searching dioxus crate and getting README summary
- Temp directory with `.mcp.json` is automatically cleaned up after execution

## Detailed Summary

The example file `docs_rs_mcp_example.rs` lives in `vol-llm-agents/examples/` and serves as a runnable demonstration of the full MCP (Model Context Protocol) integration flow.

The example follows a structured flow:
1. **Prerequisite validation** — checks `ANTHROPIC_AUTH_TOKEN` and `DOCS_RS_MCP_BIN` environment variables
2. **MCP config creation** — writes a `.mcp.json` to a temp directory pointing to the docs-rs-mcp binary
3. **LLM initialization** — creates `AnthropicProvider` with `LLMConfig::with_literal_key()` for the qwen3.6-plus model via DashScope
4. **Agent building** — uses `AgentConfig::builder()` chained with `with_llm()`, `with_system_prompt()`, and `with_mcp_from_config(Some(tmp_dir.path()))`
5. **Tool discovery inspection** — filters the tool registry for `mcp__` prefixed tools and prints them with names and descriptions
6. **Agent execution** — runs with a Chinese query asking to search for the dioxus crate and return its README summary
7. **Result printing** — displays run ID, iteration count, tool call count, and the agent's answer

This example validates that the entire MCP pipeline works end-to-end: config parsing, server connection via rmcp STDIO transport, tool discovery, tool registration into `ToolRegistry`, and transparent tool execution during the ReAct loop.

## Entities Mentioned

- [[vol-llm-agents-crate]]: The crate containing this example
- [[vol-llm-mcp-crate]]: MCP client protocol layer used by the example
- [[vol-mcp-servers-crate]]: Provides the docs-rs-mcp binary the example connects to

## Concepts Covered

- [[mcp-client-integration]]: The example exercises the full MCP client integration flow
- [[mcp-manager-lifecycle]]: Agent build uses McpManager internally via with_mcp_from_config() for connection lifecycle
- [[mcp-example-pattern]]: Pattern for demonstrating MCP tool discovery and execution
- [[docs-rs-tools]]: The example uses docs-rs MCP tools (search_crates, readme)
- [[agent-builder-pattern]]: Example of fluent builder with `with_mcp_from_config()`

## Notes

- Requires `tempfile` and `tracing-subscriber` in dev-dependencies (both present)
- `tool.description` is `Option<String>` — must use `.as_deref().unwrap_or()` for display
- Example compiles cleanly with `cargo check --example docs_rs_mcp_example -p vol-llm-agents`
