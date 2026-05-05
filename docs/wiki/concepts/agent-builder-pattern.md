---
type: concept
category: pattern
tags: [builder, configuration, fluent-api]
created: 2026-05-04
updated: 2026-05-04
source_count: 1
---

# Agent Builder Pattern

**Category:** Configuration pattern
**Related:** [[react-pattern]], [[agent-plugin-system]]

## Definition

A fluent builder API for constructing a `ReActAgent` with LLM client, tools, plugins, and configuration options.

## Key Points
- Builder pattern ensures all required fields are set before build [[react-agent-docs]]
- Chainable methods for ergonomic configuration [[react-agent-docs]]
- Validates configuration at build time, returning errors for missing fields [[react-agent-docs]]

## How It Works

```rust
let agent = ReActAgent::builder()
    .with_llm(llm_client)
    .with_tool(market_data_tool)
    .with_plugin(observability_plugin)
    .with_agent_id("vol_advice")
    .with_max_iterations(5)
    .with_verbose(true)
    .build()?;
```

The builder accumulates state (LLM, tools, plugins, config) and validates on `build()`. The `AgentConfig` struct holds runtime parameters like max_iterations, system_prompt, and verbose flag.

## Configuration Options

| Method | Purpose | Default |
|--------|---------|---------|
| `with_llm()` | Set LLM provider | Required (no default) |
| `with_tool()` | Register a tool | Empty |
| `with_plugin()` | Register a plugin | Empty |
| `with_agent_id()` | Set agent identifier | Auto-generated |
| `with_max_iterations()` | Set max ReAct cycles | 5 |
| `with_system_prompt()` | Custom system prompt | Default crypto prompt |
| `with_verbose()` | Enable debug logging | false |
| `with_log_base_path()` | Set log directory | `logs/agents` |

## Related Concepts
- [[react-pattern]]: What the builder configures
- [[agent-plugin-system]]: Plugins registered via the builder
- [[vol-llm-agent-crate]]: Where the builder is implemented
- [[skill-system]]: Skills configured via `AgentConfig::with_skills()`
- [[context-builder]]: ContextBuilder configured and enhanced via builder
- [[session-as-ssot]]: Session passed into agent config for message storage
