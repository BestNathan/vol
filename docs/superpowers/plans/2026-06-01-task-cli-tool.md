# TaskCliTool Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace 7 separate task tools with a single CLI-style `task` tool using clap for argument parsing.

**Architecture:** A new `cli/` module under `vol-llm-task` contains the parser (clap Command builder + shell tokenizer), an executor that maps parsed commands to TaskStore calls with smart defaults, and a formatter for CLI-text / JSON output. `TaskCliTool` implements `ExecutableTool`, accepting a single `command` string. Registered via `register_cli()` — mutually exclusive with existing `register_all()`.

**Tech Stack:** Rust, clap (builder API), TaskStore trait, ExecutableTool trait

---

### Task 1: Add clap dependency

**Files:**
- Modify: `crates/vol-llm-task/Cargo.toml`

- [ ] **Step 1: Add clap to Cargo.toml**

```toml
[dependencies]
# ... existing deps ...
clap = { version = "4", features = ["string"] }
```

The `string` feature is needed for `value_parser` with string types.

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p vol-llm-task`
Expected: succeeds (clap downloads, no code using it yet)

---

### Task 2: Create cli/mod.rs — module structure

**Files:**
- Create: `crates/vol-llm-task/src/cli/mod.rs`
- Modify: `crates/vol-llm-task/src/lib.rs`

- [ ] **Step 1: Create cli/mod.rs**

```rust
//! CLI-style task tool — parser, executor, and formatter.

pub(crate) mod commands;
pub(crate) mod executor;
pub(crate) mod format;
pub(crate) mod parser;
```

- [ ] **Step 2: Register cli module in lib.rs**

Read `crates/vol-llm-task/src/lib.rs` to see its current content, then add `pub(crate) mod cli;` alongside the existing module declarations.

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p vol-llm-task`
Expected: fails because submodules don't exist yet (that's fine — next tasks create them)

---

### Task 3: Create cli/commands.rs — ParsedCommand enum

**Files:**
- Create: `crates/vol-llm-task/src/cli/commands.rs`

- [ ] **Step 1: Write ParsedCommand enum**

```rust
/// Parsed CLI command — one variant per subcommand.
#[derive(Debug, Clone)]
pub(crate) enum ParsedCommand {
    /// `task create --name <NAME> --desc <DESC> [--assignee A] [--activeForm AF] [--deps 1,2] [--blocks 3]`
    Create {
        name: String,
        desc: String,
        assignee: Option<String>,
        active_form: Option<String>,
        deps: Vec<u64>,
        blocks: Vec<u64>,
        json: bool,
    },
    /// `task update --id <ID> [--status S] [--subject S] [--desc D] [--assignee A] [--activeForm AF] [--addDeps 1,2] [--addBlocks 3]`
    Update {
        id: u64,
        status: Option<String>,
        subject: Option<String>,
        desc: Option<String>,
        assignee: Option<String>,
        active_form: Option<String>,
        add_deps: Vec<u64>,
        add_blocks: Vec<u64>,
        json: bool,
    },
    /// `task get --id <ID>`
    Get {
        id: u64,
        json: bool,
    },
    /// `task list [--status S] [--assignee A]`
    List {
        status: Option<String>,
        assignee: Option<String>,
        json: bool,
    },
    /// `task stop --id <ID>`
    Stop {
        id: u64,
        json: bool,
    },
    /// `task output --id <ID>`
    Output {
        id: u64,
        json: bool,
    },
    /// `task claim [--id <ID>]`
    Claim {
        id: Option<u64>,
        json: bool,
    },
    /// `task scheme [<subcommand>]`
    Scheme {
        subcommand: Option<String>,
    },
    /// `task +task --name <NAME> [--desc D] [--assignee A]`
    QuickCreate {
        name: String,
        desc: Option<String>,
        assignee: Option<String>,
        json: bool,
    },
    /// `task +done --id <ID>`
    QuickDone {
        id: u64,
        json: bool,
    },
    /// `task +claim`
    QuickClaim {
        json: bool,
    },
}
```

No tests for this file — it's just a data enum.

---

### Task 4: Create cli/parser.rs — clap builder + shell tokenizer

**Files:**
- Create: `crates/vol-llm-task/src/cli/parser.rs`

- [ ] **Step 1: Write the tokenizer function (no test yet)**

```rust
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
```

- [ ] **Step 2: Write the clap Command builder**

