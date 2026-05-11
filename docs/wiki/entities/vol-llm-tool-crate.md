---
type: entity
category: product
tags: [crate, tools, rust, registry]
created: 2026-05-04
updated: 2026-05-04
source_count: 1
---

# vol-llm-tool Crate

**Category:** Rust crate — Tool definition and execution framework
**Related:** [[tool-registry]], [[vol-llm-agent-crate]], [[tdengine]], [[vol-llm-mcp-crate]], [[mcp-client-integration]]

## Overview

Provides the `Tool` trait, `ToolRegistry`, and execution framework for agent tools. All tools must implement `Tool` with name, description, parameters (JSON schema), and execute method.

## Key Facts
- `Tool` trait: name, description, parameters schema, async execute [[react-agent-docs]]
- `ToolRegistry`: HashMap-based registration and dispatch [[react-agent-docs]]
- `ToolContext`: provides alert info, message history, and custom metadata [[react-agent-docs]]
- `ToolResult`: structured result with content, error, and optional structured data

## Timeline
- **2026-04**: Tool framework implemented
