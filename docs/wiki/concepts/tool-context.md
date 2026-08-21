---
type: concept
category: framework
tags: [tools, context, execution]
created: 2026-05-04
updated: 2026-08-21
source_count: 2
---

# Tool Context

**Category:** Tool execution context
**Related:** [[tool-trait]], [[tool-registry]], [[react-pattern]]

## Definition

`ToolContext` is the execution context passed to every tool's `execute()` method. It carries the conversation history, a sandbox reference, and the agent definition for tools to make informed decisions.

## Key Points
- `messages: Vec<Message>` — full conversation history available to tools [[agent-tool-design]]
- `sandbox: SandboxRef` — sandbox for tool-side process execution (always set)
- `agent_def: Option<AgentDef>` — the dispatching agent's definition
- Tools can use messages for conversation awareness, the sandbox for isolated side effects, and `agent_def` for identity-aware behavior [[agent-tool-design]]

## How It Works

```rust
pub struct ToolContext {
    pub messages: Vec<Message>,
    pub sandbox: SandboxRef, // Always set
    pub agent_def: Option<vol_llm_core::AgentDef>,
}
```

The context is constructed at tool execution time by the `ToolRegistry.execute()` method. The former `alert` field was removed in the earlier tool-context simplification; this removal deleted the last `vol_core::Alert` type reference. messages give the tool awareness of conversation state, the sandbox scopes side effects, and `agent_def` carries agent identity.

## Related Concepts
- [[tool-trait]]: ToolContext is passed to execute()
- [[tool-registry]]: Constructs ToolContext at dispatch time
- [[run-context]]: Modern replacement for message-heavy context in plugin system
- [[react-pattern]]: Context available during Act phase
