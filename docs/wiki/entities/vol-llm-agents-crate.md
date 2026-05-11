---
type: entity
category: product
tags: [crate, agent, rust, high-level]
created: 2026-05-04
updated: 2026-05-11
source_count: 2
---

# vol-llm-agents Crate

**Category:** Rust crate — High-level agent implementations
**Related:** [[vol-llm-agent-crate]], [[react-pattern]]

## Overview

Contains concrete agent implementations built on top of `vol-llm-agent`: advice agent, coding agent, PPT agent, QA agent, and wiki agent.

## Key Facts
- Provides domain-specific agent implementations [[react-agent-docs]]
- Each agent type lives in its own subdirectory: `advice/`, `coding/`, `ppt/`, `qa/`, `wiki/` [[react-agent-docs]]
- Includes integration and E2E tests for each agent type
- Tests include: coding agent with Deribit WebSocket, agent-Loki integration, PPT agent integration, skill session integration
- Contains runnable examples in `examples/` directory demonstrating MCP integration [[docs-rs-mcp-example]], [[mcp-example-pattern]]

## Examples

| File | Description |
|------|-------------|
| `docs_rs_mcp_example.rs` | ReActAgent connecting to docs-rs MCP server via `with_mcp_from_config()` |
| `qa_agent_example.rs` | QaAgent with MockEmbedder and InMemoryStore |
| `agent_loki_example.rs` | Agent integration with Loki observability |
| `coding_agent_basic.rs` | Basic CodingAgent usage |
| `coding_agent_wordcount.rs` | CodingAgent with word count task |

## Timeline
- **2026-04**: Agent implementations added
- **2026-04**: Integration tests written for all agent types
- **2026-05-11**: Docs-RS MCP integration example added [[docs-rs-mcp-example]]
