---
type: entity
category: product
tags: [crate, tools, rust, registry]
created: 2026-05-04
updated: 2026-05-19
source_count: 2
---

# vol-llm-tool Crate

**Category:** Rust crate — Tool definition and execution framework
**Related:** [[tool-registry]], [[vol-llm-agent-crate]], [[tdengine]], [[vol-llm-mcp-crate]], [[mcp-client-integration]], [[mcp-manager-lifecycle]]

## Overview

Provides the `Tool` trait, `ToolRegistry`, and execution framework for agent tools. All tools must implement `Tool` with name, description, parameters (JSON schema), and execute method.

## Key Facts
- `Tool` trait: name, description, parameters schema, async execute [[react-agent-docs]]
- `ToolRegistry`: HashMap-based registration and dispatch [[react-agent-docs]]
- `ToolContext`: provides alert info, message history, and custom metadata [[react-agent-docs]]
- `McpTool` proxies MCP tool execution through `McpManager` in the current manager-based registry flow [[react-plugin-event-shutdown]]

## Timeline
- **2026-05-19**: `McpTool` constructor/storage aligned with `McpManager` to match `ToolRegistry::register_from_mcp()` after the manager lifecycle refactor [[react-plugin-event-shutdown]]
