# OpenAI Provider Design Spec

## Overview

**Goal:** Add an `OpenaiProvider` that implements the OpenAI chat completions API format, loadable via the TOML config system (`.agents/providers/*.toml` with `provider = "openai"`).

**Approach:** Follow the same architectural pattern as `AnthropicProvider`, with protocol-specific SSE streaming extracted into a pluggable parser via a `StreamProtocol` trait on the shared `StreamingSession`.

---

## File Structure

### New files
- `crates/vol-llm-provider/src/openai.rs` — `OpenaiProvider` struct, `LLMClient` impl
- `crates/vol-llm-provider/src/openai_streaming.rs` — `OpenaiStreamParser` implementing `StreamProtocol`

### Modified files
- `crates/vol-llm-core/src/streaming.rs` — Extract `StreamProtocol` trait; refactor `StreamingSession` to be protocol-agnostic; add `AnthropicProtocol` as a separate impl
- `crates/vol-llm-provider/src/factory.rs` — Add `LLMProvider::OpenAI` dispatch
- `crates/vol-llm-provider/src/lib.rs` — Export `openai` module and `OpenaiProvider`
- `crates/vol-llm-core/src/client.rs` — Add `OpenAI` to `SupportedParam` enum (no change, all needed variants already exist)

---

## OpenaiProvider

### Struct

```rust
pub struct OpenaiProvider {
    client: Client,
    api_key: String,
    model: String,
    base_url: String,
    body_defaults: HashMap<String, serde_json::Value>,
    headers: HashMap<String, String>,
}
```

### HTTP Client (`build_client()`)

- Same pattern as `AnthropicProvider`: `reqwest::Client::builder().danger_accept_invalid_certs(true)`
- Proxy support via `HTTPS_PROXY` / `https_proxy` env var, with `NoProxy` for known endpoints if needed
- Single shared client instance per provider

### Auth & Headers

- Auth: `Authorization: Bearer <api_key>`
- Content-Type: `application/json`
- Custom headers from TOML `[headers]` applied **after** hardcoded headers, allowing override

### Endpoint

- `{base_url}/v1/chat/completions` (POST, JSON body)

### Supported Parameters

```rust
fn supported_params(&self) -> &[SupportedParam] {
    &[
        SupportedParam::MaxTokens,
        SupportedParam::Temperature,
        SupportedParam::TopP,
        SupportedParam::TopK,
        SupportedParam::FrequencyPenalty,
        SupportedParam::PresencePenalty,
        SupportedParam::Stop,
        SupportedParam::Seed,
        SupportedParam::LogProbs,
        SupportedParam::Tools,
    ]
}
```

### Message Conversion

| MessageRole | OpenAI format |
|---|---|
| System | `{role: "system", content: text}` — first message in array |
| User | `{role: "user", content: text}` |
| Assistant (text) | `{role: "assistant", content: text}` |
| Assistant (tool calls) | `{role: "assistant", content: text, tool_calls: [{id, type: "function", function: {name, arguments}}]}` |
| Tool | `{role: "tool", tool_call_id: "...", content: text}` |

### Tool Conversion

```rust
fn convert_tools(&self, tools: &[ToolDefinition]) -> serde_json::Value {
    tools.iter().map(|t| {
        json!({
            "type": "function",
            "function": {
                "name": t.name,
                "description": t.description,
                "parameters": t.parameters (or empty schema),
            },
        })
    }).collect::<Vec<_>>()
}
```

### Request Body Construction

**Non-streaming:**
```json
{
  "model": "...",
  "messages": [...],
  "max_tokens": 8192,
  "temperature": ...,
  "top_p": ...,
  "top_k": ...,
  "frequency_penalty": ...,
  "presence_penalty": ...,
  "stop": [...],
  "seed": ...,
  "logprobs": ...,
  "tools": [...]
}
```

**Streaming:** same as above + `"stream": true`, `"stream_options": {"include_usage": true}`

### Body Defaults Merge (same as Anthropic)

1. Start with hardcoded fields: `model`, `messages`
2. Add `max_tokens` (from request or default 8192)
3. Add `stream` / `stream_options` if streaming
4. Apply TOML `[body]` defaults for all other keys
5. Override with `request.model_config` values when set (skip overwrite for keys that are always set from request)

