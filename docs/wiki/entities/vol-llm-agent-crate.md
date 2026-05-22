---
type: entity
category: product
tags: [crate, agent, react, rust]
created: 2026-05-04
updated: 2026-05-21
source_count: 2
---

# vol-llm-agent Crate

**Category:** Rust crate — ReAct Agent orchestration
**Related:** [[react-pattern]], [[agent-plugin-system]], [[agent-builder-pattern]], [[run-context]], [[skill-system]], [[session-as-ssot]], [[vol-llm-core-crate]], [[vol-llm-tool-crate]], [[vol-llm-agent-channel-crate]], [[vol-llm-mcp-crate]], [[mcp-client-integration]], [[mcp-manager-lifecycle]], [[agentinput-multimodal-run]]

## Overview

The core crate implementing the ReAct Agent pattern with a plugin system for cross-cutting concerns. Provides agent lifecycle management, event streaming, and tool orchestration.

## Key Facts
- Implements `ReActAgent` with builder pattern [[react-agent-docs]]
- Plugin system with `AgentPlugin` trait and priority-based execution [[react-agent-docs]]
- Built-in plugins: HITL, Observability, Caching, Retry, RateLimiter [[react-agent-docs]]
- Re-exports key types: `ReActAgent`, `AgentConfig`, `AgentStreamEvent`, `AgentError` [[react-agent-docs]]
- Source modules: `react/`, `plugins/`, `observability/`, `rag/`, `embedding/` [[react-agent-docs]]

## Timeline
- **2026-04**: Initial ReAct Agent implementation with plugin system
- **2026-04**: Observability plugin added with JSONL logging
- **2026-04**: All 10 tests passing (mock, simulation, integration)
- **2026-05-11**: MCP client integration — `AgentConfig` gains `mcp_session` field, `AgentConfigBuilder` gains `with_mcp_from_config()` for auto-discovering MCP tools [[react-agent-mcp-integration]]
- **2026-05-21**: Multimodal run input — `AgentInput`/`InputPart` added, `run_input(AgentInput)` accepts text plus image URL/data URL parts, `run(&str)` remains a wrapper, and metadata/run_id flow into run context [[agentinput-multimodal-run-implementation]]
