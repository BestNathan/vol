# Agent & AgentConfig Optimization Design

**Date**: 2026-04-25
**Status**: Draft (rev2)

## Summary

Remove `verbose` and `log_base_path` from AgentConfig/CodingAgentConfig. Add `working_dir` to AgentConfig and derive log paths from it. Remove `context_files` and dead code.

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
| `vol-llm-agents/src/ppt/agent.rs:68,76,90,98,105,116` | Remove 6 `println!` blocks (PptAgent should not print) |

---

## 2. Replace `log_base_path` with `working_dir` Derivation

### Rationale

`log_base_path` is an absolute path configured separately from the project directory. Instead, derive all log paths from `working_dir` using the convention: `{working_dir}/logs/agents/{agent_id}/`.

This follows the **path ownership principle** — all paths derive from a single `working_dir` root.

### Path Convention

```
{working_dir}/logs/agents/{agent_id}/runs/{run_id}.jsonl   — RunLog entries
{working_dir}/logs/agents/{agent_id}/sessions/              — Session entries (FileSessionEntryStore)
```

### Changes

**AgentConfig**: Replace `log_base_path: PathBuf` with `working_dir: PathBuf` (default `"."`).
Log path is derived internally: `working_dir.join("logs/agents")`.

**CodingAgentConfig**: Remove `log_base_path` (already has `working_dir`).
Log path derived: `config.working_dir.join("logs/agents")`.

### Files Affected

| File | Change |
|------|--------|
| `vol-llm-agent/src/react/agent.rs` | Replace `log_base_path` → `working_dir` field; derive `log_base_path = working_dir.join("logs/agents")` internally |
| `vol-llm-agent/src/react/agent.rs:152-157` | `log_base_path.join(&agent_id)` → `config.working_dir.join("logs/agents").join(&agent_id)` |
| `vol-llm-agent/src/react/agent.rs:193` | Same derivation for FileSessionEntryStore |
| `vol-llm-agent/src/react/builder.rs:88-90,96` | Replace `with_log_base_path()` → `with_working_dir()`; observability plugin uses derived path |
| `vol-llm-agents/src/coding/config.rs:29,65,85` | Remove `log_base_path` field |
| `vol-llm-agents/src/coding/agent.rs:175-179` | Remove `with_log_base_path()` method; derive path from `config.working_dir.join("logs/agents")` |
| `vol-llm-agents/src/coding/tests.rs:32,439` | Update tests |
| `vol-llm-agents/tests/observer_plugin_unit.rs:139` | Update to use working_dir |
| `vol-llm-agents/tests/session_recording_test.rs:74` | Remove `.with_log_base_path()` call |
| `vol-llm-agents/tests/agent_run_tests.rs:232` | Replace `.with_log_base_path()` → `.with_working_dir()` |
| `vol-llm-agent/tests/agent_run_tests.rs` (via symlink or same file) | Same |
| `vol-llm-agent/src/react/tests.rs:46` | Replace `.with_log_base_path()` → `.with_working_dir()` |
| `vol-llm-agent/examples/agent_cli_approval.rs:282` | Update example |
| `vol-llm-agent/examples/agent_observability_test.rs:78,88,149` | Update example |
| `vol-llm-agents/examples/coding_agent_basic.rs:40,43,69,97` | Update example |

**Note**: `vol-llm-observability` crate's `ObservabilityConfig` and `RunLogLogger` keep their `log_base_path` parameter — they are generic utilities. The agent layer computes the full path before passing it.

---

## 3. Remove `context_files` from AgentConfig

### Rationale

`AgentConfig::context_files` (field at line 41) is defined but **never read by any code**. CodingAgent has its own `init_context_files()` that uses hardcoded filenames (`AGENT.md`, `INSTRUCTION.md`, `CLI.md`), not this field.

### Files Affected

| File | Change |
|------|--------|
| `vol-llm-agent/src/react/agent.rs:41,68` | Remove `context_files: Vec<String>` field and Default entry |
| `vol-llm-agent/src/react/agent.rs:752` | Remove from test |

---

## 4. Remove Dead Code

| Item | Location | Reason |
|------|----------|--------|
| `ApprovalState::is_pending()` | `vol-llm-tui/src/approval.rs:43` | Never called — only `has_pending_approval()` is used |
| `generate_agent_id()` | `vol-llm-agents/src/coding/agent.rs:419` | Dead function — CodingAgent uses config's agent_id |