```rust
use clap::{Arg, Command, value_parser};

/// Build the full clap CLI definition for all task subcommands.
pub(crate) fn build_cli() -> Command {
    Command::new("task")
        .arg(
            Arg::new("json")
                .short('o')
                .num_args(0..=1)
                .default_missing_value("json")
                .require_equals(false)
                .value_parser(value_parser!(String))
        )
        // --- Standard commands ---
        .subcommand(
            Command::new("create")
                .about("Create a new task")
                .arg(Arg::new("name").long("name").required(true))
                .arg(Arg::new("desc").long("desc").required(true))
                .arg(Arg::new("assignee").long("assignee"))
                .arg(Arg::new("activeForm").long("activeForm"))
                .arg(Arg::new("deps").long("deps").value_delimiter(',').value_parser(value_parser!(u64)))
                .arg(Arg::new("blocks").long("blocks").value_delimiter(',').value_parser(value_parser!(u64)))
        )
        .subcommand(
            Command::new("update")
                .about("Update a task")
                .arg(Arg::new("id").long("id").required(true).value_parser(value_parser!(u64)))
                .arg(Arg::new("status").long("status").value_parser(["pending", "running", "completed", "failed", "killed"]))
                .arg(Arg::new("subject").long("subject"))
                .arg(Arg::new("desc").long("desc"))
                .arg(Arg::new("assignee").long("assignee"))
                .arg(Arg::new("activeForm").long("activeForm"))
                .arg(Arg::new("addDeps").long("addDeps").value_delimiter(',').value_parser(value_parser!(u64)))
                .arg(Arg::new("addBlocks").long("addBlocks").value_delimiter(',').value_parser(value_parser!(u64)))
        )
        .subcommand(
            Command::new("get")
                .about("Get task details")
                .arg(Arg::new("id").long("id").required(true).value_parser(value_parser!(u64)))
        )
        .subcommand(
            Command::new("list")
                .about("List tasks")
                .arg(Arg::new("status").long("status").value_parser(["pending", "running", "completed", "failed", "killed"]))
                .arg(Arg::new("assignee").long("assignee"))
        )
        .subcommand(
            Command::new("stop")
                .about("Stop a running task")
                .arg(Arg::new("id").long("id").required(true).value_parser(value_parser!(u64)))
        )
        .subcommand(
            Command::new("output")
                .about("Read task output")
                .arg(Arg::new("id").long("id").required(true).value_parser(value_parser!(u64)))
        )
        .subcommand(
            Command::new("claim")
                .about("Claim a pending task")
                .arg(Arg::new("id").long("id").value_parser(value_parser!(u64)))
        )
        .subcommand(
            Command::new("scheme")
                .about("Show parameter definitions for a subcommand")
                .arg(Arg::new("subcommand").required(false))
        )
        // --- Shortcut commands ---
        .subcommand(
            Command::new("+task")
                .about("Quick create — minimal params, smart defaults")
                .arg(Arg::new("name").long("name").required(true))
                .arg(Arg::new("desc").long("desc"))
                .arg(Arg::new("assignee").long("assignee"))
        )
        .subcommand(
            Command::new("+done")
                .about("Quick complete — set status to completed")
                .arg(Arg::new("id").long("id").required(true).value_parser(value_parser!(u64)))
        )
        .subcommand(
            Command::new("+claim")
                .about("Quick claim — claim first ready pending task")
        )
}
```

- [ ] **Step 3: Write the parse function**

```rust
use super::commands::ParsedCommand;

/// Parse a command string into a `ParsedCommand`.
pub(crate) fn parse(input: &str) -> Result<ParsedCommand, String> {
    let tokens = tokenize(input);
    let cli = build_cli();

    let matches = cli.try_get_matches_from(tokens)
        .map_err(|e| format!("Parse error: {}\nUsage: task <subcommand> [--flags]. Use 'task scheme <sub>' to see parameters.", e))?;

    let json = matches.get_one::<String>("json").map(|s| s == "json").unwrap_or(false);

    match matches.subcommand() {
        Some(("create", m)) => Ok(ParsedCommand::Create {
            name: m.get_one::<String>("name").cloned().unwrap_or_default(),
            desc: m.get_one::<String>("desc").cloned().unwrap_or_default(),
            assignee: m.get_one::<String>("assignee").cloned(),
            active_form: m.get_one::<String>("activeForm").cloned(),
            deps: m.get_many::<u64>("deps").map(|v| v.copied().collect()).unwrap_or_default(),
            blocks: m.get_many::<u64>("blocks").map(|v| v.copied().collect()).unwrap_or_default(),
            json,
        }),
        Some(("update", m)) => Ok(ParsedCommand::Update {
            id: *m.get_one::<u64>("id").unwrap_or(&0),
            status: m.get_one::<String>("status").cloned(),
            subject: m.get_one::<String>("subject").cloned(),
            desc: m.get_one::<String>("desc").cloned(),
            assignee: m.get_one::<String>("assignee").cloned(),
            active_form: m.get_one::<String>("activeForm").cloned(),
            add_deps: m.get_many::<u64>("addDeps").map(|v| v.copied().collect()).unwrap_or_default(),
            add_blocks: m.get_many::<u64>("addBlocks").map(|v| v.copied().collect()).unwrap_or_default(),
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
        _ => Err("Unknown subcommand. Use 'task scheme' to see available subcommands.".to_string()),
    }
}
```

- [ ] **Step 4: Write unit tests in the same file**

```rust
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
        let tokens = tokenize("create --name 'fix login' --desc \"handle OAuth error\"");
        assert_eq!(tokens, vec![
            "create",
            "--name", "fix login",
            "--desc", "handle OAuth error",
        ]);
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
        let cmd = parse("create --name fix --desc repair --assignee coding --activeForm Fixing --deps 1,2").unwrap();
        match cmd {
            ParsedCommand::Create { name, desc, assignee, active_form, deps, .. } => {
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
            ParsedCommand::List { status, .. } => assert_eq!(status, Some("pending".into())),
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
            ParsedCommand::Scheme { subcommand } => assert_eq!(subcommand, Some("create".into())),
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
        let cmd = parse("get --id 1 -o json").unwrap();
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
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p vol-llm-task -- cli::parser`
Expected: 17 tests pass

---

### Task 5: Create cli/format.rs — CLI text and JSON output

**Files:**
- Create: `crates/vol-llm-task/src/cli/format.rs`

- [ ] **Step 1: Write the formatter**

