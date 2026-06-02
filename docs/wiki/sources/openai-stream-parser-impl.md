---
type: source
source_type: code
date: 2026-05-15
ingested: 2026-05-15
tags: [crate, provider, openai, streaming, sse, rust]
---

# OpenaiStreamParser Implementation

**Authors/Creators:** vol-llm-provider team
**Date:** 2026-05-15
**Link:** `crates/vol-llm-provider/src/openai_streaming.rs`

## TL;DR

Implemented `OpenaiStreamParser` — a `StreamProtocol` trait implementation that parses OpenAI-compatible Server-Sent Events (SSE) streaming responses. Supports the `data: {...}` line format with `[DONE]` sentinel.

## Key Takeaways

- `OpenaiStreamParser` implements the `StreamProtocol` trait defined in `vol-llm-core/src/streaming.rs`
- Parses OpenAI's SSE format: lines prefixed with `data:` containing JSON payloads
- `[DONE]` sentinel marks response completion
- Supports content deltas, tool call deltas (start + argument chunks), usage metadata, model info, and finish reasons
- Empty content strings are skipped to allow tool call deltas on the same chunk
- 6 unit tests covering all event types and edge cases
- Registered as `pub mod openai_streaming` and `pub use OpenaiStreamParser` in `vol-llm-provider/src/lib.rs`

## Detailed Summary

The parser handles these OpenAI SSE patterns:

1. **`[DONE]` sentinel** — Returns `ResponseComplete` with `FinishReason::Stop`
2. **Content deltas** — Extracts `choices[0].delta.content` as `ContentDelta`
3. **Tool call deltas** — Extracts `choices[0].delta.tool_calls[]` with `index`, `id`, `function.name`, and `function.arguments` — emits `ToolCallStart` on first chunk, `ToolCallDelta` for argument fragments
4. **Usage metadata** — Extracts `usage.prompt_tokens` and `usage.completion_tokens` as `Usage` event
5. **Model info** — Extracts top-level `model` field as `ResponseStart`
6. **Finish reasons** — Maps `"stop"`, `"length"`, `"tool_calls"`, `"content_filter"`, `"function_call"` strings to `FinishReason` enum variants

Empty lines, comment lines (`: ping`), and malformed JSON all return `None` (skipped gracefully).

## Entities Mentioned

- [[vol-llm-provider-crate]]: contains the OpenaiStreamParser implementation
- [[vol-llm-core-crate]]: defines StreamProtocol trait, ParsedEvent enum, FinishReason

## Concepts Covered

- [[agent-event-stream]]: ParsedEvent variants used by the parser
- [[streaming-session]]: StreamProtocol trait and StreamingSession that uses it
- [[http-transport]]: SSE streaming transport that feeds lines to the parser

## Notes

- This is Task 2 of 6 in building a full OpenAI provider
- Task 1 extracted the `StreamProtocol` trait from `StreamingSession` into `vol-llm-core`
- The parser is standalone — no HTTP logic, just line-level parsing
