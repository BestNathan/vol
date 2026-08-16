---
type: source
source_type: code
date: 2026-08-16
ingested: 2026-08-16
tags: [fs, tools, cli, file-operations]
---

# vol-llm-fs — Unified CLI-Style `fs` Tool

**Authors/Creators:** vol repo implementation (Tasks 1-6 of the FS CLI tool plan)
**Date:** 2026-08-16
**Link:** `crates/vol-llm-fs/`

## TL;DR
The new `vol-llm-fs` crate provides the `fs` tool — a single CLI-command-string entry point over the five builtin file-op tools (read/write/edit/grep/glob), modeled on the `task` CLI in `vol-llm-task`. It contains no file-op logic of its own: the parser maps `--flag`-style subcommands 1:1 onto the builtin tools' JSON parameters, and the executor delegates to `ReadTool`/`WriteTool`/`EditTool`/`GrepTool`/`GlobTool::execute()`. It is registered from `AgentRuntimeBuilder::build()` (and `for_test()`) next to the task tool and coexists with the five original tools.

## Key Takeaways
- **Scope:** `fs read|write|edit|grep|glob|scheme` subcommands with flags identical to the tools' JSON parameter names; global `--json`/`-o` flag wraps results in a `{"success": bool, "content": string}` JSON envelope.
- **Structure:** mirrors `vol-llm-task`'s CLI layout — `cli/{commands,parser,executor,format}` (all `pub(crate)`) plus `tools/` exposing `FsCliTool` (`ExecutableTool`) and `register_cli(registry)`.
- **Parser:** hand-rolled quote-aware tokenizer (~40 lines, copied from `vol-llm-task`; `vol-llm-fs` must NOT depend on `vol-llm-task`) + clap 4 (`features = ["string"]`) definition; `parse()` accepts an optional `fs` prefix.
- **Executor:** pure dispatch — each `ParsedCommand` variant builds the builtin tool's JSON params, calls its `execute()`, and runs `finalize()` which applies the JSON envelope when `--json` is set.
- **Scheme:** `fs scheme [<subcommand>]` introspects parameter lists (flag, required marker, description) from `format::fmt_scheme`, with an unknown-subcommand arm and a no-arg subcommand listing.
- **Sensitivity:** no override — all subcommands report `ToolSensitivity::Safe`.
- **Registration:** `AgentRuntimeBuilder::build()` is the primary place; `for_test()` mirrors it. `vol-llm-runtime` gained a `vol-llm-fs` workspace dependency; `vol-llm-tools-builtin` is untouched.
- **Quality:** 32 unit tests across parser/executor/format/fs_cli; `cargo test -p vol-llm-fs -p vol-llm-runtime` green; llvm-cov line coverage 89.81% (TOTAL, ≥ 80% gate); no doc tests; `cargo check` clean.

## Detailed Summary
`vol-llm-fs` (version inherited from workspace) depends on `tokio` (fs), `serde_json`, `async-trait`, `tracing`, `vol-llm-tool`, `clap 4` with `["string"]`, and the five `vol-llm-tools-builtin-{read,write,edit,glob,grep}` sub-crates via path dependencies; `tempfile` + tokio rt/macros as dev-dependencies.

The parse flow is: `tokenize` (split command string into tokens respecting single/double quotes, skipping consecutive whitespace) → `build_cli` (fully-defined clap `Command` tree with a global `--json`/`-o` flag and per-subcommand flags — `read` has `--offset`/`--limit` defaults 0/2000; `grep` has `--path`/`--glob`/`--output_mode` (files_with_matches|count|content)/`--case_sensitive`; `glob` has `--path` (default `.`)/`--exclude` (comma-separated)/`--kind` (file|directory|all)/`--max_results` (default 100)/`--include_hidden`/`--follow_symlinks`/`--sort` (path_asc|path_desc|modified_desc|modified_asc)/`--with_metadata`) → `parse` produces a typed `ParsedCommand` per subcommand.

The executor maps each variant to the corresponding builtin tool call (e.g. `Edit` → `{"file_path", "old_string", "new_string", "replace_all"}`), maps `ToolError` to `String`, and applies `finalize`. The edit path surfaces the builtin tool's ambiguity error when multiple occurrences exist without `--replace_all` (test-verified). The glob path relies on the builtin GlobTool's relative-to-sandbox-root path contract (absolute paths rejected by design — tests use a relative path).

## Entities Mentioned
- [[vol-llm-fs-crate]]: the new crate itself — CLI layout, `FsCliTool`, `register_cli`.
- [[vol-llm-runtime-crate]]: `AgentRuntimeBuilder::build()` registers the `fs` tool next to the `task` CLI; `for_test()` mirrors it.
- [[vol-llm-task-crate]]: the `task` CLI whose layout the `fs` tool is modeled on (sibling instance of the CLI-style tool pattern).
- [[vol-llm-tool-crate]]: provides `ExecutableTool`, `ToolContext`, `ToolResult`, `ToolRegistry`, `ToolSensitivity`; the five builtin file-op tools live in `vol-llm-tools-builtin-*` sub-crates.

## Concepts Covered
- [[cli-style-tool-pattern]]: the CLI-command-string single-entry-point tool pattern; `fs` is its second implementation after the `task` CLI.
- [[tool-registry]]: registration of the `fs` tool from the runtime builder; coexists with the five original tools.

## Notes
- `vol-llm-fs` must not depend on `vol-llm-task`; the tokenizer is copied, not imported.
- Coverage data shows two known deferred untested arms (`scheme_for` unknown-subcommand, `fmt_scheme` optional-marker branch) — covered at crate level by the 89.81% gate.
- `just cover-gate` recipe fails on just 1.58.0 with a `$` escaping error (`$$` no longer unescapes); the gate was evaluated with the underlying `cargo llvm-cov --summary-only` command (89.81% ≥ 80%).
