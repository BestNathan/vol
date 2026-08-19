---
type: entity
category: product
tags: [crate, llm, abstraction, rust]
created: 2026-05-04
updated: 2026-08-19
source_count: 5
---

# vol-llm-core Crate

**Category:** Rust crate — LLM interaction abstractions
**Related:** [[vol-llm-agent-crate]], [[vol-llm-provider-crate]], [[vol-llm-tool-crate]], [[agentinput-multimodal-run]]

## Overview

Defines the core abstractions for LLM interaction: message types, conversation requests/responses, tool definitions, streaming, and the `LLMClient` trait.

## Key Facts
- Defines `Message`, `MessageRole`, `ConversationRequest`, `ToolDefinition`, `ToolCall` types
- Defines `LLMClient` trait that all providers must implement
- `test_utils::MockLlmClient` (feature `test-utils`): configurable mock with `set_converse_response`, `set_stream_events`, per-call scripting via `set_stream_event_queue` (VecDeque of event scripts — each `converse_stream` call pops the next, exhausted queue → empty stream), `set_error_at`, and call logging
- Defines `LLMProvider` enum (Anthropic, OpenAI)
- Provider-agnostic: agent code doesn't care which provider is used

## Timeline
- **2026-04**: Initial core types defined
- **2026-05-21**: Multipart message content derives equality for tests and carries `ContentPart::Image`/`ImageUrl` values used by structured agent input [[agentinput-multimodal-run-implementation]]
- **2026-08-17**: `MessageContent::display_text()` renders multipart content with each image part as a `[image]` marker (joined by newlines), so text-only consumers never see raw base64; covered by four unit tests [[multimodal-image-input]]
- **2026-08-17**: Coverage raised to ≥80% (test-only, commit `c0018d89`): 62.93% → 95.62% regions (gate reads the region column), `just cover-gate vol-llm-core 80` PASS; +7 test modules (`agent_def`, `conversation`, `message`, `model`, `provider`, `stream`, `streaming`) [[coverage-gate-work]]