Keys that skip body defaults: `"model"`, `"messages"`, `"tools"`, `"tool_choice"`, `"stream"`, `"stream_options"`, `"max_tokens"`

### Response Parsing (non-streaming)

```
response.choices[0].message.content        → content string (collect text blocks)
response.choices[0].message.tool_calls     → Vec<ToolCall>
response.choices[0].finish_reason          → FinishReason mapping
response.usage.prompt_tokens               → prompt_tokens
response.usage.completion_tokens           → completion_tokens
response.model                             → model name
```

Finish reason mapping:
- `"stop"` → `FinishReason::Stop`
- `"length"` → `FinishReason::Length`
- `"tool_calls"` → `FinishReason::ToolCalls`
- `"content_filter"` → `FinishReason::ContentFilter`
- `"function_call"` → `FinishReason::ToolCalls` (treat same as tool_calls)
- other → `FinishReason::Other`

### Response Parsing (streaming)

Each SSE event: `data: {"choices": [{"delta": {"content": "...", "tool_calls": [...]}, "finish_reason": ...}], "usage": ...}`

- `delta.content` → text content delta
- `delta.tool_calls` → indexed array of partial tool calls. Each item:
  `{index: 0, id: "call_...", type: "function", function: {name: "...", arguments: "..."}}`.
  The `id` and `name` appear on the first chunk only; subsequent chunks have
  them as null with only partial `function.arguments` (accumulated as JSON).
- `usage` → token usage (appears in later events, may be null on early events)
- `finish_reason` transitions from `null` → final reason on last chunk
- `[DONE]` sentinel marks end of stream

---

## StreamingSession Refactor

### New Trait

```rust
pub trait StreamProtocol: Send {
    fn parse_line(&self, line: &str) -> Option<Result<ParsedEvent, LLMError>>;
}

pub enum ParsedEvent {
    ContentDelta(String),
    ContentComplete(String),
    ThinkingDelta(String),
    ThinkingComplete(String),
    ToolCallStart { index: usize, id: Option<String>, name: Option<String> },
    ToolCallDelta { index: usize, delta: String },
    ToolCallComplete(ToolCall),
    Usage(TokenUsage),
    ResponseStart { model: String },
    ResponseComplete { finish_reason: FinishReason },
}
```

### StreamingSession Changes

- Add `apply(&mut self, event: &ParsedEvent) -> Vec<Result<StreamEvent, LLMError>>`
- Add `process_sse(&mut self, protocol: &impl StreamProtocol, line: &str) -> Vec<Result<StreamEvent, LLMError>>`
- Keep `process_anthropic_sse()` as convenience wrapper (backward compat)
- Move Anthropic-specific parsing into `AnthropicProtocol` impl

### OpenaiStreamParser

Implements `StreamProtocol`:
- Parse `data:` prefix
- Check for `[DONE]` sentinel → `ResponseComplete`
- Parse JSON object → extract `choices[0].delta` and `choices[0].finish_reason` and `usage`
- Map to `ParsedEvent` variants

---

## Error Handling

- **Network errors** → `LLMError::Network(reqwest::Error)`
- **API errors** (HTTP 4xx/5xx) → parse `error.message` from JSON, fallback to raw text → `LLMError::Api { status, message }`
- **Parse errors in SSE** → `LLMError::Parse`, logged and skipped (stream continues)
- **Missing response fields** → use defaults: empty string for content, 0 for tokens, `FinishReason::Other`
- **Tool call with empty input** → default to `{}`

---

## Testing

### Unit tests (openai.rs)
- `test_convert_messages_user` — user message → OpenAI JSON
- `test_convert_messages_system` — system message → first in array
- `test_convert_messages_tool` — tool result → OpenAI tool message
- `test_convert_messages_assistant_with_tools` — assistant tool calls → tool_calls array
- `test_convert_tools_basic` — ToolDefinition → OpenAI function format
- `test_body_defaults_merge` — TOML body + request override

### Unit tests (openai_streaming.rs)
- `test_parse_delta_content` — `data: {...}` → ContentDelta
- `test_parse_delta_tool_calls` — tool call delta parsing
- `test_parse_done_sentinel` — `[DONE]` → ResponseComplete
- `test_parse_usage` — usage extraction from streaming event

### Integration
- `provider_toml_chat` example works with `provider = "openai"` TOML config
