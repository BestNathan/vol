---
type: entity
category: product
tags: [crate, llm, abstraction, rust]
created: 2026-05-04
updated: 2026-05-21
source_count: 2
---

# vol-llm-core Crate

**Category:** Rust crate — LLM interaction abstractions
**Related:** [[vol-llm-agent-crate]], [[vol-llm-provider-crate]], [[vol-llm-tool-crate]], [[agentinput-multimodal-run]]

## Overview

Defines the core abstractions for LLM interaction: message types, conversation requests/responses, tool definitions, streaming, and the `LLMClient` trait.

## Key Facts
- Defines `Message`, `MessageRole`, `ConversationRequest`, `ToolDefinition`, `ToolCall` types
- Defines `LLMClient` trait that all providers must implement
- Defines `LLMProvider` enum (Anthropic, OpenAI)
- Provider-agnostic: agent code doesn't care which provider is used

## Timeline
- **2026-04**: Initial core types defined
- **2026-05-21**: Multipart message content derives equality for tests and carries `ContentPart::Image`/`ImageUrl` values used by structured agent input [[agentinput-multimodal-run-implementation]]