```rust
use crate::model::{Task, TaskStatus};

/// Format a single task as a human-readable detail block.
pub(crate) fn fmt_task_detail(task: &Task) -> String {
    let mut out = format!(
        "Task {}: \"{}\"\n\
         Status:       {:?}\n\
         Assignee:     {}\n\
         Created:      {:?}\n\
         Dependencies: {}\n\
         Blocks:       {}",
        task.id,
        task.subject,
        task.status,
        task.assignee.as_deref().unwrap_or("-"),
        task.created_at,
        fmt_task_ids(&task.dependencies),
        fmt_task_ids(&task.blocks),
    );
    if let Some(ref desc) = task.description {
        if !desc.is_empty() {
            out.push_str(&format!("\nDesc:         {}", desc));
        }
    }
    if let Some(ref af) = task.active_form {
        out.push_str(&format!("\nActiveForm:   {}", af));
    }
    out
}

/// Format a list of tasks as a compact table.
pub(crate) fn fmt_task_list(tasks: &[Task]) -> String {
    if tasks.is_empty() {
        return "No tasks found.".to_string();
    }
    let mut out = format!("{} task(s):\n", tasks.len());
    for task in tasks {
        out.push_str(&format!(
            "  {}  \"{}\"  [{:?}]  {}\n",
            task.id,
            task.subject,
            task.status,
            task.assignee.as_deref().unwrap_or("-"),
        ));
    }
    out.trim_end().to_string()
}

/// Format a create confirmation.
pub(crate) fn fmt_create_confirm(task: &Task) -> String {
    format!(
        "Task {} created: \"{}\" [{:?}] assignee: {}",
        task.id,
        task.subject,
        task.status,
        task.assignee.as_deref().unwrap_or("-"),
    )
}

/// Format a scheme (parameter list) for a specific subcommand.
pub(crate) fn fmt_scheme(subcommand: &str, params: &[(&str, bool, &str)]) -> String {
    let mut out = format!("{} parameters:\n", subcommand);
    for (name, required, desc) in params {
        let req = if *required { "(required)" } else { "(optional)" };
        out.push_str(&format!("  --{:<14} {:<10} {}\n", name, req, desc));
    }
    out.trim_end().to_string()
}

/// Serialize a value to JSON string, with error fallback.
pub(crate) fn to_json<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|e| format!("{{\"error\": \"{}\"}}", e))
}

fn fmt_task_ids(ids: &[crate::model::TaskId]) -> String {
    if ids.is_empty() {
        "-".to_string()
    } else {
        ids.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(", ")
    }
}
```

