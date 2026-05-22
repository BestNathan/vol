---
type: concept
category: architecture
tags: [agent-input, multimodal, react-agent, api-compatibility]
created: 2026-05-21
updated: 2026-05-22
source_count: 2
---

# AgentInput Multimodal Run

## Definition

`AgentInput` is the structured run-input envelope for `ReActAgent`. It separates run-level parameters (`run_id`, metadata) from ordered multimodal input parts, while keeping `run(&str)` as a text-only convenience wrapper.

## Key Points

- Text-only callers can continue using `ReActAgent::run(&str)`.
- Structured callers use `ReActAgent::run_input(AgentInput)` for run ID control, metadata, and multimodal parts.
- The first supported input parts are text and image URL/data URL.
- Empty input parts are rejected before an LLM call starts.
- `AgentInput` deserializes from either a legacy JSON string or a structured object, preserving channel compatibility.

## How It Works

`AgentInput` contains an optional `run_id`, a metadata map, and `Vec<InputPart>`. A single text part converts to `MessageContent::Text`; mixed or image-containing input converts to `MessageContent::MultiPart`. This lets provider implementations decide how to serialize content without changing the agent loop interface.

`run_input` converts the envelope into user message content, creates or reuses the run ID, writes metadata into `RunContext.data`, and then executes the existing ReAct loop. `run(&str)` simply builds `AgentInput::text(user_input)` and delegates to `run_input`.

## Examples

```rust
agent.run("summarize this").await?;

let input = AgentInput::new()
    .with_run_id("run-123")
    .text_part("What is in this image?")
    .image_url("https://example.com/chart.png");
agent.run_input(input).await?;
```

For channel clients, both forms are accepted:

```json
{ "input": "hello" }
```

```json
{
  "input": {
    "run_id": "run-123",
    "parts": [
      { "type": "text", "text": "Describe this" },
      { "type": "image_url", "url": "data:image/png;base64,..." }
    ],
    "metadata": { "source": "ui" }
  }
}
```

## Related

- [[agentinput-multimodal-run-implementation]]: original implementation source.
- [[agentinput-channel-unification]]: channel crate unified to use `AgentInput` directly.
- [[vol-llm-agent-crate]]: owns `AgentInput`, `InputPart`, and `run_input`.
- [[vol-llm-provider-crate]]: converts multipart content for Anthropic.
- [[vol-llm-agent-channel-crate]]: `AgentPayload::Submit`, `AgentRequest`, and `AgentDispatcher` all use `AgentInput` directly — no intermediate string conversion.
- [[react-pattern]]: execution loop receiving structured user content.
