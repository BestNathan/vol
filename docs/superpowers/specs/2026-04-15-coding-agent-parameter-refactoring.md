# CodingAgent Parameter Refactoring Spec

> **Goal:** Make CodingAgent a thin, pass-through service facade. Remove env var reads, auto-init sandbox from working_dir, accept LLMClient via config.

---

## Problem

CodingAgent currently:
1. Reads `ANTHROPIC_AUTH_TOKEN` from environment variables in `new()` — couples runtime behavior to env state
2. Hardcodes LLM provider/model/base_url construction — caller cannot swap LLM without forking
3. Never uses `config.working_dir` to initialize a sandbox — tools execute in arbitrary CWD
4. Has `llm_provider_id` in config that only serves to look up a hardcoded provider

ReActAgent is already clean — it takes `Arc<dyn LLMClient>` as a constructor parameter. CodingAgent should follow the same principle.

## Design Principles

- **Pass-through**: CodingAgent is a service facade, not a builder. Config carries everything explicitly.
- **No env vars**: CodingAgent never reads environment variables. If tests need mocks, they create them at the test level.
- **Simple API**: `CodingAgent::new(config)` — config is the only parameter.

## Changes

### 1. `CodingAgentConfig` (`crates/vol-llm-agents/src/coding/config.rs`)

**Add:**
```rust
/// LLM client for generating responses.
/// Caller constructs this; CodingAgent does not read env vars.
pub llm: Option<Arc<dyn vol_llm_core::LLMClient>>,
```

**Remove:**
- `llm_provider_id: String`

### 2. `CodingAgent::new()` (`crates/vol-llm-agents/src/coding/agent.rs`)

**Remove:**
- All `std::env::var("ANTHROPIC_AUTH_TOKEN")` reads
- All `LLMProviderConfig` / `LLMProviderRegistry` construction
- All hardcoded provider/model/base_url

**Add:**
- Read LLM from `config.llm` — return `CodingAgentError::Config("llm not set")` if `None`
- Auto-init sandbox: if `config.working_dir != PathBuf::from(".")`, create `FileSandbox` and pass to ReActAgent via `.with_sandbox()`

### 3. `CodingAgentBuilder` (`crates/vol-llm-agents/src/coding/agent.rs`)

**Add:**
```rust
pub fn llm(mut self, llm: Arc<dyn vol_llm_core::LLMClient>) -> Self {
    self.config.llm = Some(llm);
    self
}
```

**Remove:**
- Any `llm_provider_id()` builder method if it exists

### 4. `CodingAgentConfig::default()`

- `llm` defaults to `None` — forces explicit configuration
- Remove `llm_provider_id` default

### 5. `CodingAgentState` (`crates/vol-llm-agents/src/coding/agent.rs`)

No structural change — `state.llm: Arc<dyn LLMClient>` stays. In `new()`, populate it from `config.llm.clone().unwrap()` instead of constructing from env vars. The sandbox (if created) is stored in `self.sandbox` and passed to ReActAgent via `.with_sandbox()`.

### 6. Examples and Tests

- `coding_agent_basic.rs`: Construct LLM externally, pass via config
- Tests: Create mock LLM at test level, pass via config

## Files Changed

| File | Action |
|------|--------|
| `crates/vol-llm-agents/src/coding/config.rs` | Add `llm` field, remove `llm_provider_id` |
| `crates/vol-llm-agents/src/coding/agent.rs` | Rewrite `new()`, add sandbox init, update builder |
| `crates/vol-llm-agents/examples/coding_agent_basic.rs` | Update to construct LLM externally |
