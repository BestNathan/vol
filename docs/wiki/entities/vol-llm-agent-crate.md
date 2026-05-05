---
type: entity
category: product
tags: [crate, agent, react, rust]
created: 2026-05-04
updated: 2026-05-04
source_count: 1
---

# vol-llm-agent Crate

**Category:** Rust crate — ReAct Agent orchestration
**Related:** [[react-pattern]], [[agent-plugin-system]], [[agent-builder-pattern]], [[run-context]], [[skill-system]], [[session-as-ssot]], [[vol-llm-core-crate]], [[vol-llm-tool-crate]], [[vol-llm-agent-channel-crate]]

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
