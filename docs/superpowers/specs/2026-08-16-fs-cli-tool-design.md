# FS CLI Tool: Unified CLI for File Op Tools

**Date:** 2026-08-16
**Status:** approved

## Overview

New crate `vol-llm-fs` providing an `fs` tool — a CLI-style single entry point for the five existing file op tools (`read_file`, `write_file`, `edit_file`, `grep`, `glob`), modeled on the `task` CLI tool in `vol-llm-task`. The CLI maps the file op tools' JSON parameters 1:1 onto `--flag`-style subcommands without modifying any file op capability — the executor delegates directly to the existing tools' `execute()`.

## Design

### 1. Crate Layout (mirrors vol-llm-task)

```
crates/vol-llm-fs/
├── Cargo.toml          # deps: vol-llm-tool, 5 file-op sub-crates,
│                       #       clap, serde_json, async-trait, tokio
└── src/
    ├── lib.rs          # pub mod cli; pub mod tools;
    ├── cli/            # mirrors vol-llm-task/src/cli/
    │   ├── mod.rs
    │   ├── parser.rs   #   quote-aware tokenizer + clap command tree + parse() → ParsedCommand
    │   ├── commands.rs #   ParsedCommand enum (one variant per subcommand, typed args)
    │   ├── executor.rs #   dispatch → build tool params JSON → call the 5 tools' execute
    │   └── format.rs   #   text / --json output
    └── tools/          # mirrors vol-llm-task/src/tools/
        ├── mod.rs      #   register_cli(registry)
        └── fs_cli.rs   #   FsCliTool: ExecutableTool (tool name "fs")
```

Correspondence with the task cli:

