---
type: source
source_type: code
date: 2026-05-22
ingested: 2026-05-22
tags: [agent-input, agent-channel, unification, refactoring]
---

# AgentInput Channel Unification

**Authors/Creators:** BestNathan
**Date:** 2026-05-22
**Link:** branch feature/agentinput-unification (plan: docs/superpowers/plans/2026-05-22-agentinput-unification.md, spec: docs/superpowers/specs/2026-05-22-agentinput-unification-design.md)

## TL;DR

Unified the agent-channel crate to use `AgentInput` (from vol-llm-agent) as the canonical input type across protocol messages, internal request types, and the dispatcher execution path. Dropped redundant `run_id` and `metadata` fields on `AgentPayload::Submit` and `AgentRequest` since they are now carried inside `AgentInput`. Fixed the dispatcher's stale `run_with_id` call by switching to `run_input(AgentInput)`.

## Key Takeaways

- `AgentPayload::Submit` changed from `{ input: String, target, metadata, run_id }` to `{ input: AgentInput, target }`
- `AgentRequest` changed from `{ run_id, target_id, sender_id, input: String, metadata }` to `{ target_id, sender_id, input: AgentInput }`
- `AgentDispatcher` now calls `agent.run_input(request.input)` instead of the removed `agent.run_with_id()`
- `AgentHandler` passes `AgentInput` through directly, extracting `run_id` from `input.run_id` with UUID fallback
- Backwards compatibility preserved: `AgentInput` deserializes from both plain JSON string and structured object
- All 128 tests pass (55 channel + 23 agent-manager integration)
- Downstream crate `vol-agent-manager` updated for compatibility

## Detailed Summary

### Protocol layer (`agent_server_protocol.rs`)

Added `use vol_llm_agent::AgentInput;` import. Simplified `AgentPayload::Submit` to two fields:
- `input: AgentInput` — the multimodal input envelope (carries `run_id`, `parts`, `metadata`)
- `target: Option<String>` — channel routing target

Drop `metadata` and `run_id` from the variant and its decode path `Payload::from_operation`.

### Request type (`request.rs`)

`AgentRequest` simplified: `run_id` and `metadata` fields removed. `input` changed from `String` to `AgentInput`. Builder `new()` takes `AgentInput` instead of `impl Into<String>`. `with_run_id()` removed — callers chain `.with_run_id()` on `AgentInput` itself. Tests updated to use `AgentInput::text()` and `AgentInput::text().with_run_id()`.

### Dispatcher (`dispatcher.rs`)

Replaced `agent.run_with_id(&pending.request.input, pending.request.run_id)` with `agent.run_input(pending.request.input.clone())`. Updated `cancel()` to match `run_id` via `p.request.input.run_id.as_deref()`. Extracted `run_id` from `input.run_id` with UUID fallback for tracing and `RunResult` construction. Tests rewritten to construct `AgentRequest::new("agent_id", AgentInput::text("hello").with_run_id("run-1"))`.

### Agent handler (`domain/agent.rs`)

Submit match arm destructures only `input` and `target`. Builds `AgentRequest::new(&target_id, input)` directly. Run ID extracted from `input.run_id` with UUID fallback for ack/result messages.

### Downstream fixes

- `router.rs` test: `AgentRequest::new("nonexistent", AgentInput::text("hello"))`
- E2E test files, examples, and `vol-agent-manager/src/ws/handler.rs`: all Submit payloads use `AgentInput::text()`

## Entities Mentioned

- [[vol-llm-agent-channel-crate]]: all changes in this crate
- [[vol-llm-agent-crate]]: provides `AgentInput`, `InputPart`, `run_input`

## Concepts Covered

- [[agentinput-multimodal-run]]: `AgentInput` is now the universal input format
- [[agent-dispatcher]]: switched from `run_with_id` to `run_input`
- [[agent-server-protocol]]: Submit payload simplified

## Notes

- The pre-existing `test_code_agent_market_data_query` failure in vol-llm-agent is unrelated (tool-routing simulation issue)
- `cancel()` without explicit run_id is a no-op — UUID fallback happens at dequeue time, not queue time
