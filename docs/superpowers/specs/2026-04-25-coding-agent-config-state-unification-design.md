# CodingAgent Config & State Unification Design

**Date**: 2026-04-25
**Status**: Draft

## Summary

Delete `CodingAgentState` and flatten its fields onto `CodingAgent`. `AgentConfig` becomes a build product (constructed per-call via a helper) rather than a stored field. No public API changes.

---

## 1. Current Problem

`CodingAgent` wraps an inner `CodingAgentState` that holds the same logical data as `CodingConfig`:

```
CodingAgent {
    config: CodingAgentConfig,        // user input
    state: Option<CodingAgentState>,  // llm, tool_registry, agent_config
    observer, sandbox,
}
```

This causes duplication:
- `with_agent_id()` updates both `config.agent_id` and `state.agent_config.agent_id`
- `run()` merges `config.plugin_registry + state.agent_config` into a fresh `AgentConfig` every call
- `Option<CodingAgentState>` implies it can be `None` (consumed), but it's always `Some` after `new()`

---

## 2. New Structure

```rust
pub struct CodingAgent {
    config: CodingAgentConfig,
    llm: Arc<dyn vol_llm_core::LLMClient>,
    tool_registry: Arc<ToolRegistry>,
    context_builder: ContextBuilder,
    observer: Option<Arc<dyn EventObserver>>,
    sandbox: Option<vol_llm_core::SandboxRef>,
}
```

- **`CodingAgentState` is deleted.**
- `new()` consumes `CodingAgentConfig`, resolves the LLM, builds the tool registry and context builder, and stores them as direct fields.
- `AgentConfig` is **not** stored. It's built on-demand per `run()` call.

---

## 3. AgentConfig Build Helper

```rust
impl CodingAgent {
    fn build_agent_config(&self) -> AgentConfig {
        AgentConfig {
            max_iterations: self.config.max_iterations,
            max_history_messages: 20,
            context_builder: self.context_builder.clone(),
            plugin_registry: self.config.plugin_registry.clone(),
            agent_id: self.config.agent_id.clone(),
            working_dir: self.config.working_dir.clone(),
            unsafe_mode: self.config.unsafe_mode,
            approval_handler: self.config.approval_handler.clone(),
        }
    }
}
```

`run()` and `resume()` call `self.build_agent_config()` to create the temporary `AgentConfig`, pass it to `ReActAgent::new()`, and let it be dropped after the run completes. No more merging config + state fields.

---

## 4. Builder Method Simplification

`with_agent_id()` no longer syncs two structs:

```rust
pub fn with_agent_id(mut self, agent_id: String) -> Self {
    self.config.agent_id = agent_id;
    self
}
```

---

## 5. Files Changed

| File | Change |
|------|--------|
| `vol-llm-agents/src/coding/agent.rs` | Delete `CodingAgentState`; flatten fields; add `build_agent_config()`; simplify `with_agent_id()`, `run()`, `resume()` |
| `vol-llm-agents/src/coding/tests.rs` | Update any test assertions that reference the old `state` struct |

No public API surface changes — `CodingAgentConfig`, `CodingAgentBuilder`, and method signatures remain the same.
