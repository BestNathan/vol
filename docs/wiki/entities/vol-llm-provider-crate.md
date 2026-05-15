---
type: entity
category: product
tags: [crate, provider, anthropic, openai, rust, streaming]
created: 2026-05-04
updated: 2026-05-15
source_count: 3
---

# vol-llm-provider Crate

**Category:** Rust crate — LLM provider implementations
**Related:** [[vol-llm-core-crate]], [[vol-llm-agent-crate]], [[dashscope]]

## Overview

Implements the `LLMClient` trait for Anthropic and OpenAI providers, handling protocol conversion between the unified message format and each provider's API. Also provides protocol-abstracted SSE streaming parsers via the `StreamProtocol` trait.

## Key Facts
- `AnthropicProvider`: converts to/from Anthropic Messages API format
- `OpenAIProvider`: converts to/from OpenAI Chat Completions API format (Task 3 complete)
  - Auth: `Authorization: Bearer <api_key>` header
  - Endpoint: `{base_url}/v1/chat/completions`
  - System prompt sent as first message with `role: "system"`
  - Tool calls in OpenAI `[{id, type: "function", function: {name, arguments}}]` format
  - Streaming uses `OpenaiStreamParser` + `StreamingSession::process_sse()`
  - 6 unit tests pass for message/tool conversion
- `OpenaiStreamParser`: `StreamProtocol` implementation for OpenAI SSE format (Task 2 of OpenAI provider build-out)
- Factory pattern: `create_provider(config)` returns boxed trait
- Configuration via TOML with environment variable API key support
- `ProviderLoader`: dual-layer TOML loading (project + user configs with overrides)
- `ProviderFileConfig`: body default + header passthrough configuration

## Module Structure
| Module | Description |
|--------|-------------|
| `anthropic` | Anthropic provider implementation |
| `openai` | OpenAI provider implementation (new) |
| `openai_streaming` | OpenaiStreamParser — SSE line parser for OpenAI format |
| `config` | LLMConfig, ProviderFileConfig types |
| `factory` | Provider factory function |
| `loader` | ProviderLoader — file-based config loading |
| `registry` | LLMProviderRegistry — named provider lookup |
| `secret` | Secret resolution (env var vs literal) |

## Timeline
- **2026-04**: Provider implementations added
- **2026-05-15**: OpenaiStreamParser added — implements StreamProtocol for OpenAI SSE format; supports [DONE] sentinel, content/tool deltas, usage, finish reasons; 6 tests pass
- **2026-05-15**: OpenaiProvider added — full LLMClient impl for OpenAI Chat Completions API; message/tool conversion, SSE streaming, factory dispatch; 6 unit tests pass
