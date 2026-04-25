# Agent & AgentConfig Optimization Design

**Date**: 2026-04-25
**Status**: Approved

## Summary

Remove `verbose` field from AgentConfig/CodingAgentConfig/PptConfig (9 files), add `working_dir` to AgentConfig, and remove confirmed dead code items.

---

## 1. Remove `verbose` Field

### Rationale

Agent execution logic should not print logs — it should only emit events and handle errors. Debugging is done through observability tools (RunLogLogger, event streams). The `verbose` field and its 3 `tracing::info!` gates in shutdown code are deleted entirely (not downgraded to `debug!`).

### Architectural Principle

**Agent run logic emits events, not logs.** Observability is achieved through:
- `AgentStreamEvent` → plugin system → observability tools
- `tracing::warn!` / `tracing::error!` for real errors only

### Files Affected

| File | Change |
|------|--------|
| `vol-llm-agent/src/react/agent.rs:25` | Remove `verbose: bool` field from AgentConfig |
| `vol-llm-agent/src/react/agent.rs:62` | Remove `verbose: false` from Default |
| `vol-llm-agent/src/react/agent.rs:589-591` | **Delete** `if config.verbose { tracing::info!(...) }` block |
| `vol-llm-agent/src/react/agent.rs:605-607` | **Delete** `if config.verbose { tracing::info!(...) }` block |
| `vol-llm-agent/src/react/agent.rs:625-627` | **Delete** `if config.verbose { tracing::info!(...) }` block |
| `vol-llm-agent/src/react/agent.rs:732,746` | Update tests (remove verbose assertions) |
| `vol-llm-agent/src/react/builder.rs:53-55` | Remove `with_verbose()` method |
| `vol-llm-agent/src/react/plugin_stream.rs:107,117,128` | Remove `verbose` from InterceptorConfig |
| `vol-llm-agent/src/react/tests.rs:42` | Remove `.with_verbose(true)` |
| `vol-llm-agents/src/coding/config.rs:38,68,88` | Remove verbose field, Debug entry, Default |
| `vol-llm-agents/src/coding/tests.rs:34` | Remove `assert!(!config.verbose)` |
| `vol-llm-agents/src/advice/service.rs:158` | Remove `.with_verbose(false)` |
| `vol-llm-agents/src/ppt/config.rs:15,34-36` | Remove verbose field, `with_verbose()` |
| `vol-llm-agents/src/ppt/agent.rs:68,76,90,98,105,116` | Remove 6 `println!` blocks (replace with nothing — PptAgent should emit events, not println) |

---

## 2. Add `working_dir` to AgentConfig

### Rationale

ReActAgent needs a working directory reference for file generation (checkpoints, logs, context files). Currently only CodingAgentConfig has this field.

### Change

```rust
pub struct AgentConfig {
    // ... existing fields ...
    pub working_dir: PathBuf,
}
```

Default: `PathBuf::from(".")`. No behavior change — existing code that uses AgentConfig doesn't reference this field yet. CodingAgentConfig already has `working_dir` — no change needed there.

### Files Affected

| File | Change |
|------|--------|
| `vol-llm-agent/src/react/agent.rs` | Add `working_dir: PathBuf` field, update Default, tests |
| `vol-llm-agent/src/react/builder.rs` | Add `with_working_dir()` method |

---

## 3. Remove Dead Code

| Item | Location | Reason |
|------|----------|--------|
| `ApprovalState::is_pending()` | `vol-llm-tui/src/approval.rs:43` | Never called — only `has_pending_approval()` is used |
| `generate_agent_id()` | `vol-llm-agents/src/coding/agent.rs:419` | Dead function — CodingAgent uses config's agent_id |
| `AgentConfig::context_files` | `vol-llm-agent/src/react/agent.rs:41` | Field defined but never read — CodingAgent has its own `init_context_files()` |
