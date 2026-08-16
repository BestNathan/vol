# FS CLI Tool Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create the `vol-llm-fs` crate providing an `fs` tool — a CLI-style single entry point over the five builtin file op tools, modeled on the `task` CLI in `vol-llm-task`.

**Architecture:** `vol-llm-fs` mirrors `vol-llm-task`'s CLI layout (`cli/` with parser/commands/executor/format + `tools/` with the `ExecutableTool`). The parser maps `--flag`-style subcommands 1:1 onto the existing tools' JSON parameters; the executor delegates to `ReadTool`/`WriteTool`/`EditTool`/`GrepTool`/`GlobTool::execute()` — zero file op capability change. Registered from `AgentRuntimeBuilder::build()` next to the task tool; coexists with the five original tools.

**Tech Stack:** Rust workspace; clap 4 (features `["string"]`); `vol-llm-tool` (`ExecutableTool`, `ToolContext`, `ToolResult`); the five `vol-llm-tools-builtin-*` sub-crates.

**Spec:** `docs/superpowers/specs/2026-08-16-fs-cli-tool-design.md` — the plan argues from the spec, so the spec travels with it; executors read both.

## Global Constraints

- Flag names are identical to the tools' JSON parameter names (no renaming).
- All flags style (no positional operands) — mirrors task cli.
- `vol-llm-tools-builtin` is untouched; `fs` coexists with the 5 original tools.
- No `sensitivity()` override — all `Safe`.
- Every new `pub fn` has at least one test; no doc tests (doc examples use ` ```text`).
- Coverage gate: `just cover-gate vol-llm-fs 80` must pass.
- Registration: `AgentRuntimeBuilder::build()` is the primary place; `for_test()` mirrors it.
- Tokenizer (~40 lines) is copied from `vol-llm-task/src/cli/parser.rs`; `vol-llm-fs` must NOT depend on `vol-llm-task`.

---

## File Structure

```
crates/vol-llm-fs/
├── Cargo.toml           # deps: tokio, serde_json, async-trait, tracing, clap,
│                        #       vol-llm-tool (workspace), 5 file-op sub-crates (path)
└── src/
    ├── lib.rs           # pub mod cli; pub mod tools;
    ├── cli/
    │   ├── mod.rs       # pub(crate) mod {commands, executor, format, parser};
    │   ├── commands.rs  # ParsedCommand enum
    │   ├── parser.rs    # tokenize + build_cli + parse
    │   ├── executor.rs  # dispatch → tool execute delegation + scheme_for
    │   └── format.rs    # fmt_scheme + JSON envelope
    └── tools/
        ├── mod.rs       # register_cli(registry)
        └── fs_cli.rs    # FsCliTool: ExecutableTool
```

---

### Task 1: Workspace member + `vol-llm-fs` scaffold

**Files:**
- Modify: `Cargo.toml` (workspace members)
- Create: `crates/vol-llm-fs/Cargo.toml`
- Create: `crates/vol-llm-fs/src/lib.rs`
- Create: `crates/vol-llm-fs/src/cli/mod.rs`
- Create: `crates/vol-llm-fs/src/tools/mod.rs`

**Interfaces:**
- Consumes: nothing (scaffold only)
- Produces: empty-but-compiling `vol-llm-fs` crate with `cli` and `tools` modules

- [ ] **Step 1: Add workspace member**

In `Cargo.toml`, after the `"crates/vol-llm-task",` member line, add:

```toml
    "crates/vol-llm-fs",
```

- [ ] **Step 2: Write crate manifest**

Create `crates/vol-llm-fs/Cargo.toml`:

```toml
[package]
name = "vol-llm-fs"
version.workspace = true
edition.workspace = true


[lints]
workspace = true

[dependencies]
tokio = { workspace = true, features = ["fs"] }
serde_json = { workspace = true }
async-trait = { workspace = true }
tracing = { workspace = true }
vol-llm-tool = { workspace = true }
clap = { version = "4", features = ["string"] }
vol-llm-tools-builtin-read = { path = "../vol-llm-tools-builtin/read-tool" }
vol-llm-tools-builtin-write = { path = "../vol-llm-tools-builtin/write-tool" }
vol-llm-tools-builtin-edit = { path = "../vol-llm-tools-builtin/edit-tool" }
vol-llm-tools-builtin-glob = { path = "../vol-llm-tools-builtin/glob-tool" }
vol-llm-tools-builtin-grep = { path = "../vol-llm-tools-builtin/grep-tool" }

[dev-dependencies]
tempfile = "3"
tokio = { workspace = true, features = ["rt", "macros"] }
```

- [ ] **Step 3: Write lib.rs and empty module files**

Create `crates/vol-llm-fs/src/lib.rs`:

```rust
//! vol-llm-fs: unified CLI-style `fs` tool for file operations.
//!
//! Provides the `fs` tool — a single entry point (CLI-style command string)
//! over the builtin file op tools (read/write/edit/grep/glob), modeled on
//! the `task` CLI in `vol-llm-task`. Delegates to the existing tools'
//! implementations; contains no file op logic of its own.

pub mod cli;
pub mod tools;
```

Create `crates/vol-llm-fs/src/cli/mod.rs`:

```rust
//! CLI-style fs tool — parser, executor, and formatter.
```

Create `crates/vol-llm-fs/src/tools/mod.rs`:

```rust
//! LLM tools for filesystem operations.
```

- [ ] **Step 4: Verify the scaffold compiles**

Run: `cargo check -p vol-llm-fs`
Expected: PASS ("Finished" with no errors)

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/vol-llm-fs/
git commit -m "feat(fs): scaffold vol-llm-fs crate"
```

---

### Task 2: `cli/commands.rs` + `cli/parser.rs` — command model and parser

**Files:**
- Create: `crates/vol-llm-fs/src/cli/commands.rs`
- Create: `crates/vol-llm-fs/src/cli/parser.rs`
- Modify: `crates/vol-llm-fs/src/cli/mod.rs` (declare the two modules)

**Interfaces:**
- Consumes: nothing
- Produces: `pub(crate) enum ParsedCommand { Read{..}, Write{..}, Edit{..}, Grep{..}, Glob{..}, Scheme{..} }` (in `cli::commands`); `pub(crate) fn parse(input: &str) -> Result<ParsedCommand, String>` (in `cli::parser`); `pub(crate) fn tokenize` is private

- [ ] **Step 1: Declare the modules and write the failing parser tests**

Update `crates/vol-llm-fs/src/cli/mod.rs` (this makes the test file compile and fail — `commands.rs` does not exist yet, so the red state is a compile error):

```rust
//! CLI-style fs tool — parser, executor, and formatter.

pub(crate) mod commands;
pub(crate) mod parser;
```

Create `crates/vol-llm-fs/src/cli/parser.rs` containing ONLY the test module (implementation comes in Step 3):

```rust
//! CLI parser — tokenizer + clap definition + parse entry point.
//!
//! The flow is:
//!   1. `tokenize` — split a command string into tokens (respecting quotes).
//!   2. `build_cli` — return a fully-defined `clap::Command` tree.
//!   3. `parse` — combine the two and produce a [`ParsedCommand`].

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::commands::ParsedCommand;

    #[test]
    fn tokenize_respects_single_quotes() {
        let tokens = tokenize("read --file_path 'a b.txt' --limit 5");
        assert_eq!(
            tokens,
            vec!["read", "--file_path", "a b.txt", "--limit", "5"]
        );
    }

    #[test]
    fn tokenize_respects_double_quotes() {
        let tokens = tokenize("write --content \"hello world\" --file_path x");
        assert_eq!(
            tokens,
            vec!["write", "--content", "hello world", "--file_path", "x"]
        );
    }

    #[test]
    fn tokenize_skips_consecutive_spaces() {
        assert_eq!(tokenize("read   a"), vec!["read", "a"]);
    }

    #[test]
    fn parse_read_applies_defaults() {
        let cmd = parse("read --file_path foo.txt").unwrap();
        assert!(matches!(
            cmd,
            ParsedCommand::Read { file_path, offset: 0, limit: 2000, json: false }
                if file_path == "foo.txt"
        ));
    }

    #[test]
    fn parse_read_with_options() {
        let cmd = parse("read --file_path foo.txt --offset 10 --limit 25").unwrap();
        assert!(matches!(cmd, ParsedCommand::Read { offset: 10, limit: 25, .. }));
    }

    #[test]
    fn parse_write_requires_content() {
        let err = parse("write --file_path a.txt").unwrap_err();
        assert!(err.contains("Parse error"), "unexpected: {err}");
    }

    #[test]
    fn parse_unknown_subcommand_errors() {
        let err = parse("frobnicate").unwrap_err();
        assert!(err.contains("Parse error"), "unexpected: {err}");
    }

    #[test]
    fn parse_edit_flags() {
        let cmd = parse("edit --file_path a --old_string x --new_string y --replace_all").unwrap();
        assert!(matches!(cmd, ParsedCommand::Edit { replace_all: true, .. }));
    }

    #[test]
    fn parse_grep_defaults() {
        let cmd = parse("grep --pattern 'TODO'").unwrap();
        assert!(matches!(
            cmd,
            ParsedCommand::Grep { output_mode, path: None, case_sensitive: false, .. }
                if output_mode == "files_with_matches"
        ));
    }

    #[test]
    fn parse_glob_exclude_list() {
        let cmd =
            parse("glob --pattern '**/*.rs' --exclude target,node_modules --max_results 50").unwrap();
        match cmd {
            ParsedCommand::Glob { exclude, max_results, .. } => {
                assert_eq!(
                    exclude,
                    Some(vec!["target".to_string(), "node_modules".to_string()])
                );
                assert_eq!(max_results, 50);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn parse_json_global_flag() {
        let cmd = parse("read --file_path a --json").unwrap();
        assert!(matches!(cmd, ParsedCommand::Read { json: true, .. }));
    }

    #[test]
    fn parse_accepts_fs_prefix() {
        let cmd = parse("fs read --file_path a").unwrap();
        assert!(matches!(cmd, ParsedCommand::Read { .. }));
    }

    #[test]
    fn parse_scheme() {
        let cmd = parse("scheme read").unwrap();
        assert!(matches!(cmd, ParsedCommand::Scheme { subcommand: Some(s) } if s == "read"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vol-llm-fs cli::parser`
Expected: FAIL — compile error: `file not found for module \`commands\`` (and unresolved `tokenize`/`parse` once that is created)

- [ ] **Step 3: Write `commands.rs` and the parser implementation**

Create `crates/vol-llm-fs/src/cli/commands.rs`:

```rust
//! Parsed CLI command variants for the fs tool.
//!
//! Each variant maps to a specific subcommand of the `fs` CLI. The parser
//! (see [`super::parser`]) yields one of these values for every successfully
//! parsed invocation.

/// Parsed CLI command — one variant per subcommand.
#[derive(Debug, Clone)]
pub(crate) enum ParsedCommand {
    /// `fs read --file_path <PATH> [--offset N] [--limit N]`
    Read {
        file_path: String,
        offset: usize,
        limit: usize,
        json: bool,
    },
    /// `fs write --file_path <PATH> --content <TEXT>`
    Write {
        file_path: String,
        content: String,
        json: bool,
    },
    /// `fs edit --file_path <PATH> --old_string <S> --new_string <S> [--replace_all]`
    Edit {
        file_path: String,
        old_string: String,
        new_string: String,
        replace_all: bool,
        json: bool,
    },
    /// `fs grep --pattern <REGEX> [--path DIR] [--glob PAT] [--output_mode M] [--case_sensitive]`
    Grep {
        pattern: String,
        path: Option<String>,
        glob: Option<String>,
        output_mode: String,
        case_sensitive: bool,
        json: bool,
    },
    /// `fs glob --pattern <PAT> [--path DIR] [--exclude A,B] ...`
    Glob {
        pattern: String,
        path: String,
        exclude: Option<Vec<String>>,
        kind: String,
        max_results: usize,
        include_hidden: bool,
        follow_symlinks: bool,
        sort: String,
        with_metadata: bool,
        json: bool,
    },
    /// `fs scheme [<subcommand>]`
    Scheme { subcommand: Option<String> },
}
```

Replace the placeholder content of `crates/vol-llm-fs/src/cli/parser.rs` (keep the test module) with the full implementation:

```rust
//! CLI parser — tokenizer + clap definition + parse entry point.
//!
//! The flow is:
//!   1. `tokenize` — split a command string into tokens (respecting quotes).
//!   2. `build_cli` — return a fully-defined `clap::Command` tree.
//!   3. `parse` — combine the two and produce a [`ParsedCommand`].

use clap::{value_parser, Arg, ArgAction, Command};

use super::commands::ParsedCommand;

// ---------------------------------------------------------------------------
// Tokenizer
// ---------------------------------------------------------------------------

/// Split a command string into tokens, respecting single and double quotes.
fn tokenize(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quote: Option<char> = None;

    for ch in input.chars() {
        match (in_quote, ch) {
            (None, '"' | '\'') => in_quote = Some(ch),
            (Some(q), c) if c == q => in_quote = None,
            (None, ' ') if !current.is_empty() => {
                tokens.push(std::mem::take(&mut current));
            }
            (None, ' ') => {} // skip consecutive whitespace
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

// ---------------------------------------------------------------------------
// CLI definition
// ---------------------------------------------------------------------------

/// Build the full clap CLI definition for all fs subcommands.
pub(crate) fn build_cli() -> Command {
    Command::new("fs")
        // Global `--json` / `-o` flag — available on every subcommand.
        .arg(
            Arg::new("json")
                .long("json")
                .short('o')
                .global(true)
                .num_args(0)
                .action(ArgAction::SetTrue)
                .help("Output in JSON format"),
        )
        .subcommand(
            Command::new("read")
                .about("Read file contents with line numbers")
                .arg(
                    Arg::new("file_path")
                        .long("file_path")
                        .required(true)
                        .help("Path to the file to read"),
                )
                .arg(
                    Arg::new("offset")
                        .long("offset")
                        .value_parser(value_parser!(usize))
                        .default_value("0")
                        .help("Line offset to start reading from (0-indexed)"),
                )
                .arg(
                    Arg::new("limit")
                        .long("limit")
                        .value_parser(value_parser!(usize))
                        .default_value("2000")
                        .help("Maximum number of lines to read"),
                ),
        )
        .subcommand(
            Command::new("write")
                .about("Create or overwrite a file")
                .arg(
                    Arg::new("file_path")
                        .long("file_path")
                        .required(true)
                        .help("Path to the file to write"),
                )
                .arg(
                    Arg::new("content")
                        .long("content")
                        .required(true)
                        .help("Content to write to the file"),
                ),
        )
        .subcommand(
            Command::new("edit")
                .about("Replace exact strings in a file")
                .arg(
                    Arg::new("file_path")
                        .long("file_path")
                        .required(true)
                        .help("Path to the file to edit"),
                )
                .arg(
                    Arg::new("old_string")
                        .long("old_string")
                        .required(true)
                        .help("Exact string to find and replace"),
                )
                .arg(
                    Arg::new("new_string")
                        .long("new_string")
                        .required(true)
                        .help("String to replace with"),
                )
                .arg(
                    Arg::new("replace_all")
                        .long("replace_all")
                        .action(ArgAction::SetTrue)
                        .help("Replace all occurrences (default: error if multiple found)"),
                ),
        )
        .subcommand(
            Command::new("grep")
                .about("Search file content using regex")
                .arg(
                    Arg::new("pattern")
                        .long("pattern")
                        .required(true)
                        .help("Regex pattern to search for"),
                )
                .arg(
                    Arg::new("path")
                        .long("path")
                        .help("Root directory to search in"),
                )
                .arg(
                    Arg::new("glob")
                        .long("glob")
                        .help("File pattern filter (e.g. \"*.rs\")"),
                )
                .arg(
                    Arg::new("output_mode")
                        .long("output_mode")
                        .value_parser(["files_with_matches", "count", "content"])
                        .default_value("files_with_matches")
                        .help("Output format"),
                )
                .arg(
                    Arg::new("case_sensitive")
                        .long("case_sensitive")
                        .action(ArgAction::SetTrue)
                        .help("Case-sensitive matching"),
                ),
        )
        .subcommand(
            Command::new("glob")
                .about("Find files and directories by glob patterns")
                .arg(
                    Arg::new("pattern")
                        .long("pattern")
                        .required(true)
                        .help("Glob pattern relative to the search path"),
                )
                .arg(
                    Arg::new("path")
                        .long("path")
                        .default_value(".")
                        .help("Search root directory (default: workspace root)"),
                )
                .arg(
                    Arg::new("exclude")
                        .long("exclude")
                        .value_delimiter(',')
                        .help("Glob patterns to exclude (comma-separated)"),
                )
                .arg(
                    Arg::new("kind")
                        .long("kind")
                        .value_parser(["file", "directory", "all"])
                        .default_value("file")
                        .help("Return file, directory, or all entries"),
                )
                .arg(
                    Arg::new("max_results")
                        .long("max_results")
                        .value_parser(value_parser!(usize))
                        .default_value("100")
                        .help("Maximum results (1-1000)"),
                )
                .arg(
                    Arg::new("include_hidden")
                        .long("include_hidden")
                        .action(ArgAction::SetTrue)
                        .help("Include hidden files and directories"),
                )
                .arg(
                    Arg::new("follow_symlinks")
                        .long("follow_symlinks")
                        .action(ArgAction::SetTrue)
                        .help("Follow symbolic links"),
                )
                .arg(
                    Arg::new("sort")
                        .long("sort")
                        .value_parser(["path_asc", "path_desc", "modified_desc", "modified_asc"])
                        .default_value("path_asc")
                        .help("Sort order"),
                )
                .arg(
                    Arg::new("with_metadata")
                        .long("with_metadata")
                        .action(ArgAction::SetTrue)
                        .help("Include file size and modification time"),
                ),
        )
        .subcommand(
            Command::new("scheme")
                .about("Show parameter definitions for a subcommand")
                .arg(
                    Arg::new("subcommand")
                        .required(false)
                        .help("Subcommand name"),
                ),
        )
}

// ---------------------------------------------------------------------------
// Parse entry point
// ---------------------------------------------------------------------------

/// Parse a command string into a `ParsedCommand`.
pub(crate) fn parse(input: &str) -> Result<ParsedCommand, String> {
    let tokens = tokenize(input);
    let cli = build_cli();

    // Ensure the "fs" prefix is present so clap sees a proper argv.
    let full_tokens = if tokens.first().map(std::string::String::as_str) == Some("fs") {
        tokens
    } else {
        let mut with_prefix = vec!["fs".to_string()];
        with_prefix.extend(tokens);
        with_prefix
    };

    let matches = cli.try_get_matches_from(&full_tokens).map_err(|e| {
        format!(
            "Parse error: {e}\nUsage: fs <subcommand> [--flags]. \
             Use 'fs scheme <sub>' to see parameters."
        )
    })?;

    let json = matches.get_flag("json");

    match matches.subcommand() {
        Some(("read", m)) => Ok(ParsedCommand::Read {
            file_path: m.get_one::<String>("file_path").cloned().unwrap_or_default(),
            offset: m.get_one::<usize>("offset").copied().unwrap_or_default(),
            limit: m.get_one::<usize>("limit").copied().unwrap_or_default(),
            json,
        }),
        Some(("write", m)) => Ok(ParsedCommand::Write {
            file_path: m.get_one::<String>("file_path").cloned().unwrap_or_default(),
            content: m.get_one::<String>("content").cloned().unwrap_or_default(),
            json,
        }),
        Some(("edit", m)) => Ok(ParsedCommand::Edit {
            file_path: m.get_one::<String>("file_path").cloned().unwrap_or_default(),
            old_string: m.get_one::<String>("old_string").cloned().unwrap_or_default(),
            new_string: m.get_one::<String>("new_string").cloned().unwrap_or_default(),
            replace_all: m.get_flag("replace_all"),
            json,
        }),
        Some(("grep", m)) => Ok(ParsedCommand::Grep {
            pattern: m.get_one::<String>("pattern").cloned().unwrap_or_default(),
            path: m.get_one::<String>("path").cloned(),
            glob: m.get_one::<String>("glob").cloned(),
            output_mode: m.get_one::<String>("output_mode").cloned().unwrap_or_default(),
            case_sensitive: m.get_flag("case_sensitive"),
            json,
        }),
        Some(("glob", m)) => Ok(ParsedCommand::Glob {
            pattern: m.get_one::<String>("pattern").cloned().unwrap_or_default(),
            path: m.get_one::<String>("path").cloned().unwrap_or_default(),
            exclude: m.get_many::<String>("exclude").map(|v| v.cloned().collect()),
            kind: m.get_one::<String>("kind").cloned().unwrap_or_default(),
            max_results: m.get_one::<usize>("max_results").copied().unwrap_or_default(),
            include_hidden: m.get_flag("include_hidden"),
            follow_symlinks: m.get_flag("follow_symlinks"),
            sort: m.get_one::<String>("sort").cloned().unwrap_or_default(),
            with_metadata: m.get_flag("with_metadata"),
            json,
        }),
        Some(("scheme", m)) => Ok(ParsedCommand::Scheme {
            subcommand: m.get_one::<String>("subcommand").cloned(),
        }),
        _ => Err(
            "Parse error: no subcommand given.\nUsage: fs <subcommand> [--flags]. \
             Use 'fs scheme <sub>' to see parameters."
                .to_string(),
        ),
    }
}
```

Update `crates/vol-llm-fs/src/cli/mod.rs` (no change needed — modules were declared in Step 1):

```rust
//! CLI-style fs tool — parser, executor, and formatter.

pub(crate) mod commands;
pub(crate) mod parser;
```

(Note: `executor` and `format` are added in Task 3.)

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vol-llm-fs cli::parser`
Expected: PASS — all 13 tests

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-fs/
git commit -m "feat(fs): cli parser with flag-based subcommand mapping"
```

---

### Task 3: `cli/format.rs` + `cli/executor.rs` — delegation to the five tools

**Files:**
- Create: `crates/vol-llm-fs/src/cli/format.rs`
- Create: `crates/vol-llm-fs/src/cli/executor.rs`
- Modify: `crates/vol-llm-fs/src/cli/mod.rs` (declare the two modules)

**Interfaces:**
- Consumes: `cli::parser::parse`, `cli::commands::ParsedCommand` (Task 2); `ReadTool`/`WriteTool`/`EditTool`/`GrepTool`/`GlobTool` from the five sub-crates
- Produces: `pub(crate) async fn execute(cmd: ParsedCommand, context: &ToolContext) -> Result<ToolResult, String>` (in `cli::executor`); `pub(crate) fn fmt_scheme(subcommand: &str, params: &[(&str, bool, &str)]) -> String` and `pub(crate) fn envelope(success: bool, content: &str) -> String` (in `cli::format`)

- [ ] **Step 1: Declare the modules and write the failing executor tests**

Update `crates/vol-llm-fs/src/cli/mod.rs` (this makes the test file compile and fail — `format.rs` does not exist yet, so the red state is a compile error):

```rust
//! CLI-style fs tool — parser, executor, and formatter.

pub(crate) mod commands;
pub(crate) mod executor;
pub(crate) mod format;
pub(crate) mod parser;
```

Create `crates/vol-llm-fs/src/cli/executor.rs` containing ONLY the test module (implementation comes in Step 3):

```rust
//! FS CLI executor — execute ParsedCommand by delegating to builtin file op tools.

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use vol_llm_tool::{ToolContext, ToolResult};

    fn ctx() -> ToolContext {
        ToolContext::for_test()
    }

    async fn run(command: &str) -> Result<ToolResult, String> {
        let cmd = crate::cli::parser::parse(command).unwrap();
        execute(cmd, &ctx()).await
    }

    #[tokio::test]
    async fn write_then_read_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notes.txt");
        let p = path.display().to_string();

        let w = run(&format!("write --file_path {p} --content 'hello world'"))
            .await
            .unwrap();
        assert!(w.success);

        let r = run(&format!("read --file_path {p}")).await.unwrap();
        assert!(r.success);
        assert!(r.content.contains("hello world"));
    }

    #[tokio::test]
    async fn edit_single_occurrence() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        fs::write(&path, "one two one").unwrap();
        let p = path.display().to_string();

        let r = run(&format!(
            "edit --file_path {p} --old_string two --new_string THREE"
        ))
        .await
        .unwrap();
        assert!(r.success);
        assert_eq!(fs::read_to_string(&path).unwrap(), "one THREE one");
    }

    #[tokio::test]
    async fn edit_replace_all() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        fs::write(&path, "dup dup dup").unwrap();
        let p = path.display().to_string();

        let r = run(&format!(
            "edit --file_path {p} --old_string dup --new_string x --replace_all"
        ))
        .await
        .unwrap();
        assert!(r.success);
        assert_eq!(fs::read_to_string(&path).unwrap(), "x x x");
    }

    #[tokio::test]
    async fn edit_ambiguous_without_replace_all_errors() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        fs::write(&path, "dup dup dup").unwrap();
        let p = path.display().to_string();

        let err = run(&format!(
            "edit --file_path {p} --old_string dup --new_string x"
        ))
        .await
        .unwrap_err();
        assert!(err.contains("replace_all"), "unexpected: {err}");
    }

    #[tokio::test]
    async fn grep_content_mode_finds_matches() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("code.rs"), "fn main() {}\n// TODO: fix\n").unwrap();
        let d = dir.path().display().to_string();

        let r = run(&format!("grep --pattern 'TODO' --path {d} --output_mode content"))
            .await
            .unwrap();
        assert!(r.success);
        assert!(r.content.contains("TODO"), "unexpected: {}", r.content);
    }

    #[tokio::test]
    async fn glob_finds_matching_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("x.rs"), "").unwrap();
        fs::write(dir.path().join("y.txt"), "").unwrap();
        let d = dir.path().display().to_string();

        let r = run(&format!("glob --pattern '*.rs' --path {d}"))
            .await
            .unwrap();
        assert!(r.success);
        assert!(r.content.contains("x.rs"), "unexpected: {}", r.content);
        assert!(!r.content.contains("y.txt"), "unexpected: {}", r.content);
    }

    #[tokio::test]
    async fn scheme_lists_params() {
        let r = run("scheme read").await.unwrap();
        assert!(r.success);
        assert!(r.content.contains("--file_path"));
        assert!(r.content.contains("required"));
    }

    #[tokio::test]
    async fn scheme_without_arg_lists_subcommands() {
        let r = run("scheme").await.unwrap();
        assert!(r.success);
        assert!(r.content.contains("read"));
        assert!(r.content.contains("scheme"));
    }

    #[tokio::test]
    async fn json_envelope_wraps_result() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("j.txt");
        fs::write(&path, "json test").unwrap();
        let p = path.display().to_string();

        let r = run(&format!("read --file_path {p} --json")).await.unwrap();
        assert!(r.content.starts_with('{'), "unexpected: {}", r.content);
        assert!(r.content.contains("\"success\""), "unexpected: {}", r.content);
        assert!(r.content.contains("json test"), "unexpected: {}", r.content);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vol-llm-fs cli::executor`
Expected: FAIL — compile error: `file not found for module \`format\`` (and unresolved `execute` once that is created)

- [ ] **Step 3: Write `format.rs` and the executor implementation**

Create `crates/vol-llm-fs/src/cli/format.rs`:

```rust
//! Output formatting for the fs CLI — text passthrough and JSON envelope.

/// Format a scheme (parameter list) for a specific subcommand.
pub(crate) fn fmt_scheme(subcommand: &str, params: &[(&str, bool, &str)]) -> String {
    let mut out = format!("{subcommand} parameters:\n");
    for (name, required, desc) in params {
        let req = if *required {
            "(required)"
        } else {
            "(optional)"
        };
        out.push_str(&format!("  --{name:<14} {req:<10} {desc}\n"));
    }
    out.trim_end().to_string()
}

/// Wrap a tool result in a JSON envelope.
pub(crate) fn envelope(success: bool, content: &str) -> String {
    serde_json::json!({ "success": success, "content": content }).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fmt_scheme_lists_flags_and_required_marker() {
        let out = fmt_scheme("read", &[("file_path", true, "Path to the file to read")]);
        assert!(out.contains("--file_path"));
        assert!(out.contains("(required)"));
    }

    #[test]
    fn envelope_serializes_success_and_content() {
        let out = envelope(true, "hi");
        // serde_json preserve_order → insertion order (success, content)
        assert_eq!(out, "{\"success\":true,\"content\":\"hi\"}");
    }

    #[test]
    fn envelope_serializes_failure() {
        let out = envelope(false, "boom");
        assert!(out.contains("\"success\":false"));
    }
}
```

Replace the placeholder content of `crates/vol-llm-fs/src/cli/executor.rs` (keep the test module) with the full implementation:

```rust
//! FS CLI executor — execute ParsedCommand by delegating to builtin file op tools.

use vol_llm_tool::{ToolContext, ToolResult};
use vol_llm_tools_builtin_edit::EditTool;
use vol_llm_tools_builtin_glob::GlobTool;
use vol_llm_tools_builtin_grep::GrepTool;
use vol_llm_tools_builtin_read::ReadTool;
use vol_llm_tools_builtin_write::WriteTool;

use super::commands::ParsedCommand;

/// Execute a parsed command by delegating to the underlying file op tool.
pub(crate) async fn execute(
    cmd: ParsedCommand,
    context: &ToolContext,
) -> Result<ToolResult, String> {
    match cmd {
        ParsedCommand::Read {
            file_path,
            offset,
            limit,
            json,
        } => {
            let params = serde_json::json!({
                "file_path": file_path,
                "offset": offset,
                "limit": limit,
            });
            let result = ReadTool::new()
                .execute(&params, context)
                .await
                .map_err(|e| e.to_string())?;
            Ok(finalize(result, json))
        }
        ParsedCommand::Write {
            file_path,
            content,
            json,
        } => {
            let params = serde_json::json!({
                "file_path": file_path,
                "content": content,
            });
            let result = WriteTool::new()
                .execute(&params, context)
                .await
                .map_err(|e| e.to_string())?;
            Ok(finalize(result, json))
        }
        ParsedCommand::Edit {
            file_path,
            old_string,
            new_string,
            replace_all,
            json,
        } => {
            let params = serde_json::json!({
                "file_path": file_path,
                "old_string": old_string,
                "new_string": new_string,
                "replace_all": replace_all,
            });
            let result = EditTool::new()
                .execute(&params, context)
                .await
                .map_err(|e| e.to_string())?;
            Ok(finalize(result, json))
        }
        ParsedCommand::Grep {
            pattern,
            path,
            glob,
            output_mode,
            case_sensitive,
            json,
        } => {
            let params = serde_json::json!({
                "pattern": pattern,
                "path": path,
                "glob": glob,
                "output_mode": output_mode,
                "case_sensitive": case_sensitive,
            });
            let result = GrepTool::new()
                .execute(&params, context)
                .await
                .map_err(|e| e.to_string())?;
            Ok(finalize(result, json))
        }
        ParsedCommand::Glob {
            pattern,
            path,
            exclude,
            kind,
            max_results,
            include_hidden,
            follow_symlinks,
            sort,
            with_metadata,
            json,
        } => {
            let params = serde_json::json!({
                "pattern": pattern,
                "path": path,
                "exclude": exclude,
                "kind": kind,
                "max_results": max_results,
                "include_hidden": include_hidden,
                "follow_symlinks": follow_symlinks,
                "sort": sort,
                "with_metadata": with_metadata,
            });
            let result = GlobTool::new()
                .execute(&params, context)
                .await
                .map_err(|e| e.to_string())?;
            Ok(finalize(result, json))
        }
        ParsedCommand::Scheme { subcommand } => Ok(ToolResult {
            success: true,
            content: scheme_for(subcommand.as_deref()),
            error: None,
            data: None,
            call_id: String::new(),
        }),
    }
}

/// Wrap the content in a JSON envelope when `--json` is requested.
fn finalize(result: ToolResult, json: bool) -> ToolResult {
    if json {
        let content = super::format::envelope(result.success, &result.content);
        ToolResult { content, ..result }
    } else {
        result
    }
}

/// Scheme tables — (flag, required, description) per subcommand.
fn scheme_for(subcommand: Option<&str>) -> String {
    match subcommand {
        Some("read") => super::format::fmt_scheme(
            "read",
            &[
                ("file_path", true, "Path to the file to read"),
                ("offset", false, "Line offset to start reading from (default: 0)"),
                ("limit", false, "Maximum number of lines to read (default: 2000)"),
            ],
        ),
        Some("write") => super::format::fmt_scheme(
            "write",
            &[
                ("file_path", true, "Path to the file to write"),
                ("content", true, "Content to write to the file"),
            ],
        ),
        Some("edit") => super::format::fmt_scheme(
            "edit",
            &[
                ("file_path", true, "Path to the file to edit"),
                ("old_string", true, "Exact string to find and replace"),
                ("new_string", true, "String to replace with"),
                ("replace_all", false, "Replace all occurrences (default: false)"),
            ],
        ),
        Some("grep") => super::format::fmt_scheme(
            "grep",
            &[
                ("pattern", true, "Regex pattern to search for"),
                ("path", false, "Root directory to search in"),
                ("glob", false, "File pattern filter (e.g. \"*.rs\")"),
                (
                    "output_mode",
                    false,
                    "Output format: files_with_matches|count|content (default: files_with_matches)",
                ),
                ("case_sensitive", false, "Case-sensitive matching (default: false)"),
            ],
        ),
        Some("glob") => super::format::fmt_scheme(
            "glob",
            &[
                ("pattern", true, "Glob pattern relative to the search path"),
                ("path", false, "Search root directory (default: \".\")"),
                ("exclude", false, "Glob patterns to exclude (comma-separated)"),
                ("kind", false, "Return file|directory|all (default: file)"),
                ("max_results", false, "Maximum results (default: 100)"),
                ("include_hidden", false, "Include hidden files (default: false)"),
                ("follow_symlinks", false, "Follow symbolic links (default: false)"),
                ("sort", false, "Sort order: path_asc|path_desc|modified_desc|modified_asc"),
                ("with_metadata", false, "Include file size and modification time"),
            ],
        ),
        Some(other) => format!("Unknown subcommand: {other}\nUse 'fs scheme' to list subcommands."),
        None => "fs subcommands:\n  \
                 read    Read file contents with line numbers\n  \
                 write   Create or overwrite a file\n  \
                 edit    Replace exact strings in a file\n  \
                 grep    Search file content using regex\n  \
                 glob    Find files and directories by glob patterns\n  \
                 scheme  Show parameter definitions [<subcommand>]"
            .to_string(),
    }
}
```

Update `crates/vol-llm-fs/src/cli/mod.rs` (no change needed — modules were declared in Step 1):

```rust
//! CLI-style fs tool — parser, executor, and formatter.

pub(crate) mod commands;
pub(crate) mod executor;
pub(crate) mod format;
pub(crate) mod parser;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vol-llm-fs`
Expected: PASS — parser tests (13) + executor tests (9) + format tests (3)

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-fs/
git commit -m "feat(fs): executor delegating to builtin file op tools"
```

---

### Task 4: `tools/fs_cli.rs` + `tools/mod.rs` — the `FsCliTool` ExecutableTool

**Files:**
- Create: `crates/vol-llm-fs/src/tools/fs_cli.rs`
- Modify: `crates/vol-llm-fs/src/tools/mod.rs`

**Interfaces:**
- Consumes: `cli::parser::parse`, `cli::executor::execute` (Tasks 2–3)
- Produces: `pub struct FsCliTool` with `new()`/`Default`, implementing `ExecutableTool` (name `"fs"`); `pub fn register_cli(registry: &mut vol_llm_tool::ToolRegistry)` (in `tools`)

- [ ] **Step 1: Declare the tool module and write the failing FsCliTool tests**

Update `crates/vol-llm-fs/src/tools/mod.rs` (this makes the test file compile and fail — `FsCliTool` is not yet defined, so the red state is a compile error):

```rust
//! LLM tools for filesystem operations.

mod fs_cli;

pub use fs_cli::FsCliTool;
```

Create `crates/vol-llm-fs/src/tools/fs_cli.rs` containing ONLY the test module (implementation comes in Step 3):

```rust
//! The `fs` tool — unified CLI-style entry point for file operations.

#[cfg(test)]
mod tests {
    use super::*;

    fn tool() -> FsCliTool {
        FsCliTool::new()
    }

    #[tokio::test]
    async fn test_name_and_description() {
        let t = tool();
        assert_eq!(t.name(), "fs");
        assert!(t.description().contains("read"));
        assert!(t.description().contains("scheme"));
    }

    #[tokio::test]
    async fn test_parameters_require_command() {
        let t = tool();
        let params = t.parameters();
        let required = params.get("required").unwrap().as_array().unwrap();
        assert!(required.iter().any(|v| v.as_str() == Some("command")));
    }

    #[tokio::test]
    async fn test_execute_missing_command() {
        let t = tool();
        let result = t
            .execute(&serde_json::json!({}), &ToolContext::default())
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_invalid_command() {
        let t = tool();
        let result = t
            .execute(
                &serde_json::json!({"command": "invalid_subcommand"}),
                &ToolContext::default(),
            )
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_write_read_flow() {
        let t = tool();
        let ctx = ToolContext::for_test();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("e2e.txt");
        let p = path.display().to_string();

        let r = t
            .execute(
                &serde_json::json!({"command": format!("write --file_path {p} --content 'e2e'")}),
                &ctx,
            )
            .await
            .unwrap();
        assert!(r.success);

        let r = t
            .execute(
                &serde_json::json!({"command": format!("read --file_path {p}")}),
                &ctx,
            )
            .await
            .unwrap();
        assert!(r.content.contains("e2e"));
    }

    #[tokio::test]
    async fn test_json_output() {
        let t = tool();
        let ctx = ToolContext::for_test();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("json.txt");
        std::fs::write(&path, "j").unwrap();
        let p = path.display().to_string();

        let result = t
            .execute(
                &serde_json::json!({"command": format!("read --file_path {p} --json")}),
                &ctx,
            )
            .await
            .unwrap();
        assert!(result.content.starts_with('{'));
    }

    #[tokio::test]
    async fn test_sensitivity_safe_for_all_subcommands() {
        let t = tool();
        for cmd in [
            "read --file_path a",
            "write --file_path a --content b",
            "edit --file_path a --old_string x --new_string y",
            "grep --pattern x",
            "glob --pattern '**/*.rs'",
        ] {
            let s = t.sensitivity(&serde_json::json!({"command": cmd}));
            assert!(matches!(s, vol_llm_tool::ToolSensitivity::Safe));
        }
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vol-llm-fs tools::fs_cli`
Expected: FAIL — compile error: unresolved `FsCliTool` (and missing `ToolContext` import once that is created)

- [ ] **Step 3: Write the FsCliTool implementation**

Replace the placeholder content of `crates/vol-llm-fs/src/tools/fs_cli.rs` (keep the test module) with the full implementation:

```rust
//! The `fs` tool — unified CLI-style entry point for file operations.

use async_trait::async_trait;
use vol_llm_tool::{ExecutableTool, ToolContext, ToolError, ToolResult, ToolResultType};

use crate::cli::{commands::ParsedCommand, parser};

pub struct FsCliTool;

impl FsCliTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FsCliTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExecutableTool for FsCliTool {
    fn name(&self) -> &'static str {
        "fs"
    }

    fn description(&self) -> &'static str {
        "File system CLI. Single entry point for all file operations.\n\n\
         Usage: fs <subcommand> [--flags]\n\n\
         Subcommands:\n  \
           read    Read file contents with line numbers (--file_path required)\n  \
           write   Create or overwrite a file (--file_path, --content required)\n  \
           edit    Replace exact strings in a file (--file_path, --old_string, --new_string required)\n  \
           grep    Search file content using regex (--pattern required)\n  \
           glob    Find files/directories by glob patterns (--pattern required)\n  \
           scheme  Show parameter definitions [<subcommand>]\n\n\
         Global flags:\n  \
           --json, -o  Output as JSON instead of CLI text\n\n\
         Examples:\n  \
           fs read --file_path src/main.rs --limit 50\n  \
           fs write --file_path notes.txt --content 'hello'\n  \
           fs edit --file_path a.txt --old_string x --new_string y\n  \
           fs grep --pattern 'TODO' --output_mode content\n  \
           fs glob --pattern '**/*.rs'\n  \
           fs scheme read"
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The CLI command to execute, e.g. 'read --file_path src/main.rs --limit 50'"
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        context: &ToolContext,
    ) -> ToolResultType<ToolResult> {
        let command = args
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ToolError::InvalidArguments(
                    "Missing required parameter: 'command'. Usage: fs <subcommand> [--flags]"
                        .to_string(),
                )
            })?;

        let cmd: ParsedCommand =
            parser::parse(command).map_err(ToolError::InvalidArguments)?;

        crate::cli::executor::execute(cmd, context)
            .await
            .map_err(ToolError::ExecutionFailed)
    }
}
```

Update `crates/vol-llm-fs/src/tools/mod.rs` (add the registration function):

```rust
//! LLM tools for filesystem operations.

mod fs_cli;

pub use fs_cli::FsCliTool;

/// Register the CLI-style `fs` tool to a ToolRegistry.
pub fn register_cli(registry: &mut vol_llm_tool::ToolRegistry) {
    registry.register(FsCliTool::new());
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vol-llm-fs`
Expected: PASS — all tests (parser 13 + executor 9 + format 3 + fs_cli 7)

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-fs/
git commit -m "feat(fs): FsCliTool executable with register_cli"
```

---

### Task 5: Wire `fs` into `vol-llm-runtime`

**Files:**
- Modify: `crates/vol-llm-runtime/Cargo.toml` (add dependency)
- Modify: `crates/vol-llm-runtime/src/lib.rs` (`AgentRuntimeBuilder::build()` + `AgentRuntime::for_test()` + new test)

**Interfaces:**
- Consumes: `vol_llm_fs::tools::register_cli` (Task 4)
- Produces: `AgentRuntime::for_test()` and `AgentRuntimeBuilder::build()` register the `fs` tool

- [ ] **Step 1: Write the failing runtime test**

In `crates/vol-llm-runtime/src/lib.rs`, in the test module next to `runtime_for_test_creates_valid_runtime` (around line 1363), add:

```rust
    #[tokio::test]
    async fn for_test_registers_fs_cli_tool() {
        let rt = AgentRuntime::for_test().await;
        let names = rt.tool_registry.tool_names();
        assert!(names.iter().any(|n| *n == "fs"), "fs tool not registered: {names:?}");
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p vol-llm-runtime for_test_registers_fs_cli_tool`
Expected: FAIL — assertion panics: `fs tool not registered`

- [ ] **Step 3: Add the dependency and the two registration lines**

In `crates/vol-llm-runtime/Cargo.toml`, next to the `vol-llm-task` path dep (line 22), add:

```toml
vol-llm-fs = { path = "../vol-llm-fs" }
```

In `crates/vol-llm-runtime/src/lib.rs`, in `AgentRuntimeBuilder::build()` right after the task registration line (around line 521):

```rust
        // Register the unified CLI-style `task` tool (agents using `tools: [task]`).
        vol_llm_task::tools::register_cli(&mut tool_registry, task_store.clone());
        // Register the unified CLI-style `fs` tool — single entry point for file ops.
        vol_llm_fs::tools::register_cli(&mut tool_registry);
```

In `crates/vol-llm-runtime/src/lib.rs`, in `AgentRuntime::for_test()` right after its task registration line (around line 273):

```rust
        // Register the unified CLI-style `task` tool (agents using `tools: [task]`).
        vol_llm_task::tools::register_cli(&mut tool_registry, task_store.clone());
        // Register the unified CLI-style `fs` tool — single entry point for file ops.
        vol_llm_fs::tools::register_cli(&mut tool_registry);
```

- [ ] **Step 4: Run the runtime tests to verify they pass**

Run: `cargo test -p vol-llm-runtime for_test_registers_fs_cli_tool`
Expected: PASS

Run: `cargo test -p vol-llm-runtime` (full crate — may take a while)
Expected: PASS, no new failures

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-runtime/
git commit -m "feat(runtime): register fs cli tool alongside task"
```

---

### Task 6: Quality gates and wiki ingest

**Files:**
- Possibly: `docs/wiki/**` (via wiki-ingest skill)

**Interfaces:**
- Consumes: everything above

- [ ] **Step 1: Run the full test suite for the touched crates**

Run: `cargo test -p vol-llm-fs -p vol-llm-runtime`
Expected: PASS

- [ ] **Step 2: Coverage gate**

Run: `just cover-gate vol-llm-fs 80`
Expected: PASS — coverage ≥ 80% (exempt files per CLAUDE.md: none in this crate; `main.rs`/`app.rs`/`health.rs` don't exist here)

If coverage falls short: add unit tests for uncovered branches (e.g., `scheme_for` unknown-subcommand branch, `finalize` non-json path is already covered).

- [ ] **Step 3: No-doc-tests check**

Run: `./scripts/check-no-doc-tests.sh`
Expected: PASS

- [ ] **Step 4: Boundary check**

Run: `cargo check -p vol-llm-fs -p vol-llm-runtime`
Expected: PASS

- [ ] **Step 5: wiki-ingest**

Invoke the `wiki-ingest` skill with the source being the implementation (new crate `vol-llm-fs`, its `fs` tool, registration in runtime). Commit any wiki changes:

```bash
git add docs/wiki/
git commit -m "docs(wiki): ingest vol-llm-fs fs cli tool [skip ci]"
```

- [ ] **Step 6: Upload the plan doc to Lark (plans node)**

```bash
lark-cli docs +create --doc-format markdown \
  --content "@./docs/superpowers/plans/2026-08-16-fs-cli-tool.md" \
  --title "FS CLI Tool Implementation Plan" \
  --parent-token "TEkkw1W6niuBxQkcvswchOo5nhb" --as user
```

Expected: `"ok": true` with a document URL — report the URL to the user.
