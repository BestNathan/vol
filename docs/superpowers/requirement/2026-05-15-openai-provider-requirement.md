# Requirements: OpenAI-Compatible Provider

## Background

The `vol-llm-provider` crate currently only implements the Anthropic provider.
A generic OpenAI-compatible provider is needed so that any service following the
OpenAI chat completions API format can be used — including the official OpenAI
API, Azure OpenAI, vLLM, Ollama, LM Studio, and any other OpenAI-compatible
endpoint.

## Goals

1. Implement an `OpenaiProvider` struct that conforms to the `LLMClient` trait,
   matching the same interface as `AnthropicProvider`.
2. Support non-streaming (`converse()`) and SSE streaming (`converse_stream()`)
   request modes.
3. Map all OpenAI-specific parameters as first-class `SupportedParam` entries.
   The following `ModelConfig` fields map to OpenAI API body fields:

   | ModelConfig field        | OpenAI body field       | Value type |
   |--------------------------|-------------------------|------------|
   | `max_tokens`             | `max_tokens`            | u32        |
   | `temperature`            | `temperature`           | f64        |
   | `top_p`                  | `top_p`                 | f64        |
   | `top_k`                  | `top_k`                 | u32        |
   | `frequency_penalty`      | `frequency_penalty`     | f64        |
   | `presence_penalty`       | `presence_penalty`      | f64        |
   | `stop`                   | `stop`                  | [string]   |
   | `seed`                   | `seed`                  | u64        |
   | `logprobs`               | `logprobs`              | u32        |
   | `tools`                  | `tools` (array)         | [object]   |
   | `stream`                 | `stream` (bool)         | bool       |

   Parameters not set in `ModelConfig` are omitted from the request body (use
   `skip_serializing_if = "Option::is_none"` semantics). Parameters set in
   TOML `[body]` serve as defaults — if the same key is set in
   `ConversationRequest.model_config`, the request value wins.
4. Be loadable via the existing TOML config system (`.agents/providers/*.toml`)
   with `provider = "openai"`.
5. Support the same body-defaults and custom-headers mechanism as Anthropic —
   TOML `[body]` values serve as defaults, overridden by `ConversationRequest`
   model_config at runtime.

## Non-Goals

- No special handling for specific providers (Azure, vLLM, etc.) — the provider
  is generic and relies on `base_url` configuration.
- No assistant streaming events parsing beyond standard OpenAI SSE format.
- No image/vision support (text and tool calls only).
- No backward compatibility layer — this is a new provider, not a refactor.

## Scope

**Included:**
- New `crates/vol-llm-provider/src/openai.rs` module
- `OpenaiProvider` struct implementing `LLMClient`
- Message conversion (system/user/assistant/tool roles)
- Tool conversion (function calling)
- Full parameter mapping from `ModelConfig` to OpenAI API fields
- SSE streaming support with proper event parsing
- Body defaults merge logic (same pattern as AnthropicProvider)
- Custom headers attachment
- Factory integration (`factory.rs` dispatch)
- `LLMProvider::Openai` variant in `vol-llm-core` (if not already present)
- Unit tests for message conversion, body construction, parameter mapping
- Integration test via example file

**Excluded:**
- Audio/vision/multi-modal endpoints
- Embeddings or completions (non-chat) endpoints
- Provider-specific workarounds or quirks handling

## Constraints

- Must follow the same architectural patterns as `AnthropicProvider`.
- Must use the existing `LLMConfig` struct (no new config types needed).
- Must work with the existing `ProviderLoader` and `LLMProviderRegistry`.
- Proxy support via `HTTPS_PROXY` env var, same as Anthropic.

## Edge Cases

- **Empty response content**: Handle gracefully, return empty string.
- **Partial SSE events**: Buffer incomplete events across chunks. OpenAI SSE
  uses `data: {...}` lines with a `[DONE]` sentinel — not named events like
  Anthropic's `event: message_start`. Parse `data:` prefix, accumulate JSON
  objects, stop on `[DONE]`.
- **Unknown finish reasons**: Map to `FinishReason::Other`. OpenAI finish
  reasons include `"stop"`, `"length"`, `"tool_calls"`, `"content_filter"`,
  `"function_call"` — map appropriately.
- **Tool call with empty input**: Default to `{}`.
- **System prompt**: OpenAI supports system role directly in the messages
  array (unlike Anthropic which sends it separately). Include as
  `{"role": "system", "content": ...}` as the first message.
- **Multiple tool calls**: Support array of tool calls in assistant messages.
  OpenAI uses `tool_calls: [{id, type: "function", function: {name, arguments}}]`.
- **Tool result messages**: OpenAI uses `{"role": "tool", "tool_call_id": "...", "content": "..."}`.
- **Network errors**: Propagate as `LLMError::Network`.
- **API errors (4xx/5xx)**: Parse error JSON (`error.message` or `error.code`),
  fallback to raw text.

## API Details

- **Endpoint**: `{base_url}/v1/chat/completions` (POST)
- **Authentication**: `Authorization: Bearer <api_key>` header
- **Content-Type**: `application/json`
- **SSL**: Use `danger_accept_invalid_certs(true)` (same as AnthropicProvider)
- **Proxy**: Read `HTTPS_PROXY` / `https_proxy` env var, same as AnthropicProvider
- **Response envelope** (non-streaming):
  ```
  choices[0].message.content        → content string
  choices[0].message.tool_calls     → array of tool calls
  choices[0].finish_reason          → finish reason
  usage.prompt_tokens               → input tokens
  usage.completion_tokens           → output tokens
  model                             → model name
  ```
- **Response envelope** (streaming):
  - Requires `stream_options: {"include_usage": true}` in request body
  - Each SSE event is `data: {choices: [{delta: {content: "...", tool_calls: [...]}}], usage: {...}}`
  - Final event has `[DONE]` sentinel
- **Body defaults skip list**: These keys from TOML `[body]` must NOT be
  overwritten by the defaults merge loop (they are always set from the request):
  `"model"`, `"messages"`, `"tools"`, `"tool_choice"`, `"stream"`,
  `"stream_options"`, `"max_tokens"`. Override checks use `request.model_config.<field>.is_some()`.

## Success Criteria

1. `cargo test -p vol-llm-provider` passes with all new tests (≥ 5 unit tests
   covering message conversion, tool conversion, body construction, parameter
   mapping, and body-defaults merge).
2. `cargo build -p vol-llm-provider` compiles cleanly with zero new warnings.
3. The example `crates/vol-llm-provider/examples/provider_toml_chat.rs` works
   with a `provider = "openai"` TOML config file.
4. Streaming response: when `converse_stream()` is called, the receiver yields
   `LLMEvent` chunks that concatenate to the full assistant message.
5. Non-streaming response: `converse()` returns a `ConversationResponse` with
   correct content, usage, finish_reason, and model fields.
6. Body defaults from TOML `[body]` are correctly merged with request-level
   overrides: if request sets `temperature = 0.5` and TOML sets
  `temperature = 0.7`, the request value (0.5) is used.
7. Custom headers from TOML `[headers]` are attached to HTTP requests after
  default headers, allowing override of any non-essential header.

## Open Questions

None.
