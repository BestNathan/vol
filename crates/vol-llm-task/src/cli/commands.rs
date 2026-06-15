//! Parsed CLI command variants for the task management tool.
//!
//! Each variant maps to a specific subcommand or syntactic shortcut supported
//! by the `task` CLI.  The parser (see [`super::parser`]) yields one of these
//! values for every successfully parsed invocation.

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
    Get { id: u64, json: bool },
    /// `task list [--status S] [--assignee A]`
    List {
        status: Option<String>,
        assignee: Option<String>,
        json: bool,
    },
    /// `task stop --id <ID>`
    Stop { id: u64, json: bool },
    /// `task output --id <ID> [--block] [--timeout <MS>]`
    Output {
        id: u64,
        block: bool,
        timeout_ms: u64,
        #[allow(dead_code)]
        json: bool,
    },
    /// `task claim [--id <ID>]`
    Claim { id: Option<u64>, json: bool },
    /// `task scheme [<subcommand>]`
    Scheme { subcommand: Option<String> },
    /// `task +task --name <NAME> [--desc D] [--assignee A]`
    QuickCreate {
        name: String,
        desc: Option<String>,
        assignee: Option<String>,
        json: bool,
    },
    /// `task +done --id <ID>`
    QuickDone { id: u64, json: bool },
    /// `task +claim`
    QuickClaim { json: bool },
}
