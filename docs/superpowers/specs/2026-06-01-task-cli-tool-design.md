# TaskCliTool Design Spec

## Overview

Replace the 7 separate task tools (TaskCreate, TaskUpdate, TaskGet, TaskList, TaskStop, TaskOutput, TaskClaim) with a single `TaskCliTool` that accepts CLI-style command strings, similar to how a user interacts with a command-line tool.

### Motivation

- Single tool = smaller LLM context (one tool definition vs seven)
- CLI syntax is familiar to LLMs, easy to compose
- Shortcut commands (`+task`, `+done`, `+claim`) with smart defaults reduce parameter burden
- `scheme` subcommand lets LLM query parameter definitions before constructing commands
- clap handles parsing, validation, and help text generation

## Subcommands

### Standard Commands (strict required params)

| Subcommand | Description |
|------------|-------------|
| `create` | Create a new task — `--name`, `--desc` required |
| `update` | Update a task — `--id` required, plus fields to change |
| `get` | Get task details by `--id` |
| `list` | List tasks, optional `--status` / `--assignee` filter |
| `stop` | Stop a running task by `--id` |
| `output` | Read task output by `--id` |
| `claim` | Claim a pending task by `--id` |
| `scheme` | Show parameter definitions for a subcommand |

### Shortcut Commands (`+` prefix, minimal params + smart defaults)

| Shortcut | Equivalent | Required | Auto-filled |
|----------|-----------|----------|-------------|
| `+task` | `create` shortcut | `--name` | status=Pending, assignee=current agent, desc="", deps=[] |
| `+done` | `update` shortcut | `--id` | status=Completed |
| `+claim` | `claim` shortcut | none | claims first ready Pending task, assignee=current agent |

Shortcut commands are syntax sugar — they expand to standard commands internally after filling defaults.

### Global Flags

| Flag | Description |
|------|-------------|
| `-o json` | Output as JSON instead of CLI text |

## Tool Interface

### Registration as Single Tool

```rust
// LLM sees ONE tool, not seven
{
  "name": "task",
  "description": "Task management CLI. Usage: task <subcommand> [--flags]\n\n...",
  "parameters": {
    "type": "object",
    "properties": {
      "command": {
        "type": "string",
        "description": "The CLI command to execute"
      }
    },
    "required": ["command"]
  }
}
```

### LLM Invocation Examples

```
task create --name 'Fix login' --desc 'Handle OAuth callback error' --assignee coding-agent
task +task --name 'Quick fix'
task update --id 1 --status completed
task list --status pending
task get --id 42
task +done --id 42
task +claim
task scheme create
task get --id 42 -o json
```

## Output Format

### Default: CLI-style text

```
task create --name 'Fix login' --desc 'Handle OAuth error'
  → Task t42 created: "Fix login" [Pending] assignee: coding-agent

task get --id 42
  → Task t42: "Fix login"
     Status:     Running
     Assignee:   coding-agent
     Created:    2026-06-01 14:30
     Dependencies: t40, t41
     Blocks:     t45

task list --status pending
  → 3 pending tasks:
     t42  "Fix login"     [Pending]  coding-agent
     t43  "Add logging"   [Pending]  -
     t44  "Refactor auth" [Pending]  -

task scheme create
  → create parameters:
     --name        (required)  Task subject
     --desc        (required)  Task description
     --assignee    (optional)  Agent type to assign
     --activeForm  (optional)  Spinner display text
     --deps        (optional)  Comma-separated task IDs
     --blocks      (optional)  Comma-separated task IDs
```

### `-o json` mode

```
task get --id 42 -o json
  → {"id": 42, "subject": "Fix login", "status": "Running", ...}
```

Always returns valid ToolResult with `content` (text) and `data` (structured JSON for programmatic use).

## Smart Defaults

Shortcut commands fill omitted fields automatically:

- **assignee**: current agent type (from ToolContext.agent_def)
- **status**: Pending (for create), Completed (for +done)
- **desc**: empty string
- **deps / blocks**: empty arrays
- **id** (for +claim): first Pending task with all dependencies Completed

Standard commands enforce required params — if missing, return error with hint to use shortcut:
```
Error: --desc is required for 'create'. Use '+task' shortcut for quick create.
```

## Internal Architecture

### File Layout

