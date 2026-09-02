---
type: entity
category: product
tags: [crate, agent, react, rust, session, binding]
created: 2026-05-04
updated: 2026-09-02
source_count: 6
---

# vol-llm-agent Crate

**Category:** Rust crate — ReAct Agent orchestration
**Related:** [[react-pattern]], [[agent-plugin-system]], [[agent-builder-pattern]], [[run-context]], [[skill-system]], [[session-as-ssot]], [[vol-llm-core-crate]], [[vol-llm-tool-crate]], [[vol-llm-agent-protocol-crate]], [[vol-llm-mcp-crate]], [[mcp-client-integration]], [[mcp-manager-lifecycle]], [[agentinput-multimodal-run]], [[vol-llm-agent-tool-crate]], [[session-task-binding]], [[vol-llm-task-crate]]

## Overview

The core crate implementing the ReAct Agent pattern with a plugin system for cross-cutting concerns. Provides agent lifecycle management, event streaming, and tool orchestration.

## Key Facts
- Implements `ReActAgent` with builder pattern [[react-agent-docs]]
- Plugin system with `AgentPlugin` trait and priority-based execution [[react-agent-docs]]
- Built-in plugins: HITL, Observability, Caching, Retry, RateLimiter [[react-agent-docs]]
- Re-exports key types: `ReActAgent`, `AgentConfig`, `AgentStreamEvent`, `AgentError` [[react-agent-docs]]
- Source modules: `react/`, `plugins/`, `observability/`, `rag/`, `embedding/` [[react-agent-docs]]
- `AgentInput.task_ids: Vec<vol_llm_task::TaskId>` carries bound task ids from the wire; every downstream layer passes `AgentInput` by value so `AgentPayload::Submit`, `AgentRequest`, the dispatcher, and the control-plane re-wrap need zero changes [[session-task-binding]]
- `ReActAgent::run_input` calls `Session::bind_task_ids` before any spawned task, logs `warn!` (with `run_id`, `session_id`, `task_ids`, `error`) on failure, and **does not abort the run** — binding is metadata; losing it must not kill a user's turn [[session-task-binding]]
- `RunContext.task_ids` is an attachment point for future context-injection and tool-scoping; nothing reads it yet [[session-task-binding]]
- Depends on `vol-llm-task` for `TaskId`. This closes a dev-dependency cycle (`vol-llm-task` dev-depends on `vol-llm-agent`) — Cargo permits this because it builds `vol-llm-task` lib, then `vol-llm-agent`, then `vol-llm-task` tests.

## Timeline
- **2026-04**: Initial ReAct Agent implementation with plugin system
- **2026-04**: Observability plugin added with JSONL logging
- **2026-04**: All 10 tests passing (mock, simulation, integration)
- **2026-05-11**: MCP client integration — `AgentConfig` gains `mcp_session` field, `AgentConfigBuilder` gains `with_mcp_from_config()` for auto-discovering MCP tools [[react-agent-mcp-integration]]
- **2026-05-21**: Multimodal run input — `AgentInput`/`InputPart` added, `run_input(AgentInput)` accepts text plus image URL/data URL parts, `run(&str)` remains a wrapper, and metadata/run_id flow into run context [[agentinput-multimodal-run-implementation]]
- **2026-08-17**: `AgentInput::display_text()` / `text_content()` render `InputPart::ImageUrl` as `[image]` markers; the `agent_start` stream event input now shows markers instead of raw base64 [[multimodal-image-input]]
- **2026-08-20**: `agent_tool` module moved out to the new [[vol-llm-agent-tool-crate]] (no re-export); `AgentLoader` gains `get_by_id()` for exact `"{scope}:{name}"` lookup [[agenttool-builtin-impl]]
- **2026-09-02**: `AgentInput.task_ids` field added; `run_input` binds to session before agent loop with warn-only-on-failure; `RunContext.task_ids` field added as future attachment point. Depends on `vol-llm-task` (dev-dep cycle closed) [[session-task-binding]], [[taskid-unification-session-task-binding]]
