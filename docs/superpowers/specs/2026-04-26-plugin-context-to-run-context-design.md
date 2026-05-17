# Design: PluginContext → RunContext Migration

## Problem

1. **Dead variables in `agent.rs:run()`** (lines 231-234): `_session_id`, `_session`, `_run_id_clone` are cloned but never used.
2. **PluginContext duplicates RunContext data**: `PluginContext` manually copies 6 fields from `RunContext` via `plugin_context_from_run_ctx()`.
3. **Plugin concept misplaced**: `AgentPlugin` trait lives in `vol-llm-core` (base package), but plugins are an agent concept.
4. **Agent depends on ob package**: `vol-llm-agent` has `vol-llm-observability` in Cargo deps. The `observability/` module in agent just wraps/re-exports ob types — this belongs in ob, not agent.

## Design

### Principle: Plugin is an agent concept; observability is a separate concern

- `AgentPlugin` trait and `RunContext` belong in `vol-llm-agent`.
- `vol-llm-core` should contain only LLM primitives (Message, ToolCall, LLMClient, streaming).
- `vol-llm-agent` should NOT depend on `vol-llm-observability`. All observability code moves to `vol-llm-observability`.
- `LoggerPlugin`'s `AgentPlugin` impl lives in `vol-llm-observability` (which depends on `vol-llm-agent`).
- `vol-session` and `vol-llm-observability` become pure library crates with no plugin awareness.

### Architecture Change

**Before:**
```
vol-llm-core          → defines AgentPlugin trait, PluginContext
vol-session           → depends on vol-llm-core (AgentPlugin, PluginContext)
                        → has SessionRecorderPlugin (AgentPlugin impl)
vol-llm-observability → depends on vol-llm-core (AgentPlugin, PluginContext)
                        → has LoggerPlugin (AgentPlugin impl)
vol-llm-agent         → depends on vol-llm-observability + vol-session
                        → has ObservabilityPlugin wrapper in observability/ module
```

**After:**
```
vol-llm-core          → LLM primitives only (no plugin types)
vol-session           → session management only (no plugin types, no AgentPlugin)
vol-llm-observability → logging only (no plugin types from core)
                        → depends on vol-llm-agent (for AgentPlugin trait + RunContext)
                        → contains LoggerPlugin AgentPlugin impl
vol-llm-agent         → defines AgentPlugin trait + RunContext
                        → contains ALL plugin implementations (caching, hitl, retry, rate_limiter, session_recorder, logger)
                        → depends on vol-session (for Session)
                        → NO dep on vol-llm-observability
                        → NO observability/ module
vol-llm-agents        → depends on vol-llm-agent
                        → contains ObserverPlugin (depends on vol-llm-agent)
```

### Changes by File

#### 1. Move `AgentPlugin` trait from vol-llm-core to vol-llm-agent

- **DELETE `vol-llm-core/src/plugin.rs`** entirely.
- **EDIT `vol-llm-core/src/lib.rs`** — Remove `pub mod plugin` and `pub use plugin::*`.
- **REWRITE `vol-llm-agent/src/react/plugin.rs`** — Define trait + types here (move from core). Change trait signatures from `&PluginContext` to `&RunContext`:

```rust
// Before:
async fn intercept(&self, _event: &AgentStreamEvent, _ctx: &PluginContext) -> PluginDecision
async fn listen(&self, _event: &AgentStreamEvent, _ctx: &PluginContext)

// After:
async fn intercept(&self, _event: &AgentStreamEvent, _ctx: &RunContext) -> PluginDecision
async fn listen(&self, _event: &AgentStreamEvent, _ctx: &RunContext)
```

#### 2. Delete PluginContext, clean up RunContext

- **`vol-llm-agent/src/react/run_context.rs`**:
  - Delete `plugin_context_from_run_ctx()` function.
  - Remove `use vol_llm_core::PluginContext` import.
- **`vol-llm-agent/src/react/mod.rs`**:
  - Remove `PluginContext` from plugin re-exports.
  - Remove `plugin_context_from_run_ctx` from run_context re-exports.
- **`vol-llm-agent/src/react/agent.rs`**:
  - Delete dead variables: `_session_id`, `_session`, `_run_id_clone`.
  - Replace `plugin_context_from_run_ctx(&run_ctx)` with `run_ctx.clone()` at call sites.
  - Remove `plugin_context_from_run_ctx` from imports.

#### 3. Update plugin_stream.rs

- **`vol-llm-agent/src/react/plugin_stream.rs`**:
  - `spawn_listener_task`: Change `plugin_ctx: PluginContext` → `ctx: RunContext`.
  - `run_interceptor_loop`: Change `plugin_ctx: PluginContext` → `ctx: &RunContext`.
  - Update comments referencing PluginContext.

