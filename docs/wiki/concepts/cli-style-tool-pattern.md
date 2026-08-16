---
type: concept
category: pattern
tags: [tools, cli, patterns]
created: 2026-08-16
updated: 2026-08-16
source_count: 1
---

# CLI-Style Tool Pattern

**Category:** Tool design pattern
**Related:** [[tool-registry]], [[tool-trait]], [[vol-llm-fs-crate]], [[vol-llm-task-crate]], [[fs-cli-tool]]

## Definition

Expose a family of related capabilities through a single `ExecutableTool` that takes one `command` string in CLI style (`tool <subcommand> --flag value`), parsed internally into a typed command enum and executed by delegating to the underlying implementations.

## Key Points
- One tool name for many operations — the LLM learns a single entry point instead of five.
- A single `command` parameter keeps the tool schema flat (`{"type": "object", "properties": {"command": ...}, "required": ["command"]}`).
- Parser (quote-aware tokenizer + clap definition) produces a typed `ParsedCommand` enum — one variant per subcommand.
- Executor dispatches each variant to the underlying tool implementations; no duplicated logic.
- A `scheme [<subcommand>]` introspection subcommand lists available subcommands and their flags (flag, required marker, description) so the LLM can discover parameters without a separate tool.
- Optional global flags (e.g. `--json`/`-o`) transform output uniformly (e.g. wrap in a JSON envelope).

## How It Works

The pattern has three layers, mirroring the `cli/` module layout used by both implementations:

1. **Parse** — `tokenize` splits the command string into tokens respecting quotes; `build_cli` defines the clap `Command` tree with per-subcommand flags whose names match the underlying tools' JSON parameter names; `parse` yields a typed enum (`ParsedCommand`).
2. **Execute** — a match over the enum builds the underlying tool's JSON params and calls its `execute()` (delegation), then applies output formatting (JSON envelope when requested).
3. **Introspect** — `scheme` renders parameter tables from static flag metadata.

## Examples

| Implementation | Subcommands | Underlying capabilities |
|---|---|---|
| `task` CLI in `vol-llm-task` (registered as `task` tool) | task lifecycle ops | task store |
| `fs` tool in `vol-llm-fs` (registered as `fs` tool) | `read`, `write`, `edit`, `grep`, `glob`, `scheme` | the five `vol-llm-tools-builtin-*` file-op tools |

The `fs` tool is the second implementation, modeled on the `task` CLI; the tokenizer is copied from `vol-llm-task` (the crates must not depend on each other).

## Related Concepts
- [[tool-registry]]: CLI-style tools register as normal `ExecutableTool`s and coexist with the tools they wrap
- [[tool-trait]]: `ExecutableTool` contract (`name`, `description`, `parameters`, `execute`, `sensitivity`)
- [[vol-llm-fs-crate]]: the `fs` implementation of this pattern
- [[vol-llm-task-crate]]: the `task` implementation of this pattern
- [[fs-cli-tool]]: source page documenting the `fs` implementation
