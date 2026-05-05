---
type: entity
category: product
tags: [crate, provider, anthropic, openai, rust]
created: 2026-05-04
updated: 2026-05-04
source_count: 1
---

# vol-llm-provider Crate

**Category:** Rust crate — LLM provider implementations
**Related:** [[vol-llm-core-crate]], [[vol-llm-agent-crate]], [[dashscope]]

## Overview

Implements the `LLMClient` trait for Anthropic and OpenAI providers, handling protocol conversion between the unified message format and each provider's API.

## Key Facts
- `AnthropicProvider`: converts to/from Anthropic Messages API format
- `OpenAIProvider`: converts to/from OpenAI Chat Completions format
- Factory pattern: `create_provider(config)` returns boxed trait
- Configuration via TOML with environment variable API key support

## Timeline
- **2026-04**: Provider implementations added
