---
type: concept
category: framework
tags: [plugin, context, migration]
created: 2026-05-04
updated: 2026-05-04
source_count: 1
---

# Plugin Context Migration

**Category:** Architectural migration
**Related:** [[run-context]], [[agent-plugin-system]], [[session-as-ssot]]

## Definition

The migration of the `AgentPlugin` trait from using `PluginContext` to using `RunContext` directly. This moved the plugin trait from `vol-llm-core` to `vol-llm-agent` and eliminated the `PluginContext` struct entirely.

## Key Points
- `AgentPlugin` trait moved from `vol-llm-core` to `vol-llm-agent` — plugin is an agent concept, not a core LLM concept [[plugin-context-migration]]
- `PluginContext` struct deleted — all plugin methods now accept `&RunContext` [[plugin-context-migration]]
- `plugin_context_from_run_ctx()` conversion function deleted — no longer needed [[plugin-context-migration]]
- `PluginDecision` simplified: `Continue`, `Skip`, `Abort` (removed `ShortCircuit` variant from intercept) [[plugin-context-migration]]
- `AgentPlugin` trait simplified to 2 hooks: `intercept()` and `listen()` (removed `on_start`, `on_complete`, `on_error`) [[plugin-context-migration]]
- All 4 built-in plugins updated: caching, rate_limiter, retry, hitl [[plugin-context-migration]]
- `vol-llm-observability` dependency removed from `vol-llm-agent` — observability now depends on agent (inverted dependency) [[plugin-context-migration]]
- `SessionRecorderPlugin` moved from `vol-session` to `vol-llm-agent` — session should not know about plugins [[plugin-context-migration]]

## How It Works

### Before

```rust
// PluginContext — separate struct, converted from RunContext
struct PluginContext {
    run_id: String,
    user_input: String,
    session_id: String,
    all_tool_calls: Arc<RwLock<Vec<ToolCall>>>,
    current_tool_calls: Arc<RwLock<Vec<ToolCall>>>,
    data: Arc<RwLock<HashMap<String, Value>>>,
}

trait AgentPlugin {
    async fn intercept(&self, event: &AgentStreamEvent, ctx: &PluginContext) -> PluginDecision;
    async fn listen(&self, event: &AgentStreamEvent, ctx: &PluginContext);
}
```

### After

```rust
// RunContext directly — no conversion needed
trait AgentPlugin {
    async fn intercept(&self, event: &AgentStreamEvent, ctx: &RunContext) -> PluginDecision;
    async fn listen(&self, event: &AgentStreamEvent, ctx: &RunContext);
}
```

The migration involved 10 tasks across multiple crates:
1. Delete `vol-llm-core/src/plugin.rs`, define types in `vol-llm-agent/src/react/plugin.rs`
2. Clean up `RunContext` — delete `plugin_context_from_run_ctx`, remove dead variables
3. Update plugin implementations in `vol-llm-agent` (caching, rate_limiter, retry, hitl)
4. Update all tests (12+ test files)
5. Remove observability module from `vol-llm-agent`, break dependency
6. Move `SessionRecorderPlugin` from `vol-session` to `vol-llm-agent`
7. Update `vol-llm-observability` to depend on `vol-llm-agent`
8. Update `vol-llm-agents` (observer_plugin)
9. Verify `vol-llm-tui`
10. Full workspace build and test

## Related Concepts
- [[run-context]]: The replacement for PluginContext
- [[agent-plugin-system]]: Plugin architecture after migration
- [[session-as-ssot]]: Related message management change
- [[vol-llm-agent-crate]]: Where the migration was centered