| task cli (`vol-llm-task`) | fs cli (`vol-llm-fs`) |
|---|---|
| `src/cli/parser.rs` | `src/cli/parser.rs` |
| `src/cli/commands.rs` | `src/cli/commands.rs` |
| `src/cli/executor.rs` (→ TaskStore) | `src/cli/executor.rs` (→ 5 tools' `execute`, zero capability change) |
| `src/cli/format.rs` | `src/cli/format.rs` |
| `src/tools/task_cli.rs` → `TaskCliTool` | `src/tools/fs_cli.rs` → `FsCliTool` |
| `tools::register_cli(registry, store)` | `tools::register_cli(registry)` (no store — stateless) |

The tokenizer (~40 lines, quote-aware) is copied from `vol-llm-task/src/cli/parser.rs` — depending on `vol-llm-task` directly would drag in its store/seaorm dependencies.

`vol-llm-fs` depends on the five file-op sub-crates directly (`vol-llm-tools-builtin-read/write/edit/glob/grep`), not the `vol-llm-tools-builtin` umbrella. `vol-llm-tools-builtin` is **untouched**.

### 2. CLI Command Mapping (all --flags, mirroring task style)

Flag names are identical to the tools' JSON parameter names — a direct 1:1 mapping, no translation or renaming.

```
fs read   --file_path <path> [--offset N] [--limit N]
fs write  --file_path <path> --content <text>
fs edit   --file_path <path> --old_string <s> --new_string <s> [--replace_all]
fs grep   --pattern <regex> [--path <dir>] [--glob <pat>]
          [--output_mode files_with_matches|count|content] [--case_sensitive]
fs glob   --pattern <pat> [--path <dir>] [--exclude a,b] [--kind file|directory|all]
          [--max_results N] [--include_hidden] [--follow_symlinks]
          [--sort path_asc|path_desc|modified_desc|modified_asc] [--with_metadata]
fs scheme [<subcommand>]     # show a subcommand's flag definitions (mirrors task scheme)
```

Parameter mapping (defaults match each tool's serde defaults):

| Subcommand | Tool | Params → flags |
|---|---|---|
| `read` | `ReadParams` | `file_path` (required), `offset` (0), `limit` (2000) |
| `write` | `WriteParams` | `file_path`, `content` (required) |
| `edit` | `EditParams` | `file_path`, `old_string`, `new_string` (required), `replace_all` (flag, default false) |
| `grep` | `GrepParams` | `pattern` (required), `path`, `glob` (optional), `output_mode` (files_with_matches), `case_sensitive` (flag) |
| `glob` | `GlobParams` | `pattern` (required), `path` ("."), `exclude` (comma-separated, optional), `kind` ("file"), `max_results` (100), `include_hidden`, `follow_symlinks`, `with_metadata` (flags), `sort` ("path_asc") |

Details (all borrowed from existing task cli practice):

- **Global flag:** `--json` / `-o` global on every subcommand (same as task). Text mode passes through the tool's result content as-is; JSON mode wraps it in an envelope `{"success": bool, "content": "..."}`.
- **Multi-value params:** `--exclude` uses comma delimiter (`value_delimiter(',')`), same as task's `--deps` / `--blocks`.
- **Boolean flags:** `--replace_all`, `--case_sensitive`, `--include_hidden`, etc. use `ArgAction::SetTrue` (same as task's `--json`).
- **`fs scheme`:** mirrors `task scheme` — no args lists all subcommands; with an arg shows that subcommand's flags and defaults.
- Unsupported parameters are not introduced (YAGNI).

### 3. Data Flow

```
LLM → FsCliTool.execute(args.command, context)
  → parser::parse(command)           # tokenize → clap → ParsedCommand
  → executor::execute(cmd, context)  # dispatch per variant
  → build serde_json::json!({...})   # flag values → tool params JSON (identical names)
  → ReadTool::new().execute(&params, context).await   # direct delegation, zero capability change
  → format (text / --json) → ToolResult
```

Key points:

- **Zero capability change:** the executor instantiates the five tools (`ReadTool` / `WriteTool` / `EditTool` / `GrepTool` / `GlobTool` — stateless unit structs, `new()` suffices) and calls their `ExecutableTool::execute()`. Path resolution, sandbox operations, and the ripgrep backend are all reused as-is; `vol-llm-fs` contains no file op implementation.
- **ToolContext passthrough:** the `context` (agent sandbox) received by `FsCliTool::execute` is passed through unchanged, so sandbox behavior is identical to calling `read_file` etc. directly.

### 4. Registration

```rust
// vol-llm-runtime/src/lib.rs — AgentRuntimeBuilder::build()
vol_llm_task::tools::register_cli(&mut tool_registry, task_store.clone());
vol_llm_fs::tools::register_cli(&mut tool_registry);   // ← new line
```

- `register_cli` mirrors task exactly: a thin wrapper that calls `registry.register(FsCliTool::new())` (no explicit collision check, same as `vol_llm_task::tools::register_cli`).
- Coexists with the five original tools; agents select via `tools: [...]`.
- No `ToolConfig` needed — `FsCliTool` is stateless; the sandbox comes from `ToolContext`.

### 5. Error Handling (same layering as task_cli.rs)

| Error | Mapping |
|---|---|
| Missing `command` param | `ToolError::InvalidArguments` (with usage hint) |
| clap parse failure (unknown subcommand / missing required flag / invalid value) | `ToolError::InvalidArguments` (clap error message passed through, same as task) |
| Underlying tool failure (file not found, sandbox error, etc.) | `ToolError::ExecutionFailed` (tool error passed through) |

### 6. Sensitivity

All `Safe` — no `sensitivity()` override (matches the current behavior of the five file op tools; no approval gates).

### 7. Testing

All tests are `#[cfg(test)]` unit tests in `vol-llm-fs`; no doc tests.

1. **Parser tests:** tokenizer (quotes / escapes / consecutive spaces); flag→field mapping per subcommand; defaults (offset/limit/output_mode/max_results/sort, etc.); missing-required errors; unknown-subcommand errors; global `--json`.
2. **Executor tests:** real-file round-trips via `ToolContext::for_test()` (sandbox rooted at `/`) + `tempfile` — `fs write` then `fs read` reads back, `fs edit` replaces (including the multi-occurrence ambiguity error path without `--replace_all`), `fs grep` matches, `fs glob` pattern matching.
3. **FsCliTool tests** (mirror `task_cli.rs` tests): name/description/parameters require `command`; missing command error; invalid subcommand error; full flow; `--json` output starts with `{`; sensitivity Safe for all subcommands.
4. **Format tests:** text passthrough vs JSON envelope serialization.
5. **Gates:** `just cover-gate vol-llm-fs 80` passes; `./scripts/check-no-doc-tests.sh` passes.

### 8. Files Touched

| File | Action |
|---|---|
| `crates/vol-llm-fs/**` | New (Cargo.toml + src/lib.rs + cli/{mod,parser,commands,executor,format}.rs + tools/{mod,fs_cli}.rs) |
| `Cargo.toml` (workspace) | Add member + workspace deps |
| `crates/vol-llm-runtime/src/lib.rs` | `build()`: add one `register_cli` line |
| `crates/vol-llm-tools-builtin/**` | **Untouched** |
