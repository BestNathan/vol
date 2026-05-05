---
type: concept
category: framework
tags: [tools, context, execution]
created: 2026-05-04
updated: 2026-05-04
source_count: 1
---

# Tool Context

**Category:** Tool execution context
**Related:** [[tool-trait]], [[tool-registry]], [[react-pattern]]

## Definition

`ToolContext` is the execution context passed to every tool's `execute()` method. It provides the current alert, conversation history, and custom metadata for tools to make informed decisions.

## Key Points
- `alert: Option<Alert>` — current alert info for domain-specific tools [[agent-tool-design]]
- `messages: Vec<Message>` — full conversation history available to tools [[agent-tool-design]]
- `metadata: HashMap<String, String>` — custom key-value metadata for extensibility [[agent-tool-design]]
- Tools can use alert info to tailor queries, messages for conversation awareness, or metadata for custom parameters [[agent-tool-design]]

## How It Works

```rust
pub struct ToolContext {
    pub alert: Option<vol_core::Alert>,
    pub messages: Vec<vol_llm_core::Message>,
    pub metadata: std::collections::HashMap<String, String>,
}
```

The context is constructed at tool execution time by the `ToolRegistry.execute()` method. The alert provides domain context (symbol, tenor, alert type), messages give the tool awareness of conversation state, and metadata allows arbitrary custom parameters to be passed through.

## Related Concepts
- [[tool-trait]]: ToolContext is passed to execute()
- [[tool-registry]]: Constructs ToolContext at dispatch time
- [[run-context]]: Modern replacement for message-heavy context in plugin system
- [[react-pattern]]: Context available during Act phase
