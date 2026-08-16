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
            (Some(q), c) if c == q => {
                in_quote = None;
                // A quoted segment that closed with nothing accumulated is an
                // explicit empty value (e.g. `--content ''`) — preserve it as a
                // token so it reaches clap instead of being dropped.
                if current.is_empty() {
                    tokens.push(String::new());
                }
            }
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
            file_path: m
                .get_one::<String>("file_path")
                .cloned()
                .unwrap_or_default(),
            offset: m.get_one::<usize>("offset").copied().unwrap_or_default(),
            limit: m.get_one::<usize>("limit").copied().unwrap_or_default(),
            json,
        }),
        Some(("write", m)) => Ok(ParsedCommand::Write {
            file_path: m
                .get_one::<String>("file_path")
                .cloned()
                .unwrap_or_default(),
            content: m.get_one::<String>("content").cloned().unwrap_or_default(),
            json,
        }),
        Some(("edit", m)) => Ok(ParsedCommand::Edit {
            file_path: m
                .get_one::<String>("file_path")
                .cloned()
                .unwrap_or_default(),
            old_string: m
                .get_one::<String>("old_string")
                .cloned()
                .unwrap_or_default(),
            new_string: m
                .get_one::<String>("new_string")
                .cloned()
                .unwrap_or_default(),
            replace_all: m.get_flag("replace_all"),
            json,
        }),
        Some(("grep", m)) => Ok(ParsedCommand::Grep {
            pattern: m.get_one::<String>("pattern").cloned().unwrap_or_default(),
            path: m.get_one::<String>("path").cloned(),
            glob: m.get_one::<String>("glob").cloned(),
            output_mode: m
                .get_one::<String>("output_mode")
                .cloned()
                .unwrap_or_default(),
            case_sensitive: m.get_flag("case_sensitive"),
            json,
        }),
        Some(("glob", m)) => Ok(ParsedCommand::Glob {
            pattern: m.get_one::<String>("pattern").cloned().unwrap_or_default(),
            path: m.get_one::<String>("path").cloned().unwrap_or_default(),
            exclude: m
                .get_many::<String>("exclude")
                .map(|v| v.cloned().collect()),
            kind: m.get_one::<String>("kind").cloned().unwrap_or_default(),
            max_results: m
                .get_one::<usize>("max_results")
                .copied()
                .unwrap_or_default(),
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
    fn tokenize_preserves_empty_quoted_value() {
        let tokens = tokenize("write --file_path a --content ''");
        assert_eq!(tokens, vec!["write", "--file_path", "a", "--content", ""]);
    }

    #[test]
    fn parse_write_accepts_empty_content() {
        let cmd = parse("write --file_path a.txt --content ''").unwrap();
        assert!(matches!(cmd, ParsedCommand::Write { content, .. } if content.is_empty()));
    }

    #[test]
    fn parse_edit_accepts_empty_new_string() {
        let cmd = parse("edit --file_path a --old_string foo --new_string ''").unwrap();
        assert!(matches!(cmd, ParsedCommand::Edit { new_string, .. } if new_string.is_empty()));
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
        assert!(matches!(
            cmd,
            ParsedCommand::Read {
                offset: 10,
                limit: 25,
                ..
            }
        ));
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
        assert!(matches!(
            cmd,
            ParsedCommand::Edit {
                replace_all: true,
                ..
            }
        ));
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
        let cmd = parse("glob --pattern '**/*.rs' --exclude target,node_modules --max_results 50")
            .unwrap();
        match cmd {
            ParsedCommand::Glob {
                exclude,
                max_results,
                ..
            } => {
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
