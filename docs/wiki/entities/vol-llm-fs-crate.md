---
type: entity
category: service
tags: [fs, tools, cli, file-operations]
created: 2026-08-16
updated: 2026-08-16
source_count: 1
---

# vol-llm-fs Crate

## Overview
`vol-llm-fs` provides the `fs` tool — a single CLI-command-string entry point over the five builtin file-op tools (read/write/edit/grep/glob), modeled on the `task` CLI in `vol-llm-task`. It delegates to the builtin tools' implementations and contains no file-op logic of its own.

## Key Facts
- `FsCliTool` implements `ExecutableTool` (name `fs`); `tools::register_cli(registry)` registers it on a `ToolRegistry`.
- Registered from `AgentRuntimeBuilder::build()` next to the task tool (primary place); `AgentRuntimeBuilder::for_test()` mirrors it.
- Subcommands: `read` (`--file_path`, `--offset` default 0, `--limit` default 2000), `write` (`--file_path`, `--content`), `edit` (`--file_path`, `--old_string`, `--new_string`, `--replace_all`), `grep` (`--pattern`, `--path`, `--glob`, `--output_mode`, `--case_sensitive`), `glob` (`--pattern`, `--path` default `.`, `--exclude`, `--kind`, `--max_results`, `--include_hidden`, `--follow_symlinks`, `--sort`, `--with_metadata`), `scheme [<subcommand>]`.
- Global `--json`/`-o` flag wraps results in a JSON envelope `{"success": bool, "content": string}` via `cli::format::envelope`.
- Flag names are identical to the builtin tools' JSON parameter names (no renaming); no sensitivity override — all subcommands are `Safe`.
- `vol-llm-tools-builtin` is untouched; the `fs` tool coexists with the five original tools.

## Module Structure
```
crates/vol-llm-fs/
├── Cargo.toml           # tokio(fs), serde_json, async-trait, tracing, clap 4 ["string"],
│                        # vol-llm-tool (workspace), 5 builtin file-op sub-crates (path)
└── src/
    ├── lib.rs           # pub mod cli; pub mod tools;
    ├── cli/
    │   ├── commands.rs  # ParsedCommand enum (one variant per subcommand)
    │   ├── parser.rs    # quote-aware tokenizer + clap build_cli + parse
    │   ├── executor.rs  # dispatch → builtin tool execute() delegation + scheme_for
    │   └── format.rs    # fmt_scheme + JSON envelope
    └── tools/
        ├── mod.rs       # register_cli(registry)
        └── fs_cli.rs    # FsCliTool: ExecutableTool
```

## Design Notes
- The parser flow: `tokenize` (quote-aware, copied from `vol-llm-task/src/cli/parser.rs`) → `build_cli` (clap 4 tree) → `parse` (accepts optional `fs` prefix, produces `ParsedCommand`).
- The executor maps each `ParsedCommand` variant to the builtin tool's JSON params, calls `execute()`, and applies the JSON envelope when `--json` is set; edit ambiguity without `--replace_all` surfaces the builtin tool's error.
- `fs scheme` introspects parameter lists (flag, required marker, description) via `format::fmt_scheme`.

## Quality
- 32 unit tests (parser 12, executor 11, format 3, fs_cli 6) — every `pub fn` has at least one test; no doc tests.
- Line coverage 89.81% (TOTAL via `cargo llvm-cov --package vol-llm-fs --summary-only`) — passes the 80% gate.
- `cargo test -p vol-llm-fs -p vol-llm-runtime` and `cargo check` both green.

## Related
- [[vol-llm-runtime-crate]] — registration in `AgentRuntimeBuilder::build()` / `for_test()`
- [[vol-llm-task-crate]] — the `task` CLI this tool is modeled on (sibling CLI-style tool)
- [[vol-llm-tool-crate]] — `ExecutableTool`, `ToolContext`, `ToolResult`, `ToolRegistry`, `ToolSensitivity`
- [[cli-style-tool-pattern]] — the CLI-command-string tool pattern
- [[fs-cli-tool]] — source page for this implementation