```
crates/vol-llm-task/src/
├── cli/
│   ├── mod.rs          — module entry
│   ├── parser.rs       — clap Command builder, parses command string → ParsedCommand
│   ├── commands.rs     — ParsedCommand enum + per-subcommand params structs
│   ├── executor.rs     — ParsedCommand → TaskStore calls + default filling
│   └── format.rs       — Output formatting: CLI text / JSON

tools/
├── task_cli.rs         — TaskCliTool (ExecutableTool impl), wires above modules
└── mod.rs              — register_cli() + existing register_all() (mutually exclusive)
```

### Parsing with clap

```rust
use clap::{Command, Arg};

fn build_cli() -> Command {
    Command::new("task")
        .arg(Arg::new("json").short('o').value_parser(["json"]))
        .subcommand(
            Command::new("create")
                .arg(Arg::new("name").long("name").required(true))
                .arg(Arg::new("desc").long("desc").required(true))
                .arg(Arg::new("assignee").long("assignee"))
                .arg(Arg::new("activeForm").long("activeForm"))
                .arg(Arg::new("deps").long("deps").value_delimiter(','))
                .arg(Arg::new("blocks").long("blocks").value_delimiter(','))
        )
        .subcommand(
            Command::new("+task")
                .arg(Arg::new("name").long("name").required(true))
                .arg(Arg::new("desc").long("desc"))
                .arg(Arg::new("assignee").long("assignee"))
        )
        .subcommand(
            Command::new("update")
                .arg(Arg::new("id").long("id").required(true))
                .arg(Arg::new("status").long("status"))
                .arg(Arg::new("subject").long("subject"))
                .arg(Arg::new("desc").long("desc"))
                .arg(Arg::new("assignee").long("assignee"))
                .arg(Arg::new("activeForm").long("activeForm"))
                // dependency management
                .arg(Arg::new("addDeps").long("addDeps").value_delimiter(','))
                .arg(Arg::new("addBlocks").long("addBlocks").value_delimiter(','))
        )
        // ... remaining subcommands
}
```

### Execution Flow

```
command_string: "create --name 'Fix' --desc '...'"
  ↓ parser.rs (clap)
ParsedCommand::Create { name: "Fix", desc: "...", assignee: None, ... }
  ↓ executor.rs
  - Standard: validate required params → build Task → store.create()
  - Shortcut: defaults::fill() → validate → build Task → store.create()
  ↓ format.rs
  - json flag? → serde_json::to_string(&task)
  - default → CLI text formatter
ToolResult { success: true, content: "...", data: {...} }
```

### Registration (Mutually Exclusive)

```rust
// Old tools — existing agents
pub fn register_all(registry: &mut ToolRegistry, store: Arc<dyn TaskStore>) {
    registry.register(TaskCreate::new(store.clone()));
    // ... 6 others
}

// New CLI tool — test agent only
pub fn register_cli(registry: &mut ToolRegistry, store: Arc<dyn TaskStore>) {
    registry.register(TaskCliTool::new(store));
}
```

Agent config uses tool allowlist to select one set:
- Existing agents: `tools: ["task_create", "task_get", "task_list", "task_update", "task_stop", "task_output", "task_claim"]`
- Test agent: `tools: ["task"]`

### Tool Sensitivity

Mirrors existing sensitivity rules:
- `create`, `get`, `list`, `output`, `claim`, `scheme` → Safe
- `update`, `stop` → RequiresApproval

## Testing Plan

1. **Unit tests** — parser.rs: verify clap parsing for each subcommand and shortcut
2. **Unit tests** — executor.rs: verify TaskStore calls with correct params
3. **Unit tests** — format.rs: verify CLI text and JSON output
4. **Unit tests** — defaults.rs: verify smart defaults for shortcut commands
5. **Integration** — create a test agent with only `task` tool in allowlist
6. **Manual testing** — interact with the test agent and verify CLI-style task operations work end-to-end

## Scope

**In scope:**
- `TaskCliTool` with all 7 subcommands + 3 shortcuts + scheme
- clap-based parser
- CLI text and JSON output modes
- Smart defaults for shortcuts
- Test agent configuration

**Out of scope:**
- Replacing existing 7 tools (keep both, mutually exclusive)
- Additional shortcut commands beyond `+task`, `+done`, `+claim`
- Shell-style piping or redirection
- Tab completion or interactive mode
