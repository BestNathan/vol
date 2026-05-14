---
type: concept
category: framework
tags: [context, state, run-lifecycle]
created: 2026-05-04
updated: 2026-05-06
source_count: 3
---

# Run Context

**Category:** Agent run state management
**Related:** [[session-as-ssot]], [[context-builder]], [[agent-plugin-system]], [[tool-registry]], [[otel-log-routing]]

## Definition

`RunContext` encapsulates all state and resources for a single `ReActAgent::run()` invocation. It provides immutable fields (run_id, user_input, session_id, model), mutable fields with internal mutability (iteration, tool_calls, data), and resource references (session, tools, config).

## Key Points
- Replaced the older `PluginContext` — the `AgentPlugin` trait now accepts `&RunContext` directly [[plugin-context-migration]]
- Immutable fields: `run_id`, `user_input`, `session_id`, `model` — fixed at run start [[run-context]]
- **`model` field**: The LLM model name used for this run, extracted from `config.llm.model()`. Empty string normalized to `"unknown"`. Enables observability plugins to include model identity in logs [[loki-plugin-otel-migration-tasks-3-4]].
- Mutable fields use `AtomicU32` (iteration) and `Arc<RwLock<>>` (tool_calls, data) for safe sharing across async tasks [[run-context]]
- Resource references: `session: Arc<Session>`, `tools: Arc<ToolRegistry>`, `config: AgentConfig` [[run-context]]
- Plugin data storage via typed `get<T>()` / `set<T>()` methods with serde serialization [[run-context]]
- Implements `Clone` — shares underlying Arcs, copies immutable fields [[run-context]]

## How It Works

`RunContext` is created at the start of `ReActAgent::run()`:

```rust
let ctx = RunContext::new(
    run_id, user_input, session_id,
    session, tools, config,
);
```

It is cloned and passed to spawned tasks (agent loop, plugin interceptor, plugin listener). The clone shares all Arc references — mutations in one clone are visible to all others.

Key methods:
- `next_iteration()` / `current_iteration()` — atomic iteration tracking
- `add_message()` / `get_messages()` — message management (now writes to Session only) [[session-as-ssot]]
- `add_tool_call()` / `get_current_tool_calls()` / `get_all_tool_calls()` — tool call tracking
- `get<T>(key)` / `set<T>(key, value)` — plugin data storage

The migration from `PluginContext` to `RunContext` involved:
1. Moving `AgentPlugin` trait from `vol-llm-core` to `vol-llm-agent` (plugin is an agent concept, not a core LLM concept) [[plugin-context-migration]]
2. Deleting `PluginContext` struct entirely
3. Updating all plugin implementations to accept `&RunContext`
4. Removing `plugin_context_from_run_ctx()` conversion function
5. Cleaning up dead variables in `agent.rs`

## Examples / Applications

- **Plugin interceptors**: Access `ctx.run_id`, `ctx.user_input`, `ctx.current_iteration()` for logging
- **Plugin data sharing**: `ctx.set("cache_key", value)` / `ctx.get("cache_key")` for cross-plugin state
- **Listener plugins**: Receive `ctx.clone()` in spawned tasks for async event logging
- **HITL plugin**: Uses `ctx.tools` to access available tool definitions for approval decisions

## Related Concepts
- [[session-as-ssot]]: RunContext holds Session reference
- [[context-builder]]: RunContext uses ContextBuilder for context construction
- [[agent-plugin-system]]: Plugins receive RunContext in hook signatures
- [[plugin-context-migration]]: Migration from PluginContext to RunContext
- [[tool-registry]]: RunContext holds tools reference
- [[vol-llm-agent-crate]]: Where RunContext is defined
- [[otel-log-routing]]: `model` field used in structured log enrichment
- [[loki-plugin-otel-migration-tasks-3-4]]: Implementation adding `model` field