- [ ] **Step 2: Write unit tests in the same file**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Task, TaskId, TaskKind, TaskStatus};
    use std::time::SystemTime;

    fn sample_task() -> Task {
        Task {
            id: TaskId(42),
            status: TaskStatus::Pending,
            kind: TaskKind::Agent,
            publisher: Some("test-agent".into()),
            assignee: Some("coding-agent".into()),
            subject: "fix login bug".into(),
            description: "handle OAuth callback".into(),
            active_form: Some("Fixing login bug".into()),
            dependencies: vec![TaskId(1), TaskId(2)],
            blocks: vec![TaskId(50)],
            result: None,
            summary: None,
            output_file: None,
            created_at: SystemTime::now(),
            started_at: None,
            completed_at: None,
        }
    }

    #[test]
    fn test_fmt_task_detail() {
        let output = fmt_task_detail(&sample_task());
        assert!(output.contains("Task t42"));
        assert!(output.contains("fix login bug"));
        assert!(output.contains("Pending"));
        assert!(output.contains("coding-agent"));
        assert!(output.contains("t1, t2"));
        assert!(output.contains("t50"));
        assert!(output.contains("handle OAuth callback"));
    }

    #[test]
    fn test_fmt_task_list_empty() {
        assert_eq!(fmt_task_list(&[]), "No tasks found.");
    }

    #[test]
    fn test_fmt_task_list_nonempty() {
        let tasks = vec![sample_task()];
        let output = fmt_task_list(&tasks);
        assert!(output.contains("1 task(s)"));
        assert!(output.contains("t42"));
        assert!(output.contains("fix login bug"));
    }

    #[test]
    fn test_fmt_create_confirm() {
        let output = fmt_create_confirm(&sample_task());
        assert!(output.contains("Task t42 created"));
        assert!(output.contains("fix login bug"));
    }

    #[test]
    fn test_fmt_scheme() {
        let params = vec![
            ("name", true, "Task subject"),
            ("desc", true, "Task description"),
            ("assignee", false, "Agent type"),
        ];
        let output = fmt_scheme("create", &params);
        assert!(output.contains("create parameters"));
        assert!(output.contains("--name"));
        assert!(output.contains("(required)"));
        assert!(output.contains("(optional)"));
    }

    #[test]
    fn test_to_json() {
        let json = to_json(&sample_task());
        assert!(json.contains("\"fix login bug\""));
        assert!(json.starts_with('{'));
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p vol-llm-task -- cli::format`
Expected: 5 tests pass

---

### Task 6: Create cli/executor.rs — execute commands against TaskStore

**Files:**
- Create: `crates/vol-llm-task/src/cli/executor.rs`

- [ ] **Step 1: Write the executor**

```rust
use std::sync::Arc;

use vol_llm_tool::{ToolContext, ToolResult};

use super::commands::ParsedCommand;
use crate::model::{Task, TaskId, TaskKind, TaskStatus};
use crate::store::TaskStore;

/// Scheme parameter definition helper.
pub(crate) struct ParamDef {
    pub name: &'static str,
    pub required: bool,
    pub description: &'static str,
}

/// Execute a parsed command against the task store.
pub(crate) async fn execute(
    cmd: ParsedCommand,
    store: &Arc<dyn TaskStore>,
    context: &ToolContext,
) -> Result<ToolResult, String> {
    match cmd {
        ParsedCommand::Create { name, desc, assignee, active_form, deps, blocks, json } => {
            let mut task = Task::new(TaskKind::Agent, name.clone(), deps.into_iter().map(TaskId).collect());
            task.description = desc;
            task.active_form = active_form;
            task.assignee = assignee;
            task.publisher = context.agent_def.as_ref().map(|a| a.r#type.clone());
            task.blocks = blocks.into_iter().map(TaskId).collect();

            let id = store.create(task).await.map_err(|e| format!("Failed to create task: {}", e))?;
            let created = store.get(&id).await.map_err(|e| format!("Failed to read task: {}", e))?
                .ok_or_else(|| "Task not found after creation".to_string())?;

            let content = if json {
                super::format::to_json(&created)
            } else {
                super::format::fmt_create_confirm(&created)
            };
            Ok(ToolResult {
                success: true,
                content,
                error: None,
                data: Some(serde_json::to_value(&created).unwrap_or_default()),
                call_id: String::new(),
            })
        }

        ParsedCommand::Update { id, status, subject, desc, assignee, active_form, add_deps, add_blocks, json } => {
            let task_id = TaskId(id);
            let task = store.get(&task_id).await.map_err(|e| format!("Failed to get task: {}", e))?
                .ok_or_else(|| format!("Task t{} not found", id))?;

            let mut task = task;
            let mut updated = Vec::new();

            if let Some(s) = subject { task.subject = s; updated.push("subject"); }
            if let Some(d) = desc { task.description = d; updated.push("description"); }
            if let Some(a) = assignee { task.assignee = Some(a); updated.push("assignee"); }
            if let Some(af) = active_form { task.active_form = Some(af); updated.push("activeForm"); }
            if let Some(s) = status {
                task.status = parse_status(&s)?;
                updated.push("status");
            }
            for dep_id in add_deps {
                let tid = TaskId(dep_id);
                if !task.dependencies.contains(&tid) { task.dependencies.push(tid); }
            }
            if !add_deps.is_empty() { updated.push("dependencies"); }
            for block_id in add_blocks {
                let tid = TaskId(block_id);
                if !task.blocks.contains(&tid) { task.blocks.push(tid); }
            }
            if !add_blocks.is_empty() { updated.push("blocks"); }

            store.update(task).await.map_err(|e| format!("Failed to update task: {}", e))?;

            let updated_task = store.get(&task_id).await.map_err(|e| format!("Failed to read task: {}", e))?
                .ok_or_else(|| "Task not found after update".to_string())?;

            let content = if json {
                super::format::to_json(&updated_task)
            } else {
                format!("Task {} updated: {}", task_id, updated.join(", "))
            };
            Ok(ToolResult {
                success: true,
                content,
                error: None,
                data: Some(serde_json::json!({
                    "taskId": id.to_string(),
                    "updatedFields": updated,
                })),
                call_id: String::new(),
            })
        }

        ParsedCommand::Get { id, json } => {
            let task_id = TaskId(id);
            let task = store.get(&task_id).await.map_err(|e| format!("Failed to get task: {}", e))?
                .ok_or_else(|| format!("Task t{} not found", id))?;

            let content = if json {
                super::format::to_json(&task)
            } else {
                super::format::fmt_task_detail(&task)
            };
            Ok(ToolResult {
                success: true,
                content,
                error: None,
                data: Some(serde_json::to_value(&task).unwrap_or_default()),
                call_id: String::new(),
            })
        }

        ParsedCommand::List { status, assignee, json } => {
            let status_filter = status.map(|s| parse_status(&s)).transpose()?;
            let all = store.list(status_filter).await.map_err(|e| format!("Failed to list tasks: {}", e))?;

            let tasks: Vec<Task> = if let Some(ref a) = assignee {
                all.into_iter().filter(|t| t.assignee.as_deref() == Some(a.as_str())).collect()
            } else {
                all
            };

            let content = if json {
                super::format::to_json(&tasks)
            } else {
                super::format::fmt_task_list(&tasks)
            };
            Ok(ToolResult {
                success: true,
                content,
                error: None,
                data: Some(serde_json::to_value(&tasks).unwrap_or_default()),
                call_id: String::new(),
            })
        }

        ParsedCommand::Stop { id, json } => {
            let task_id = TaskId(id);
            let task = store.get(&task_id).await.map_err(|e| format!("Failed to get task: {}", e))?
                .ok_or_else(|| format!("Task t{} not found", id))?;

            let mut task = task;
            task.status = TaskStatus::Killed;
            store.update(task).await.map_err(|e| format!("Failed to stop task: {}", e))?;

            let content = if json {
                format!("{{\"success\": true, \"taskId\": \"{}\", \"status\": \"killed\"}}", id)
            } else {
                format!("Task t{} stopped (killed)", id)
            };
            Ok(ToolResult {
                success: true,
                content,
                error: None,
                data: Some(serde_json::json!({"taskId": id.to_string(), "status": "killed"})),
                call_id: String::new(),
            })
        }

        ParsedCommand::Output { id, json: _ } => {
            let task_id = TaskId(id);
            let task = store.get(&task_id).await.map_err(|e| format!("Failed to get task: {}", e))?
                .ok_or_else(|| format!("Task t{} not found", id))?;

            match (task.output_file.as_ref(), task.result.as_ref()) {
                (Some(path), _) => {
                    match tokio::fs::read_to_string(path).await {
                        Ok(output) => Ok(ToolResult {
                            success: true,
                            content: output,
                            error: None,
                            data: Some(serde_json::json!({"taskId": id.to_string()})),
                            call_id: String::new(),
                        }),
                        Err(e) => Ok(ToolResult {
                            success: false,
                            content: format!("Failed to read output file: {}", e),
                            error: Some(format!("IO error: {}", e)),
                            data: Some(serde_json::json!({"taskId": id.to_string()})),
                            call_id: String::new(),
                        }),
                    }
                }
                (_, Some(result)) => Ok(ToolResult {
                    success: true,
                    content: result.output_truncated.clone(),
                    error: None,
                    data: Some(serde_json::json!({"taskId": id.to_string()})),
                    call_id: String::new(),
                }),
                _ => Ok(ToolResult {
                    success: false,
                    content: format!("No output for task t{}", id),
                    error: Some("No output available".to_string()),
                    data: Some(serde_json::json!({"taskId": id.to_string()})),
                    call_id: String::new(),
                }),
            }
        }

        ParsedCommand::Claim { id, json } => {
            let task = if let Some(tid) = id {
                let task_id = TaskId(tid);
                store.get(&task_id).await.map_err(|e| format!("Failed to get task: {}", e))?
                    .ok_or_else(|| format!("Task t{} not found", tid))?
            } else {
                let ready = store.get_ready_tasks().await.map_err(|e| format!("Failed to get ready tasks: {}", e))?;
                let first = ready.first().ok_or_else(|| "No pending tasks available to claim".to_string())?;
                store.get(first).await.map_err(|e| format!("Failed to get task: {}", e))?
                    .ok_or_else(|| "Task not found".to_string())?
            };

            if task.status != TaskStatus::Pending {
                return Err(format!("Task {} is already {:?}", task.id, task.status));
            }

            let mut task = task;
            task.status = TaskStatus::Running;
            task.assignee = Some(context.agent_def.as_ref().map(|a| a.r#type.clone()).unwrap_or_default());
            task.started_at = Some(std::time::SystemTime::now());
            let task_id = task.id;
            store.update(task).await.map_err(|e| format!("Failed to claim task: {}", e))?;

            let content = if json {
                format!("{{\"success\": true, \"taskId\": \"{}\", \"status\": \"running\"}}", task_id.0)
            } else {
                format!("Task {} claimed and set to Running", task_id)
            };
            Ok(ToolResult {
                success: true,
                content,
                error: None,
                data: Some(serde_json::json!({"taskId": task_id.0.to_string(), "status": "running"})),
                call_id: String::new(),
            })
        }

        ParsedCommand::Scheme { subcommand } => {
            let content = scheme_for(subcommand.as_deref());
            Ok(ToolResult {
                success: true,
                content,
                error: None,
                data: None,
                call_id: String::new(),
            })
        }

        // Shortcuts — fill defaults then delegate
        ParsedCommand::QuickCreate { name, desc, assignee, json } => {
            let agent_type = context.agent_def.as_ref().map(|a| a.r#type.clone());
            let cmd = ParsedCommand::Create {
                name,
                desc: desc.unwrap_or_default(),
                assignee: assignee.or_else(|| agent_type),
                active_form: None,
                deps: vec![],
                blocks: vec![],
                json,
            };
            Box::pin(execute(cmd, store, context)).await
        }

        ParsedCommand::QuickDone { id, json } => {
            let cmd = ParsedCommand::Update {
                id,
                status: Some("completed".to_string()),
                subject: None,
                desc: None,
                assignee: None,
                active_form: None,
                add_deps: vec![],
                add_blocks: vec![],
                json,
            };
            Box::pin(execute(cmd, store, context)).await
        }

        ParsedCommand::QuickClaim { json } => {
            let cmd = ParsedCommand::Claim { id: None, json };
            Box::pin(execute(cmd, store, context)).await
        }
    }
}

fn parse_status(s: &str) -> Result<TaskStatus, String> {
    match s.to_lowercase().as_str() {
        "pending" => Ok(TaskStatus::Pending),
        "running" => Ok(TaskStatus::Running),
        "completed" => Ok(TaskStatus::Completed),
        "failed" => Ok(TaskStatus::Failed),
        "killed" => Ok(TaskStatus::Killed),
        _ => Err(format!("Invalid status: {}. Valid: pending, running, completed, failed, killed", s)),
    }
}

/// Return scheme (parameter definitions) for a subcommand, or list all subcommands.
fn scheme_for(subcommand: Option<&str>) -> String {
    match subcommand {
        Some("create") => super::format::fmt_scheme("create", &[
            ("name", true, "Task subject"),
            ("desc", true, "Task description"),
            ("assignee", false, "Agent type to assign"),
            ("activeForm", false, "Spinner display text"),
            ("deps", false, "Comma-separated dependency task IDs"),
            ("blocks", false, "Comma-separated blocking task IDs"),
        ]),
        Some("update") => super::format::fmt_scheme("update", &[
            ("id", true, "Task ID to update"),
            ("status", false, "New status: pending|running|completed|failed|killed"),
            ("subject", false, "New subject"),
            ("desc", false, "New description"),
            ("assignee", false, "Reassign to agent type"),
            ("activeForm", false, "Spinner display text"),
            ("addDeps", false, "Add dependencies (comma-separated IDs)"),
            ("addBlocks", false, "Add blocking tasks (comma-separated IDs)"),
        ]),
        Some("get") => super::format::fmt_scheme("get", &[
            ("id", true, "Task ID to retrieve"),
        ]),
        Some("list") => super::format::fmt_scheme("list", &[
            ("status", false, "Filter by status"),
            ("assignee", false, "Filter by assignee"),
        ]),
        Some("stop") => super::format::fmt_scheme("stop", &[
            ("id", true, "Task ID to stop"),
        ]),
        Some("output") => super::format::fmt_scheme("output", &[
            ("id", true, "Task ID to read output from"),
        ]),
        Some("claim") => super::format::fmt_scheme("claim", &[
            ("id", false, "Task ID to claim (omit to claim first ready)"),
        ]),
        Some("+task") => super::format::fmt_scheme("+task", &[
            ("name", true, "Task subject"),
            ("desc", false, "Task description"),
            ("assignee", false, "Agent type (defaults to current agent)"),
        ]),
        Some("+done") => super::format::fmt_scheme("+done", &[
            ("id", true, "Task ID to mark completed"),
        ]),
        Some("+claim") => super::format::fmt_scheme("+claim", &[
        ]),
        _ => {
            let mut out = String::from("Available subcommands:\n");
            for (name, desc) in &[
                ("create", "Create a new task"),
                ("update", "Update a task"),
                ("get", "Get task details"),
                ("list", "List tasks"),
                ("stop", "Stop a running task"),
                ("output", "Read task output"),
                ("claim", "Claim a pending task"),
                ("scheme", "Show parameter definitions"),
                ("+task", "Quick create with smart defaults"),
                ("+done", "Quick complete a task"),
                ("+claim", "Quick claim first ready task"),
            ] {
                out.push_str(&format!("  {:<12} {}\n", name, desc));
            }
            out.push_str("\nUse 'task scheme <subcommand>' for detailed parameters.");
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TaskKind;
    use crate::stores::InMemoryTaskStore;

    fn store() -> Arc<dyn TaskStore> {
        Arc::new(InMemoryTaskStore::new())
    }

    fn ctx() -> ToolContext {
        ToolContext::default()
    }

    #[tokio::test]
    async fn test_execute_create() {
        let s = store();
        let cmd = ParsedCommand::Create {
            name: "test task".into(), desc: "do something".into(),
            assignee: None, active_form: None, deps: vec![], blocks: vec![], json: false,
        };
        let result = execute(cmd, &s, &ctx()).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("test task"));

        let tasks = s.list(None).await.unwrap();
        assert_eq!(tasks.len(), 1);
    }

    #[tokio::test]
    async fn test_execute_create_json() {
        let s = store();
        let cmd = ParsedCommand::Create {
            name: "json task".into(), desc: "test".into(),
            assignee: None, active_form: None, deps: vec![], blocks: vec![], json: true,
        };
        let result = execute(cmd, &s, &ctx()).await.unwrap();
        assert!(result.success);
        assert!(result.content.starts_with('{'));
    }

    #[tokio::test]
    async fn test_execute_get() {
        let s = store();
        let task = Task::new(TaskKind::Agent, "find me".into(), vec![]);
        let id = s.create(task).await.unwrap();

        let cmd = ParsedCommand::Get { id: id.0, json: false };
        let result = execute(cmd, &s, &ctx()).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("find me"));
    }

    #[tokio::test]
    async fn test_execute_get_not_found() {
        let s = store();
        let cmd = ParsedCommand::Get { id: 999, json: false };
        let result = execute(cmd, &s, &ctx()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[tokio::test]
    async fn test_execute_list() {
        let s = store();
        s.create(Task::new(TaskKind::Agent, "task a".into(), vec![])).await.unwrap();
        s.create(Task::new(TaskKind::Agent, "task b".into(), vec![])).await.unwrap();

        let cmd = ParsedCommand::List { status: None, assignee: None, json: false };
        let result = execute(cmd, &s, &ctx()).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("task a"));
        assert!(result.content.contains("task b"));
    }

    #[tokio::test]
    async fn test_execute_list_filter_status() {
        let s = store();
        s.create(Task::new(TaskKind::Agent, "pending task".into(), vec![])).await.unwrap();
        let cmd = ParsedCommand::List { status: Some("completed".into()), assignee: None, json: false };
        let result = execute(cmd, &s, &ctx()).await.unwrap();
        assert!(result.content.contains("No tasks found"));
    }

    #[tokio::test]
    async fn test_execute_update_status() {
        let s = store();
        let task = Task::new(TaskKind::Agent, "update me".into(), vec![]);
        let id = s.create(task).await.unwrap();

        let cmd = ParsedCommand::Update {
            id: id.0, status: Some("completed".into()), subject: None, desc: None,
            assignee: None, active_form: None, add_deps: vec![], add_blocks: vec![], json: false,
        };
        let result = execute(cmd, &s, &ctx()).await.unwrap();
        assert!(result.success);

        let updated = s.get(&id).await.unwrap().unwrap();
        assert_eq!(updated.status, TaskStatus::Completed);
    }

    #[tokio::test]
    async fn test_execute_stop() {
        let s = store();
        let task = Task::new(TaskKind::Agent, "kill me".into(), vec![]);
        let id = s.create(task).await.unwrap();

        let cmd = ParsedCommand::Stop { id: id.0, json: false };
        let result = execute(cmd, &s, &ctx()).await.unwrap();
        assert!(result.success);

        let stopped = s.get(&id).await.unwrap().unwrap();
        assert_eq!(stopped.status, TaskStatus::Killed);
    }

    #[tokio::test]
    async fn test_execute_claim() {
        let s = store();
        let task = Task::new(TaskKind::Agent, "claim me".into(), vec![]);
        let id = s.create(task).await.unwrap();

        let cmd = ParsedCommand::Claim { id: Some(id.0), json: false };
        let result = execute(cmd, &s, &ctx()).await.unwrap();
        assert!(result.success);

        let claimed = s.get(&id).await.unwrap().unwrap();
        assert_eq!(claimed.status, TaskStatus::Running);
    }

    #[tokio::test]
    async fn test_execute_quick_create() {
        let s = store();
        let cmd = ParsedCommand::QuickCreate {
            name: "quick one".into(), desc: None, assignee: None, json: false,
        };
        let result = execute(cmd, &s, &ctx()).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("quick one"));
    }

    #[tokio::test]
    async fn test_execute_quick_done() {
        let s = store();
        let task = Task::new(TaskKind::Agent, "finish me".into(), vec![]);
        let id = s.create(task).await.unwrap();

        let cmd = ParsedCommand::QuickDone { id: id.0, json: false };
        let result = execute(cmd, &s, &ctx()).await.unwrap();
        assert!(result.success);

        let done = s.get(&id).await.unwrap().unwrap();
        assert_eq!(done.status, TaskStatus::Completed);
    }

    #[tokio::test]
    async fn test_execute_quick_claim() {
        let s = store();
        let task = Task::new(TaskKind::Agent, "first ready".into(), vec![]);
        let id = s.create(task).await.unwrap();

        let cmd = ParsedCommand::QuickClaim { json: false };
        let result = execute(cmd, &s, &ctx()).await.unwrap();
        assert!(result.success);

        let claimed = s.get(&id).await.unwrap().unwrap();
        assert_eq!(claimed.status, TaskStatus::Running);
    }

    #[tokio::test]
    async fn test_execute_scheme_create() {
        let s = store();
        let cmd = ParsedCommand::Scheme { subcommand: Some("create".into()) };
        let result = execute(cmd, &s, &ctx()).await.unwrap();
        assert!(result.content.contains("--name"));
        assert!(result.content.contains("required"));
    }

    #[tokio::test]
    async fn test_execute_scheme_all() {
        let s = store();
        let cmd = ParsedCommand::Scheme { subcommand: None };
        let result = execute(cmd, &s, &ctx()).await.unwrap();
        assert!(result.content.contains("create"));
        assert!(result.content.contains("+task"));
    }

    #[tokio::test]
    async fn test_parse_status_invalid() {
        assert!(parse_status("bogus").is_err());
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p vol-llm-task -- cli::executor`
Expected: 16 tests pass

---

### Task 7: Create tools/task_cli.rs — TaskCliTool (ExecutableTool impl)

**Files:**
- Create: `crates/vol-llm-task/src/tools/task_cli.rs`

- [ ] **Step 1: Write TaskCliTool**

```rust
//! TaskCliTool — single CLI-style tool replacing 7 separate task tools.

use std::sync::Arc;

use async_trait::async_trait;
use vol_llm_tool::{ExecutableTool, ToolContext, ToolResult, ToolResultType, ToolSensitivity};

use crate::cli::{commands::ParsedCommand, parser};
use crate::store::TaskStore;

pub struct TaskCliTool {
    store: Arc<dyn TaskStore>,
}

impl TaskCliTool {
    pub fn new(store: Arc<dyn TaskStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl ExecutableTool for TaskCliTool {
    fn name(&self) -> &'static str {
        "task"
    }

    fn description(&self) -> &'static str {
        "Task management CLI. Single entry point for all task operations.\n\n\
         Usage: task <subcommand> [--flags]\n\n\
         Subcommands:\n  \
           create   Create a new task (--name, --desc required)\n  \
           update   Update a task (--id required)\n  \
           get      Get task details by --id\n  \
           list     List tasks [--status S] [--assignee A]\n  \
           stop     Stop a running task (--id required)\n  \
           output   Read task output (--id required)\n  \
           claim    Claim a pending task [--id ID]\n  \
           scheme   Show parameter definitions [<subcommand>]\n\n\
         Shortcuts (minimal params, smart defaults):\n  \
           +task    Quick create (--name required, rest auto-filled)\n  \
           +done    Quick complete (--id required)\n  \
           +claim   Quick claim first ready pending task\n\n\
         Global flags:\n  \
           -o json  Output as JSON instead of CLI text\n\n\
         Examples:\n  \
           task create --name 'Fix login' --desc 'Handle OAuth error'\n  \
           task +task --name 'Quick fix'\n  \
           task update --id 1 --status completed\n  \
           task list --status pending\n  \
           task get --id 42 -o json\n  \
           task scheme create"
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The CLI command to execute, e.g. 'create --name FixBug --desc ...'"
                }
            },
            "required": ["command"]
        })
    }

    fn sensitivity(&self, args: &serde_json::Value) -> ToolSensitivity {
        // Check if the command is a mutating operation
        let cmd_str = args.get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let first_token = cmd_str.split_whitespace().next().unwrap_or("");

        match first_token {
            "update" | "stop" => ToolSensitivity::RequiresApproval {
                reason: "This operation modifies task state".to_string(),
            },
            _ => ToolSensitivity::Safe,
        }
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        context: &ToolContext,
    ) -> ToolResultType<ToolResult> {
        let command = args
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| vol_llm_tool::ToolError::InvalidArguments(
                "Missing required parameter: 'command'. Usage: task <subcommand> [--flags]".to_string()
            ))?;

        let cmd: ParsedCommand = parser::parse(command).map_err(|e| {
            vol_llm_tool::ToolError::InvalidArguments(e)
        })?;

        crate::cli::executor::execute(cmd, &self.store, context)
            .await
            .map_err(|e| vol_llm_tool::ToolError::ExecutionFailed(e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stores::InMemoryTaskStore;
    use crate::model::TaskKind;
    use crate::Task;

    fn tool() -> TaskCliTool {
        TaskCliTool::new(Arc::new(InMemoryTaskStore::new()))
    }

    #[tokio::test]
    async fn test_name_and_description() {
        let t = tool();
        assert_eq!(t.name(), "task");
        assert!(t.description().contains("create"));
    }

    #[tokio::test]
    async fn test_parameters_require_command() {
        let t = tool();
        let params = t.parameters();
        let required = params.get("required").unwrap().as_array().unwrap();
        assert!(required.iter().any(|v| v.as_str() == Some("command")));
    }

    #[tokio::test]
    async fn test_execute_create_via_cli() {
        let t = tool();
        let args = serde_json::json!({
            "command": "create --name 'test via cli' --desc 'from CLI tool'"
        });
        let result = t.execute(&args, &ToolContext::default()).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("test via cli"));
    }

    #[tokio::test]
    async fn test_execute_missing_command() {
        let t = tool();
        let args = serde_json::json!({});
        let result = t.execute(&args, &ToolContext::default()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_invalid_command() {
        let t = tool();
        let args = serde_json::json!({"command": "invalid_subcommand"});
        let result = t.execute(&args, &ToolContext::default()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_full_flow() {
        let store = Arc::new(InMemoryTaskStore::new());
        let t = TaskCliTool::new(store.clone());
        let ctx = ToolContext::default();

        // Create
        let r = t.execute(&serde_json::json!({"command": "+task --name 'e2e test'"}), &ctx).await.unwrap();
        assert!(r.success);

        // Get
        let r = t.execute(&serde_json::json!({"command": "get --id 1"}), &ctx).await.unwrap();
        assert!(r.content.contains("e2e test"));

        // Update
        let r = t.execute(&serde_json::json!({"command": "update --id 1 --status completed"}), &ctx).await.unwrap();
        assert!(r.success);

        // Verify
        let task = store.get(&crate::model::TaskId(1)).await.unwrap().unwrap();
        assert_eq!(task.status, crate::model::TaskStatus::Completed);
    }

    #[tokio::test]
    async fn test_sensitivity_safe_for_read() {
        let t = tool();
        let s = t.sensitivity(&serde_json::json!({"command": "get --id 1"}));
        assert!(matches!(s, ToolSensitivity::Safe));
    }

    #[tokio::test]
    async fn test_sensitivity_approval_for_mutate() {
        let t = tool();
        let s = t.sensitivity(&serde_json::json!({"command": "update --id 1 --status completed"}));
        assert!(matches!(s, ToolSensitivity::RequiresApproval { .. }));
    }

    #[tokio::test]
    async fn test_json_output() {
        let t = tool();
        let args = serde_json::json!({
            "command": "create --name 'json test' --desc 'test' -o json"
        });
        let result = t.execute(&args, &ToolContext::default()).await.unwrap();
        assert!(result.success);
        assert!(result.content.starts_with('{'));
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p vol-llm-task -- tools::task_cli`
Expected: 9 tests pass

---

### Task 8: Update tools/mod.rs — add register_cli

**Files:**
- Modify: `crates/vol-llm-task/src/tools/mod.rs`

- [ ] **Step 1: Add TaskCliTool module declaration and register_cli function**

Read the current file and add:

```rust
mod task_cli;
pub use task_cli::TaskCliTool;
```

Then append to the end of the file:

```rust
/// Register the CLI-style task tool (mutually exclusive with register_all).
pub fn register_cli(registry: &mut vol_llm_tool::ToolRegistry, store: Arc<dyn TaskStore>) {
    registry.register(TaskCliTool::new(store));
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p vol-llm-task`
Expected: compiles successfully

---

### Task 9: Run full test suite

**Files:** None (verification only)

- [ ] **Step 1: Run all vol-llm-task tests**

Run: `cargo test -p vol-llm-task`
Expected: all tests pass (existing + new)

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -p vol-llm-task -- -D warnings`
Expected: no warnings

---

### Task 10: Commit

- [ ] **Step 1: Commit all changes**

```bash
git add crates/vol-llm-task/Cargo.toml \
        crates/vol-llm-task/src/lib.rs \
        crates/vol-llm-task/src/cli/ \
        crates/vol-llm-task/src/tools/task_cli.rs \
        crates/vol-llm-task/src/tools/mod.rs
git commit -m "feat(task): add TaskCliTool with CLI-style interface

Single 'task' tool replacing 7 separate task tools. Uses clap for
argument parsing. Supports all CRUD operations + scheme subcommand
for parameter discovery. Shortcut commands (+task, +done, +claim)
with smart defaults for minimal-parameter workflows.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```
