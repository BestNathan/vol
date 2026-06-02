---
type: source
source_type: code
date: 2026-05-15
ingested: 2026-05-15
tags: [crate, provider, openai, rust, llm]
---

# OpenaiProvider Implementation

**Authors/Creators:** vol-llm team
**Date:** 2026-05-15
**Link:** `crates/vol-llm-provider/src/openai.rs`

## TL;DR

Full `OpenaiProvider` implementing `LLMClient` for the OpenAI Chat Completions API, completing Task 3 of 6 in the OpenAI provider build-out. Mirrors the `AnthropicProvider` architecture with provider-specific message/tool conversion, SSE streaming, and factory dispatch.

## Key Takeaways

- `OpenaiProvider` struct with same fields as `AnthropicProvider` (client, api_key, model, base_url, body_defaults, headers)
- `convert_messages()`: system as first message with `role: "system"`, user/assistant/tool roles mapped directly, assistant tool calls use OpenAI `[{id, type: "function", function: {name, arguments}}]` format
- `convert_tools()`: wraps each tool in `{"type": "function", "function": {name, description, parameters}}`
- Auth: `Authorization: Bearer <api_key>` header (not `x-api-key`)
- Endpoint: `{base_url}/v1/chat/completions`
- Response parsing: `choices[0].message.content`, `choices[0].message.tool_calls`, `choices[0].finish_reason`, `usage.prompt_tokens`/`usage.completion_tokens`
- Streaming: `stream: true` + `stream_options: {"include_usage": true}` in request; uses `OpenaiStreamParser` and `StreamingSession::process_sse()` (not `process_anthropic_sse()`)
- 6 unit tests pass: user message, system message, tool message, assistant with tools, tool conversion, multiple messages
- Factory dispatch: `create_provider()` now matches `LLMProvider::OpenAI`

## Detailed Summary

### Message Conversion

Messages are converted 1:1 to OpenAI format:
- **System**: `{"role": "system", "content": "..."}` -- sent as first element in messages array
- **User**: `{"role": "user", "content": "..."}`
- **Assistant**: `{"role": "assistant", "content": "...", "tool_calls": [...]}` with tool calls in OpenAI format
- **Tool**: `{"role": "tool", "tool_call_id": "...", "content": "..."}`

### Tool Conversion

Tools are wrapped in OpenAI's nested function format:
```json
{
  "type": "function",
  "function": {
    "name": "...",
    "description": "...",
    "parameters": { ... }
  }
}
```

### Converse Implementation

- Builds request body with `model`, `messages`, `max_tokens` (default 4096 for OpenAI vs 8192 for Anthropic)
- Optional params: `temperature`, `top_p`, `tools`
- Body defaults merged with override detection
- Response parsed from JSON: content, tool_calls, usage, finish_reason mapped to `FinishReason` enum variants

### Streaming Implementation

- `stream: true` and `stream_options: {"include_usage": true}` in body
- SSE chunks processed via `StreamingSession::process_sse(&OpenaiStreamParser, line)`
- Spawns tokio task to feed bytes into session, sends `StreamEvent`s through mpsc channel

## Entities Mentioned

- [[vol-llm-provider-crate]]: crate containing the implementation
- [[vol-llm-core-crate]]: contains `LLMClient`, `LLMProvider::OpenAI`, `Message`, `ToolDefinition`, etc.
- [[streaming-session]]: `StreamingSession::process_sse()` used for streaming
- [[openai-stream-parser-impl]]: `OpenaiStreamParser` used in streaming

## Concepts Covered

- [[agent-event-stream]]: StreamEvent types used in streaming
- [[http-transport]]: underlying HTTP transport for SSE streaming

## Notes

- No `thinking` block handling (OpenAI doesn't support extended thinking in this format)
- No multi-part content (images) handled yet -- only text messages
- Proxy configuration mirrors Anthropic (HTTPS_PROXY / https_proxy env vars)