#### 4. Remove observability module from vol-llm-agent

- **DELETE `vol-llm-agent/src/observability/`** entire directory.
- **EDIT `vol-llm-agent/src/lib.rs`**:
  - Remove `pub mod observability`.
  - Remove `pub use observability::{append_log, LogEntry, LoggerPlugin, ObservabilityPlugin}`.
- **EDIT `vol-llm-agent/Cargo.toml`**:
  - Remove `vol-llm-observability = { path = "../vol-llm-observability" }`.
- **EDIT `vol-llm-agent/src/react/builder.rs`**:
  - Remove `with_observability_plugin()` method.
- **DELETE/UPDATE** examples that call `.with_observability_plugin()`:
  - `vol-llm-agent/examples/agent_cli_approval.rs` — remove `.with_observability_plugin()` call.
  - `vol-llm-agent/examples/agent_observability_test.rs` — remove observability references.
- **DELETE/UPDATE** `vol-llm-agent/tests/observability_integration.rs`.

#### 5. Move LoggerPlugin AgentPlugin impl to vol-llm-observability

- **`vol-llm-observability/src/plugin.rs`** — Keep LoggerPlugin struct as-is. Replace `AgentPlugin` impl to use `&RunContext` instead of `&PluginContext`. Update imports: `use vol_llm_agent::react::{AgentPlugin, RunContext, PluginDecision}`.
- **`vol-llm-observability/Cargo.toml`** — Add `vol-llm-agent` dependency.

#### 6. Move SessionRecorderPlugin from vol-session to vol-llm-agent

- **DELETE `vol-session/src/recorder.rs`** (or move impl to vol-llm-agent).
- **CREATE `vol-llm-agent/src/plugins/session_recorder.rs`** — Contains `SessionRecorderPlugin` AgentPlugin impl.
- **`vol-llm-session/Cargo.toml`** — Remove `async-trait` and `vol-llm-core` if only used for plugin.

#### 7. Update plugin implementations in vol-llm-agent

- **`vol-llm-agent/src/plugins/caching.rs`**: Change trait impl from `&PluginContext` → `&RunContext`. Tests use `RunContext` directly.
- **`vol-llm-agent/src/plugins/rate_limiter.rs`**: Same.
- **`vol-llm-agent/src/plugins/retry.rs`**: Same.
- **`vol-llm-agent/src/react/hitl.rs`**: Same. Remove `use super::plugin::PluginContext`.

#### 8. Update vol-llm-agents

- **`vol-llm-agents/src/coding/observer_plugin.rs`**: Change `&PluginContext` → `&RunContext`. Update import: `use vol_llm_agent::react::RunContext`.

#### 9. Update vol-llm-tui

- TUI uses `vol_llm_observability::LogEntry` directly — no PluginContext usage. Should be unaffected.

#### 10. Update all tests

All test files that construct `PluginContext` → use `RunContext` directly:
- `vol-llm-agent/src/react/tests.rs`
- `vol-llm-agent/src/plugins/caching.rs` (tests)
- `vol-llm-agent/src/plugins/rate_limiter.rs` (tests)
- `vol-llm-agent/src/plugins/retry.rs` (tests)
- `vol-llm-agent/src/observability/plugin.rs` (tests) — DELETED with module
- `vol-llm-agent/tests/plugin_test.rs`
- `vol-llm-agent/tests/plugin_flow_test.rs`
- `vol-llm-agent/tests/agent_run_tests.rs`
- `vol-llm-agent/tests/react_mock_test.rs`
- `vol-llm-agent/tests/code_agent_simulation.rs`
- `vol-llm-agent/tests/session_recording_test.rs`
- `vol-llm-agent/tests/observability_integration.rs` — DELETED
- `vol-llm-observability/src/plugin.rs` (tests)
- `vol-session/src/recorder.rs` (tests) — MOVED
- `vol-llm-agents/tests/observer_plugin_unit.rs`

### Dependency Changes

**`vol-llm-agent/Cargo.toml`**:
- **REMOVE** `vol-llm-observability`
- Keep `vol-session` (for Session)

**`vol-llm-observability/Cargo.toml`**:
- **ADD** `vol-llm-agent` (for AgentPlugin trait + RunContext)
- Remove `vol-llm-core` plugin imports

**`vol-session/Cargo.toml`**:
- Remove `async-trait` dep if only used for SessionRecorderPlugin
- Keep `vol-llm-core` for `Message`/`ToolCall` types (used in SessionMessage)

### RunContext Clone in Agent Loop

The agent loop is inside `tokio::spawn`. Current code clones `RunContext` for listener and interceptor tasks via `plugin_context_from_run_ctx(&run_ctx)`. After this change, just use `run_ctx.clone()` directly.
