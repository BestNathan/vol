# Task CLI Tool Gap Filling & Consolidation — Design Spec

**Date**: 2026-06-05
**Status**: design-approved
**Scope**: `vol-llm-task`, `vol-llm-runtime`

## Problem

1. `AgentRuntime` registers **8 task tools** (7 individual `task_xxx` + 1 `task` CLI), but the CLI already covers all operations except `task output --block --timeout`.
2. Having both sets registered wastes tool namespace slots and confuses agents.

## Design

### Change 1: Add `--block` and `--timeout` to `task output` CLI

**Parser** (`crates/vol-llm-task/src/cli/parser.rs`):

```
task output --id 42                     # return existing output immediately
task output --id 42 --block             # wait until task completes, then return output
task output --id 42 --block --timeout 60000  # wait up to 60s
```

New clap args in the `output` subcommand:
```rust
.arg(Arg::new("block").long("block").action(ArgAction::SetTrue).help("Wait for task to complete"))
.arg(Arg::new("timeout").long("timeout").value_parser(value_parser!(u64)).default_value("30000").help("Max wait in ms"))
```

**Commands** (`commands.rs`) — `ParsedCommand::Output` gets two new fields:
```rust
Output {
    id: u64,
    block: bool,
    timeout_ms: u64,
    json: bool,
},
```

**Executor** (`executor.rs`) — When `block=true`, poll task status every 500ms until completed/failed/killed or timeout reached, then read output.

### Change 2: Remove individual `task_xxx` tools from AgentRuntime

**`crates/vol-llm-runtime/src/lib.rs`** — both `build()` and `for_test()`:

Remove `vol_llm_task::tools::register_all(...)` call. Keep only `register_cli(...)`.

```rust
// Before (build() line 271-275):
vol_llm_task::tools::register_all(&mut tool_registry, task_store.clone());
vol_llm_task::tools::register_cli(&mut tool_registry, task_store.clone());

// After:
vol_llm_task::tools::register_cli(&mut tool_registry, task_store.clone());
```

### Unchanged

- 7 individual task tool files — kept on disk (may be used elsewhere), just not registered in AgentRuntime
- `register_all()` function — kept, not deleted
- `TaskHandler` in AgentServerCore — `task.list`/`task.get` JSON-RPC preserved for frontend Tasks panel
- `TaskCliTool` — remains the single agent-facing task tool

### Files Touched

| File | Change |
|------|--------|
| `crates/vol-llm-task/src/cli/parser.rs` | Add `--block`, `--timeout` to output subcommand |
| `crates/vol-llm-task/src/cli/commands.rs` | Add `block`, `timeout_ms` fields to `Output` variant |
| `crates/vol-llm-task/src/cli/executor.rs` | Blocking poll logic in `Output` arm |
| `crates/vol-llm-runtime/src/lib.rs` | Remove `register_all()`, keep `register_cli()` |
