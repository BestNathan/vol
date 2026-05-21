---
type: source
source_type: code
date: 2026-05-21
ingested: 2026-05-21
tags: [agent-channel, protocol, run-id, dispatcher, react-agent]
---

# Run ID Unification

**Authors/Creators:** BestNathan + Claude Code
**Date:** 2026-05-21
**Link:** crates/vol-llm-agent-channel, crates/vol-llm-agent

## TL;DR

The Agent Server/channel identity model was unified so `run_id` is the business identifier for one ReAct inference run, while `message_id` remains only a protocol correlation identifier. `agent.submit` can now carry an optional caller-provided `run_id`; if absent, the channel request layer generates one and passes it through the dispatcher to `ReActAgent::run_with_id()`.

## Key Takeaways

- `message_id` correlates command/ack/result/error protocol messages and is not a run lifecycle id.
- `run_id` identifies one agent inference run across submit ack/result, cancellation, dispatcher state, logs, and `RunContext`.
- `AgentRequest` and `RunResult` now carry `run_id` instead of the previous internal request-id naming.
- `AgentDispatcher` calls `ReActAgent::run_with_id()` so lower-level agent execution receives the channel-selected run id.
- Legacy HTTP/WS shims still bridge old `req_id` wire fields into the new internal `run_id` model.
- Focused regressions cover protocol decoding, supplied/missing run ids, dispatcher cancellation, agent flow, JSON-RPC E2E dispatch, and ReActAgent caller-provided run ids.

## Detailed Summary

The unified protocol model updates `AgentPayload::Submit` to include optional `run_id`, changes `AgentPayload::Cancel` to target `run_id`, and returns `run_id` in submit and cancel results. Payload decoding now accepts supplied `run_id` for `agent.submit`, defaults missing values to `None`, and decodes `agent.cancel` from `{ "run_id": ... }`.

The channel request layer now models business identity directly: `AgentRequest::new()` generates a run id, `AgentRequest::with_run_id()` preserves a caller-provided run id, and `RunResult` carries the same id back to callers. `AgentDispatcher` cancels queued work by `run_id` and passes `pending.request.run_id` into `ReActAgent::run_with_id()`. `AgentRouter::cancel()` also uses run id across registered dispatchers.

The agent domain handler preserves a supplied `run_id` from `agent.submit`, otherwise allows `AgentRequest::new()` to allocate one. Submit acknowledgements and synthetic submit results reuse the command `message_id` for protocol correlation while carrying the chosen `run_id` in payloads. `agent.cancel` now routes by run id and returns `CancelResult { run_id, cancelled }`.

`ReActAgent::run()` remains the convenience entrypoint and generates a new id internally. The new `ReActAgent::run_with_id()` accepts a caller-provided id and constructs the `RunContext` with that exact value so plugins, logs, and final responses observe the same run identity selected by the channel layer.

## Entities Mentioned

- [[vol-llm-agent-channel-crate]]: owns the Agent Server protocol payloads, request model, dispatcher, router, and transport shims updated for run id identity.
- [[vol-llm-agent-crate]]: owns `ReActAgent::run_with_id()` and `RunContext` creation with caller-provided ids.

## Concepts Covered

- [[run-context]]: now receives externally supplied run ids through `ReActAgent::run_with_id()`.
- [[agent-dispatcher]]: queues and cancels work by run id, then passes that id into agent execution.
- [[agent-router]]: cancels by run id across registered dispatchers.
- [[json-rpc-websocket]]: JSON-RPC ids remain transport correlation ids; run identity lives in operation payloads.

## Notes

The legacy `Message` transport shim still exposes old `req_id` field names on its compatibility boundary. Those fields are bridged into `run_id` internally and should not be treated as part of the unified Agent Server protocol model.
