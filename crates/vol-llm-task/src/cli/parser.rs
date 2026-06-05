//! CLI parser — tokenizer + clap definition + parse entry point.
//!
//! The flow is:
//!   1. `tokenize` — split a command string into tokens (respecting quotes).
//!   2. `build_cli` — return a fully-defined `clap::Command` tree.
//!   3. `parse` — combine the two and produce a [`ParsedCommand`].

use clap::{Arg, ArgAction, Command, value_parser};

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

/// Build the full clap CLI definition for all task subcommands.
pub(crate) fn build_cli() -> Command {
    Command::new("task")
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
        // --- Standard commands ---
        .subcommand(
            Command::new("create")
                .about("Create a new task")
                .arg(Arg::new("name").long("name").required(true).help("Task name"))
                .arg(Arg::new("desc").long("desc").required(true).help("Task description"))
                .arg(Arg::new("assignee").long("assignee").help("Task assignee"))
                .arg(Arg::new("activeForm").long("activeForm").help("Active form text"))
                .arg(
                    Arg::new("deps")
                        .long("deps")
                        .value_delimiter(',')
                        .value_parser(value_parser!(u64))
                        .help("Dependency task IDs"),
                )
                .arg(
                    Arg::new("blocks")
                        .long("blocks")
                        .value_delimiter(',')
                        .value_parser(value_parser!(u64))
                        .help("Blocked task IDs"),
                ),
        )
        .subcommand(
            Command::new("update")
                .about("Update a task")
                .arg(
                    Arg::new("id")
                        .long("id")
                        .required(true)
                        .value_parser(value_parser!(u64))
                        .help("Task ID"),
                )
                .arg(
                    Arg::new("status")
                        .long("status")
                        .value_parser([
                            "pending", "running", "completed", "failed", "killed",
                        ])
                        .help("New status"),
                )
                .arg(Arg::new("subject").long("subject").help("New subject"))
                .arg(Arg::new("desc").long("desc").help("New description"))
                .arg(Arg::new("assignee").long("assignee").help("New assignee"))
                .arg(Arg::new("activeForm").long("activeForm").help("New active form text"))
                .arg(
                    Arg::new("addDeps")
                        .long("addDeps")
                        .value_delimiter(',')
                        .value_parser(value_parser!(u64))
                        .help("Dependency task IDs to add"),
                )
                .arg(
                    Arg::new("addBlocks")
                        .long("addBlocks")
                        .value_delimiter(',')
                        .value_parser(value_parser!(u64))
                        .help("Blocked task IDs to add"),
                ),
        )
        .subcommand(
            Command::new("get")
                .about("Get task details")
                .arg(
                    Arg::new("id")
                        .long("id")
                        .required(true)
                        .value_parser(value_parser!(u64))
                        .help("Task ID"),
                ),
        )
        .subcommand(
            Command::new("list")
                .about("List tasks")
                .arg(
                    Arg::new("status")
                        .long("status")
                        .value_parser([
                            "pending", "running", "completed", "failed", "killed",
                        ])
                        .help("Filter by status"),
                )
                .arg(Arg::new("assignee").long("assignee").help("Filter by assignee")),
        )
        .subcommand(
            Command::new("stop")
                .about("Stop a running task")
                .arg(
                    Arg::new("id")
                        .long("id")
                        .required(true)
                        .value_parser(value_parser!(u64))
                        .help("Task ID"),
                ),
        )
        .subcommand(
            Command::new("output")
                .about("Read task output")
                .arg(
                    Arg::new("id")
                        .long("id")
                        .required(true)
                        .value_parser(value_parser!(u64))
                        .help("Task ID"),
                )
                .arg(
                    Arg::new("block")
                        .long("block")
                        .action(ArgAction::SetTrue)
                        .help("Wait for task to complete before returning output"),
                )
                .arg(
                    Arg::new("timeout")
                        .long("timeout")
                        .value_parser(value_parser!(u64))
                        .default_value("30000")
                        .help("Max wait time in milliseconds (default 30000)"),
                ),
        )
        .subcommand(
            Command::new("claim")
                .about("Claim a pending task")
                .arg(
                    Arg::new("id")
                        .long("id")
                        .value_parser(value_parser!(u64))
                        .help("Task ID (default: first available)"),
                ),
        )
        .subcommand(
            Command::new("scheme")
                .about("Show parameter definitions for a subcommand")
                .arg(Arg::new("subcommand").required(false).help("Subcommand name")),
        )
        // --- Shortcut commands ---
        .subcommand(
            Command::new("+task")
                .about("Quick create — minimal params, smart defaults")
                .arg(Arg::new("name").long("name").required(true).help("Task name"))
                .arg(Arg::new("desc").long("desc").help("Task description"))
                .arg(Arg::new("assignee").long("assignee").help("Task assignee")),
        )
        .subcommand(
            Command::new("+done")
                .about("Quick complete — set status to completed")
                .arg(
                    Arg::new("id")
                        .long("id")
                        .required(true)
                        .value_parser(value_parser!(u64))
                        .help("Task ID"),
                ),
        )
        .subcommand(
            Command::new("+claim")
                .about("Quick claim — claim first ready pending task"),
        )
}

