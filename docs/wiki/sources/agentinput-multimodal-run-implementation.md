---
type: source
source_type: code
date: 2026-05-21
ingested: 2026-05-21
tags: [agent-input, multimodal, react-agent, anthropic, agent-channel]
---

# AgentInput Multimodal Run Implementation

**Authors/Creators:** Claude Code session with user-approved design
**Date:** 2026-05-21
**Link:** `docs/superpowers/specs/2026-05-21-agentinput-multimodal-run-design.md`, `docs/superpowers/plans/2026-05-21-agentinput-multimodal-run.md`

## TL;DR

The ReAct agent run API now accepts structured multimodal input through `AgentInput` while preserving the existing `run(&str)` convenience API. The first supported modalities are text and image URL/data URL parts, with caller-provided `run_id` and metadata carried into run context; Anthropic conversion emits provider-native multipart blocks, and agent-channel transports accept both legacy string input and structured input.

## Key Takeaways

- `vol_llm_agent::AgentInput` wraps `run_id`, `metadata`, and ordered `InputPart` values for text and image URL/data URL inputs.
- `ReActAgent::run(&str)` remains a compatibility wrapper over `run_input(AgentInput::text(...))`.
- `run_input` rejects empty `parts` before calling the LLM, uses provided `run_id` when present, and stores metadata in `RunContext.data`.
- `vol-llm-provider` converts user multipart messages to Anthropic text/image content blocks, including base64 data URL parsing.
- `vol-llm-agent-channel` accepts old JSON string input and new structured input, with HTTP, WebSocket, dispatcher, and JSON-RPC paths updated.
- `vol-agent-manager` protocol tests and WebSocket handling were updated to read text content from `AgentInput` where manager control messages remain JSON text.

## Detailed Summary

The implementation introduces `crates/vol-llm-agent/src/react/input.rs` with `AgentInput`, `InputPart`, and conversion helpers into `vol_llm_core::MessageContent`. Plain text inputs remain `MessageContent::Text`, while mixed text/image inputs become `MessageContent::MultiPart` containing `ContentPart::Text` and `ContentPart::Image` values.

`ReActAgent` now exposes `run_input(AgentInput)` in addition to `run(&str)`. The structured API allows callers to pass a stable `run_id`, attach metadata to the run context, and send multimodal user messages through the existing session and LLM request path. Empty structured inputs produce `AgentError::InvalidInput` before any LLM call.

Anthropic provider conversion now preserves multipart user content. URL images are sent as Anthropic `source.type = "url"`; data URLs are split into media type and base64 data and sent as `source.type = "base64"`. Invalid data URLs return `LLMError::InvalidRequest`.

The channel protocol changed `Submit.input` and `AgentRequest.input` from raw text to `AgentInput`, with custom deserialization preserving old clients that still send `{ "input": "hello" }`. HTTP, WebSocket, dispatcher, and JSON-RPC integration tests cover both compatibility paths.

## Entities Mentioned

- [[vol-llm-agent-crate]]: owns `AgentInput`, `InputPart`, and `ReActAgent::run_input`.
- [[vol-llm-core-crate]]: provides provider-neutral multipart message content types.
- [[vol-llm-provider-crate]]: converts multipart user content to Anthropic request JSON.
- [[vol-llm-agent-channel-crate]]: carries `AgentInput` through transports and dispatchers.
- [[vol-llm-tool-crate]]: received a related compile fix for `McpTool` to use `McpManager`.

## Concepts Covered

- [[agentinput-multimodal-run]]: structured multimodal run input envelope for ReAct agents.
- [[react-pattern]]: run loop now starts from structured user content when available.
- [[jsonrpc-transport]]: channel compatibility preserves legacy string input while supporting structured input.
- [[mcp-manager-lifecycle]]: related compile blocker fixed by aligning `McpTool` with `McpManager`.

## Notes

The implementation intentionally does not read local image files or infer MIME types at the agent layer. Future modalities such as audio should extend `InputPart` and add provider conversion only when a target provider path is implemented.
