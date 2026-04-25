# CodingAgent + LoggerPlugin Integration Design

**Date**: 2026-04-25
**Status**: Draft

## Summary

Add `with_logger()` builder method on `CodingAgentBuilder` to register `LoggerPlugin` into the plugin registry. Migrate TUI's `spawn_agent` from direct `CodingAgentConfig` construction to the builder pattern, adopting `with_logger()`.

## Current State

- `LoggerPlugin` is fully implemented in `vol-llm-observability` — writes JSONL events to `{base_dir}/logs/{run_id}.jsonl`
- TUI creates `CodingAgent` via `CodingAgent::new(config)` with a manually constructed `CodingAgentConfig`
- `CodingAgentBuilder` already exists with methods for `working_dir`, `store_dir`, `max_iterations`, `llm`, `sandbox`, etc.
- TUI uses direct `CodingAgentConfig` because it has an already-built `session` to pass

## Design

### 1. `CodingAgentBuilder::with_logger()`

```rust
impl CodingAgentBuilder {
    pub fn with_logger(mut self) -> Self {
        let logger = vol_llm_observability::LoggerPlugin::new(self.config.store_dir.clone());
        self.config.plugin_registry.register(logger);
        self
    }
}
```

### 2. Builder gains `session()` and `llm_provider_id()` methods

TUI currently passes `session` and `tool_config` via `CodingAgentConfig`. Add builder methods for these:

```rust
pub fn session(mut self, session: Arc<vol_session::Session>) -> Self {
    self.config.session = Some(session);
    self
}

pub fn llm_provider_id(mut self, id: String) -> Self {
    self.config.llm_provider_id = id;
    self
}
```

### 3. TUI `spawn_agent` migrates to builder

Replace the current direct config construction + `CodingAgent::new(config)` with:

```rust
let agent = CodingAgentBuilder::new()
    .working_dir(working_dir)
    .store_dir(store_dir)
    .max_iterations(10)
    .session(session)
    .hitl_enabled(true)
    .unsafe_mode(unsafe_mode)
    .approval_handler(approval_state.into_handler())
    .tool_config(tool_config)
    .with_logger()
    .build()
    .await?;
```

## Files Changed

| File | Change |
|------|--------|
| `crates/vol-llm-agents/src/coding/agent.rs` | Add `with_logger()`, `session()`, `llm_provider_id()` builder methods |
| `crates/vol-llm-tui/src/main.rs` | Migrate `spawn_agent` to builder + `with_logger()` |

## Dependencies

- `vol_llm_observability::LoggerPlugin` already available