// ---------------------------------------------------------------------------
// Parse entry point
// ---------------------------------------------------------------------------

/// Parse a command string into a `ParsedCommand`.
pub(crate) fn parse(input: &str) -> Result<ParsedCommand, String> {
    let tokens = tokenize(input);
    let cli = build_cli();

    // Ensure the "task" prefix is present so clap sees a proper argv.
    let full_tokens = if tokens.first().map(|s| s.as_str()) == Some("task") {
        tokens
    } else {
        let mut with_prefix = vec!["task".to_string()];
        with_prefix.extend(tokens);
        with_prefix
    };

    let matches = cli
        .try_get_matches_from(&full_tokens)
        .map_err(|e| {
            format!(
                "Parse error: {}\nUsage: task <subcommand> [--flags]. \
                 Use 'task scheme <sub>' to see parameters.",
                e
            )
        })?;

    let json = matches.get_flag("json");

    match matches.subcommand() {
        Some(("create", m)) => Ok(ParsedCommand::Create {
            name: m.get_one::<String>("name").cloned().unwrap_or_default(),
            desc: m.get_one::<String>("desc").cloned().unwrap_or_default(),
            assignee: m.get_one::<String>("assignee").cloned(),
            active_form: m.get_one::<String>("activeForm").cloned(),
            deps: m
                .get_many::<u64>("deps")
                .map(|v| v.copied().collect())
                .unwrap_or_default(),
            blocks: m
                .get_many::<u64>("blocks")
                .map(|v| v.copied().collect())
                .unwrap_or_default(),
            json,
        }),
        Some(("update", m)) => Ok(ParsedCommand::Update {
            id: *m.get_one::<u64>("id").unwrap_or(&0),
            status: m.get_one::<String>("status").cloned(),
            subject: m.get_one::<String>("subject").cloned(),
            desc: m.get_one::<String>("desc").cloned(),
            assignee: m.get_one::<String>("assignee").cloned(),
            active_form: m.get_one::<String>("activeForm").cloned(),
            add_deps: m
                .get_many::<u64>("addDeps")
                .map(|v| v.copied().collect())
                .unwrap_or_default(),
            add_blocks: m
                .get_many::<u64>("addBlocks")
                .map(|v| v.copied().collect())
                .unwrap_or_default(),
            json,
        }),
        Some(("get", m)) => Ok(ParsedCommand::Get {
            id: *m.get_one::<u64>("id").unwrap_or(&0),
            json,
        }),
        Some(("list", m)) => Ok(ParsedCommand::List {
            status: m.get_one::<String>("status").cloned(),
            assignee: m.get_one::<String>("assignee").cloned(),
            json,
        }),
        Some(("stop", m)) => Ok(ParsedCommand::Stop {
            id: *m.get_one::<u64>("id").unwrap_or(&0),
            json,
        }),
        Some(("output", m)) => Ok(ParsedCommand::Output {
            id: *m.get_one::<u64>("id").unwrap_or(&0),
            block: m.get_flag("block"),
            timeout_ms: *m.get_one::<u64>("timeout").unwrap_or(&30000),
            json,
        }),
        Some(("claim", m)) => Ok(ParsedCommand::Claim {
            id: m.get_one::<u64>("id").copied(),
            json,
        }),
        Some(("scheme", m)) => Ok(ParsedCommand::Scheme {
            subcommand: m.get_one::<String>("subcommand").cloned(),
        }),
        Some(("+task", m)) => Ok(ParsedCommand::QuickCreate {
            name: m.get_one::<String>("name").cloned().unwrap_or_default(),
            desc: m.get_one::<String>("desc").cloned(),
            assignee: m.get_one::<String>("assignee").cloned(),
            json,
        }),
        Some(("+done", m)) => Ok(ParsedCommand::QuickDone {
            id: *m.get_one::<u64>("id").unwrap_or(&0),
            json,
        }),
        Some(("+claim", _)) => Ok(ParsedCommand::QuickClaim { json }),
        _ => Err(
            "Unknown subcommand. Use 'task scheme' to see available subcommands."
                .to_string(),
        ),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::commands::ParsedCommand;

    #[test]
    fn test_tokenize_simple() {
        let tokens = tokenize("create --name hello");
        assert_eq!(tokens, vec!["create", "--name", "hello"]);
    }

    #[test]
    fn test_tokenize_quotes() {
        let tokens =
            tokenize("create --name 'fix login' --desc \"handle OAuth error\"");
        assert_eq!(
            tokens,
            vec!["create", "--name", "fix login", "--desc", "handle OAuth error"]
        );
    }

    #[test]
    fn test_parse_create() {
        let cmd = parse("create --name 'fix bug' --desc 'repair auth'").unwrap();
        match cmd {
            ParsedCommand::Create { name, desc, .. } => {
                assert_eq!(name, "fix bug");
                assert_eq!(desc, "repair auth");
            }
            _ => panic!("expected Create"),
        }
    }

    #[test]
    fn test_parse_create_optional() {
        let cmd = parse(
            "create --name fix --desc repair --assignee coding --activeForm Fixing --deps 1,2",
        )
        .unwrap();
        match cmd {
            ParsedCommand::Create {
                name,
                desc,
                assignee,
                active_form,
                deps,
                ..
            } => {
                assert_eq!(name, "fix");
                assert_eq!(desc, "repair");
                assert_eq!(assignee, Some("coding".into()));
                assert_eq!(active_form, Some("Fixing".into()));
                assert_eq!(deps, vec![1, 2]);
            }
            _ => panic!("expected Create"),
        }
    }

    #[test]
    fn test_parse_update() {
        let cmd = parse("update --id 5 --status completed").unwrap();
        match cmd {
            ParsedCommand::Update { id, status, .. } => {
                assert_eq!(id, 5);
                assert_eq!(status, Some("completed".into()));
            }
            _ => panic!("expected Update"),
        }
    }

    #[test]
    fn test_parse_get() {
        let cmd = parse("get --id 42").unwrap();
        match cmd {
            ParsedCommand::Get { id, .. } => assert_eq!(id, 42),
            _ => panic!("expected Get"),
        }
    }

    #[test]
    fn test_parse_list() {
        let cmd = parse("list --status pending").unwrap();
        match cmd {
            ParsedCommand::List { status, .. } => {
                assert_eq!(status, Some("pending".into()))
            }
            _ => panic!("expected List"),
        }
    }

    #[test]
    fn test_parse_stop() {
        let cmd = parse("stop --id 99").unwrap();
        match cmd {
            ParsedCommand::Stop { id, .. } => assert_eq!(id, 99),
            _ => panic!("expected Stop"),
        }
    }

    #[test]
    fn test_parse_output() {
        let cmd = parse("output --id 7").unwrap();
        match cmd {
            ParsedCommand::Output { id, .. } => assert_eq!(id, 7),
            _ => panic!("expected Output"),
        }
    }

    #[test]
    fn test_parse_output_with_block() {
        let cmd = parse("output --id 7 --block").unwrap();
        match cmd {
            ParsedCommand::Output { id, block, .. } => {
                assert_eq!(id, 7);
                assert!(block);
            }
            _ => panic!("expected Output"),
        }
    }

    #[test]
    fn test_parse_output_with_block_and_timeout() {
        let cmd = parse("output --id 7 --block --timeout 60000").unwrap();
        match cmd {
            ParsedCommand::Output { id, block, timeout_ms, .. } => {
                assert_eq!(id, 7);
                assert!(block);
                assert_eq!(timeout_ms, 60000);
            }
            _ => panic!("expected Output"),
        }
    }

    #[test]
    fn test_parse_claim() {
        let cmd = parse("claim --id 3").unwrap();
        match cmd {
            ParsedCommand::Claim { id, .. } => assert_eq!(id, Some(3)),
            _ => panic!("expected Claim"),
        }
    }

    #[test]
    fn test_parse_claim_no_id() {
        let cmd = parse("claim").unwrap();
        match cmd {
            ParsedCommand::Claim { id, .. } => assert!(id.is_none()),
            _ => panic!("expected Claim"),
        }
    }

    #[test]
    fn test_parse_scheme() {
        let cmd = parse("scheme create").unwrap();
        match cmd {
            ParsedCommand::Scheme { subcommand } => {
                assert_eq!(subcommand, Some("create".into()))
            }
            _ => panic!("expected Scheme"),
        }
    }

    #[test]
    fn test_parse_quick_create() {
        let cmd = parse("+task --name 'quick fix'").unwrap();
        match cmd {
            ParsedCommand::QuickCreate { name, .. } => assert_eq!(name, "quick fix"),
            _ => panic!("expected QuickCreate"),
        }
    }

    #[test]
    fn test_parse_quick_done() {
        let cmd = parse("+done --id 10").unwrap();
        match cmd {
            ParsedCommand::QuickDone { id, .. } => assert_eq!(id, 10),
            _ => panic!("expected QuickDone"),
        }
    }

    #[test]
    fn test_parse_quick_claim() {
        let cmd = parse("+claim").unwrap();
        assert!(matches!(cmd, ParsedCommand::QuickClaim { .. }));
    }

    #[test]
    fn test_parse_json_flag() {
        // `-o` is the short form of `--json` (a `SetTrue` boolean flag).
        let cmd = parse("get --id 1 --json").unwrap();
        match cmd {
            ParsedCommand::Get { id: 1, json: true } => {}
            _ => panic!("expected Get with json=true"),
        }
    }

    #[test]
    fn test_parse_missing_required() {
        let result = parse("create --name only");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("desc"));
    }

    #[test]
    fn test_parse_unknown_subcommand() {
        let result = parse("foobar");
        assert!(result.is_err());
    }
}
