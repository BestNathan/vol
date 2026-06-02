# AgentConfig Consolidation & Contributor SOT Design

**Date**: 2026-06-01
**Status**: Approved

## Summary

Three related improvements to make `AgentConfig` the single source of truth for agent resources:

1. **Delete `AgentConfig::new()`** — keep only `builder()` as the constructor, eliminating the dual-path problem where `new()` skipped default contributor injection
2. **Agent as contributor SOT** — `ReActAgent` exposes `add_contributor()` / `contributors()` methods for external use
3. **Stable agent list sorting** — scope-grouped + alphabetical ordering in `agent.list`

---

## 1. AgentConfig: remove `new()`, add contributor API

**File**: `crates/vol-llm-agent/src/react/agent.rs`

- Delete `AgentConfig::new()` — `builder()` is the only constructor
- `context_builder` visibility: `pub` → `pub(crate)` — external access through methods only
- Add `add_contributor()` and `contributor_infos()` methods

## 2. AgentConfigBuilder::build() — unified default injection

**File**: `crates/vol-llm-agent/src/react/config_builder.rs`

`build()` injection order:

1. Clone existing context_builder if provided, else create default (128k tokens)
2. **System prompt** — if `def.prompt` is non-empty, inject `SimpleContributor::system(def.prompt)` first (Head(0))
3. **SkillInjector** — always injected once
4. **Manual contributors** — from `with_system_prompt()` / `with_contributor()` calls

This eliminates the double `SkillInjector` bug (builder injected one, `server_core.rs` injected another).

## 3. ReActAgent — contributor API (SOT)

**File**: `crates/vol-llm-agent/src/react/agent.rs`

```rust
impl ReActAgent {
    pub fn add_contributor(&mut self, contributor: Box<dyn ContextContributor>);
    pub fn contributors(&self) -> Vec<ContributorInfo>;
    pub fn contributor_names(&self) -> Vec<String>;
}
```

All external code queries contributor info through the agent, not through `config.context_builder` directly.

## 4. Agent list stable sorting

**File**: `crates/vol-llm-agent-channel/src/domain/agent.rs`

`agent.list` handler sorts results before returning:

- **Primary key**: scope order — `builtin(0) > repo(1) > external(2)`
- **Secondary key**: name alphabetical

## 5. Caller updates

### server_core.rs

- `register_agent()`: switch from `AgentConfig::new()` to `AgentConfig::builder()...build()`
- Remove manual `context_builder.add_contributor(SkillInjector::new(...))` — builder handles it
- `for_test()`: same switch

### vol-llm-runtime/src/lib.rs

- `build_agent()`: switch from `AgentConfig::new()` to builder — this path was also missing SkillInjector + system prompt

### Test code

- `agent.rs:775` (test module): switch to builder
- `server_core.rs:495` (`for_test`): switch to builder

## 6. RPC impact

- `agent.context_config` / `agent.context_snapshot` already read from `context_builder` — they continue to work, now returning the full set (system prompt + skills + any dynamically added contributors)
- No protocol changes needed

## Files touched

| File | Change |
|------|--------|
| `crates/vol-llm-agent/src/react/agent.rs` | Delete `new()`, add contributor methods |
| `crates/vol-llm-agent/src/react/config_builder.rs` | `build()` auto-injects system prompt + skills |
| `crates/vol-llm-agent-channel/src/domain/agent.rs` | Sort `agent.list` results |
| `crates/vol-llm-agent-channel/src/server_core.rs` | Switch to builder, remove manual SkillInjector |
| `crates/vol-llm-agent-channel/src/server_core.rs` (`for_test`) | Switch to builder |
| All other `AgentConfig::new()` call sites | Switch to builder |
