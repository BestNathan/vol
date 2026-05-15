---
type: concept
category: pattern
tags: [streaming, sse, protocol, openai, anthropic, rust]
created: 2026-05-15
updated: 2026-05-15
source_count: 2
---

# Streaming Session and StreamProtocol

**Category:** Architecture pattern — protocol-abstracted SSE streaming
**Related:** [[agent-event-stream]], [[http-transport]], [[vol-llm-core-crate]], [[vol-llm-provider-crate]]

## Definition

The `StreamProtocol` trait and `StreamingSession` struct in `vol-llm-core/src/streaming.rs` provide a protocol-abstracted layer for parsing Server-Sent Events (SSE) from LLM providers and accumulating streaming chunks into structured `StreamEvent`s.

## Key Points

- `StreamProtocol` trait: protocol-specific parser with single method `parse_line(&self, line: &str) -> Option<Result<ParsedEvent, LLMError>>`
- `ParsedEvent` enum: internal event types parsers produce (ContentDelta, ToolCallStart, ToolCallDelta, Usage, ResponseStart, ResponseComplete, etc.)
- `StreamingSession` struct: accumulates state across chunks (content buffer, thinking buffer, tool call builder, event ID counter)
- `StreamingSession::apply()`: converts `ParsedEvent` into `StreamEvent`s for external consumption
- `StreamingSession::process_sse()`: feeds a line through a protocol parser and applies the result
- `ToolCallBuilder`: accumulates tool call chunks (id, name, arguments) into a complete `ToolCall`

## How It Works

1. Raw SSE lines arrive from the HTTP transport (e.g., `data: {"id":"...","choices":[...]}`)
2. Each line is passed to `StreamingSession::process_sse(parser, line)`
3. The `StreamProtocol` implementation parses the line and returns a `ParsedEvent`
4. `StreamingSession::apply()` updates internal state and emits `StreamEvent`s
5. On completion, `StreamingSession::finalize()` flushes any remaining accumulated content

Two protocol implementations exist:
- `AnthropicProtocol`: parses Anthropic's `type`-field-based SSE format (`message_start`, `content_block_delta`, etc.)
- `OpenaiStreamParser`: parses OpenAI's `choices[0].delta` format with `[DONE]` sentinel

## Usage in Providers

Both `AnthropicProvider` and `OpenaiProvider` use `StreamingSession` for their `converse_stream()` implementations:
- `AnthropicProvider` uses `process_anthropic_sse()` (backward compat wrapper for `process_sse(&AnthropicProtocol, line)`)
- `OpenaiProvider` uses `process_sse(&OpenaiStreamParser, line)` directly

## Examples

```rust
// OpenAI-style streaming
let mut session = StreamingSession::new();
let parser = OpenaiStreamParser;

let events = session.process_sse(&parser, "data: [DONE]");
// -> [ResponseComplete { finish_reason: Stop }]

let events = session.process_sse(&parser,
    r#"data: {"choices":[{"delta":{"content":"Hello"},"finish_reason":null}]}"#);
// -> [ContentDelta("Hello")]
```

## Related Concepts

- [[agent-event-stream]]: StreamEvent types emitted by StreamingSession
- [[http-transport]]: HTTP/SSE transport that feeds lines into the session
- [[vol-llm-core-crate]]: crate containing the trait and session types
- [[vol-llm-provider-crate]]: crate containing protocol implementations
