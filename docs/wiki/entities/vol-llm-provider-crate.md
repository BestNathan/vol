---
type: entity
category: product
tags: [crate, provider, anthropic, openai, rust, streaming]
created: 2026-05-04
updated: 2026-05-15
source_count: 2
---

# vol-llm-provider Crate

**Category:** Rust crate — LLM provider implementations
**Related:** [[vol-llm-core-crate]], [[vol-llm-agent-crate]], [[dashscope]]

## Overview

Implements the `LLMClient` trait for Anthropic and OpenAI providers, handling protocol conversion between the unified message format and each provider's API. Also provides protocol-abstracted SSE streaming parsers via the `StreamProtocol` trait.

## Key Facts
- `AnthropicProvider`: converts to/from Anthropic Messages API format
- `OpenAIProvider`: converts to/from OpenAI Chat Completions format
- `OpenaiStreamParser`: `StreamProtocol` implementation for OpenAI SSE format (Task 2 of OpenAI provider build-out)
- Factory pattern: `create_provider(config)` returns boxed trait
- Configuration via TOML with environment variable API key support
- `ProviderLoader`: dual-layer TOML loading (project + user configs with overrides)
- `ProviderFileConfig`: body default + header passthrough configuration

## Module Structure
| Module | Description |
|--------|-------------|
| `anthropic` | Anthropic provider implementation |
| `openai_streaming` | OpenaiStreamParser — SSE line parser for OpenAI format |
| `config` | LLMConfig, ProviderFileConfig types |
| `factory` | Provider factory function |
| `loader` | ProviderLoader — file-based config loading |
| `registry` | LLMProviderRegistry — named provider lookup |
| `secret` | Secret resolution (env var vs literal) |

## Timeline
- **2026-04**: Provider implementations added
- **2026-05-15**: OpenaiStreamParser added — implements StreamProtocol for OpenAI SSE format; supports [DONE] sentinel, content/tool deltas, usage, finish reasons; 6 tests pass
