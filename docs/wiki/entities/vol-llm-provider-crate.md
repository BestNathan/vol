---
type: entity
category: product
tags: [crate, provider, anthropic, openai, rust]
created: 2026-05-04
updated: 2026-08-17
source_count: 5
---

# vol-llm-provider Crate

**Category:** Rust crate — LLM provider implementations
**Related:** [[vol-llm-core-crate]], [[vol-llm-agent-crate]], [[dashscope]], [[agentinput-multimodal-run]]

## Overview

Implements the `LLMClient` trait for Anthropic and OpenAI providers, handling protocol conversion between the unified message format and each provider's API.

## Key Facts
- `AnthropicProvider`: converts to/from Anthropic Messages API format
- `OpenAIProvider`: converts to/from OpenAI Chat Completions format
- Factory pattern: `create_provider(config)` returns boxed trait
- Configuration via TOML with environment variable API key support

## Timeline
- **2026-04**: Provider implementations added
- **2026-05-21**: Anthropic user-content conversion preserves multipart text/image input, mapping URL images and base64 data URLs to provider-native content blocks [[agentinput-multimodal-run-implementation]]
- **2026-08-17**: OpenAI provider converts multipart user content into vision content arrays (`text` + `image_url` parts) for Chat Completions, completing image support for both provider families; text-only streaming verified as a no-op [[multimodal-image-input]]
- **2026-08-17**: Coverage raised to ≥80% (test-only, commits `3846cee7`, `9600911d`): 60.81% region / 63.31% line → 85.79%, +40 unit and +20 integration tests [[coverage-gate-work]]
- **2026-08-17**: Four production bugfixes (TDD, commits `72277b0a`, `9fa770f0`, `6f56f327`, `de04fe83`): raw string tool-call arguments in non-streaming OpenAI converse (`parse_tool_arguments`); `request.system` forwarded as first `system` message in both OpenAI converse paths without duplicating a leading caller system message (`apply_system_prompt`); `Secret` JSON deserialization now accepts both tagged and plain-string forms, making `LLMConfig` round-trips symmetric with TOML config path intact; streamed OpenAI tool calls flush `ToolCallComplete` at stream end via `session.apply(&ParsedEvent::ContentBlockStop)` (SSE has no per-block stop marker; no-op without pending tool calls). Gate re-verified `just cover-gate vol-llm-provider 80` PASS 95.41%; suite 120 passed / 0 failed [[provider-bugfixes]]
