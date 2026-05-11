---
type: concept
category: pattern
tags: [mcp, example, documentation, integration-testing]
created: 2026-05-11
updated: 2026-05-11
source_count: 1
---

# MCP Example Pattern

**Category:** Documentation and integration testing pattern
**Related:** [[mcp-client-integration]], [[agent-builder-pattern]], [[docs-rs-mcp-example]]

## Definition

Pattern for creating runnable example files that demonstrate MCP (Model Context Protocol) integration with ReActAgent, serving as both documentation and integration tests.

## Key Points

- **Self-contained example** — single file demonstrates the complete flow: config creation, LLM init, agent build, tool discovery, execution, and result printing
- **Temp directory for MCP config** — uses `tempfile::tempdir()` with a `.mcp.json` file, avoiding persistent state or requiring real user config files
- **Tool discovery visibility** — filters and prints discovered MCP tools (`mcp__` prefix) before running, making it clear which tools are available
- **Environment variable prerequisites** — validates `ANTHROPIC_AUTH_TOKEN` and binary paths upfront with helpful error messages
- **Run as example** — lives in `examples/` directory of the crate, runnable via `cargo run --example <name> -p <crate>`

## How It Works

```
1. Validate env vars (API token, binary paths)
2. Create temp dir with .mcp.json
   └── mcpServers: { "docs-rs": { command: binary_path, args: [], env: {} } }
3. Initialize LLM provider (AnthropicProvider via DashScope)
4. Build agent
   ├── AgentConfig::builder()
   ├── .with_llm(Arc::new(llm))
   ├── .with_system_prompt("...")
   └── .with_mcp_from_config(Some(tmp_dir.path())).await
5. Inspect discovered MCP tools
   └── Filter registry.definitions() for "mcp__" prefix
6. Run agent with query
7. Print results (run_id, iterations, tool_calls, content)
8. Temp dir auto-cleaned on drop
```

## Examples / Applications

- **docs_rs_mcp_example.rs** — connects to docs-rs MCP server, searches for dioxus crate [[docs-rs-mcp-example]]
- Can be extended for other MCP servers (filesystem, database, web search, etc.)

## Related Concepts

- [[mcp-client-integration]]: The underlying MCP bridge mechanism this example exercises
- [[agent-builder-pattern]]: The builder API used to configure the agent
- [[react-agent-docs]]: Documentation context for ReActAgent usage
