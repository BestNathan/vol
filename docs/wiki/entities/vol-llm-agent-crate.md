---
type: entity
category: product
tags: [crate, agent, react, rust]
created: 2026-05-04
updated: 2026-05-19
source_count: 2
---

# vol-llm-agent Crate

**Category:** Rust crate — ReAct Agent orchestration
**Related:** [[react-pattern]], [[agent-plugin-system]], [[agent-builder-pattern]], [[run-context]], [[skill-system]], [[session-as-ssot]], [[vol-llm-core-crate]], [[vol-llm-tool-crate]], [[vol-llm-agent-channel-crate]], [[vol-llm-mcp-crate]], [[mcp-client-integration]], [[mcp-manager-lifecycle]]

## Overview

The core crate implementing the ReAct Agent pattern with a plugin system for cross-cutting concerns. Provides agent lifecycle management, event streaming, and tool orchestration.

## Key Facts
- Implements `ReActAgent` with builder pattern [[react-agent-docs]]
- Plugin system with `AgentPlugin` trait and priority-based execution [[react-agent-docs]]
- Built-in plugins: HITL, Observability, Caching, Retry, RateLimiter [[react-agent-docs]]
- Plugin event shutdown now uses optional shared channel sender handles, `emit_traced()`, interceptor `plugin_rx` close, and listener task draining instead of normal timeout-based abort [[react-plugin-event-shutdown]]
- Source modules: `react/`, `plugins/`, `observability/`, `rag/`, `embedding/` [[react-agent-docs]]

## Timeline
- **2026-04**: Initial ReAct Agent implementation with plugin system
- **2026-04**: Observability plugin added with JSONL logging
- **2026-04**: All 10 tests passing (mock, simulation, integration)
- **2026-05-19**: Plugin event shutdown refactor — `RunContext` channel senders are optional shared handles, `PluginRequest::Emit` preserves trace ids through `emit_traced()`, interceptor exits on `plugin_rx` close, and listener tasks drain in-flight plugin work [[react-plugin-event-shutdown]]
