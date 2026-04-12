# Coding Agent

AI-powered code assistant built on the ReActAgent framework.

## Features

- **Code Understanding**: Read and analyze codebases
- **Code Modification**: Edit files with precision
- **Test & Compile**: Run tests and builds via bash
- **HITL Protection**: Dangerous operations require user confirmation
- **HTML Reports**: Visual timeline of agent execution

## Quick Start

```rust
use vol_llm_agents::coding::{CodingAgent, CodingAgentConfig};

let config = CodingAgentConfig {
    max_iterations: 10,
    working_dir: std::path::PathBuf::from("."),
    hitl_enabled: true,
    verbose: false,
    html_report_path: None,
    llm_provider_id: "anthropic-main".to_string(),
};

let agent = CodingAgent::new(config).await?;
let result = agent.run("Add a new API endpoint for user login").await?;
```

## Configuration

| Field | Default | Description |
|-------|---------|-------------|
| `max_iterations` | 10 | Maximum reasoning iterations |
| `working_dir` | "." | Working directory |
| `hitl_enabled` | true | Enable HITL for dangerous ops |
| `verbose` | false | Verbose output |
| `html_report_path` | None | HTML report output path |
| `llm_provider_id` | "anthropic-main" | LLM provider ID |

## Available Tools

- `read_file` - Read file content with line numbers
- `edit_file` - Edit file content with precise string replacement
- `bash` - Execute shell commands

## HITL Protection

Dangerous operations that require confirmation:
- `rm -rf /` and similar destructive commands
- Fork bombs
- Disk formatting
- Device writes
- Reverse shells

The HITL system uses pattern matching to detect dangerous commands in bash tool arguments. When detected, the operation is rejected.

## HTML Reports

Set `html_report_path` to generate visual timeline reports:

```rust
use vol_llm_agents::coding::{CodingAgent, CodingAgentConfig, HTMLReporter};
use std::sync::Arc;

let config = CodingAgentConfig {
    html_report_path: Some("report.html".into()),
    ..Default::default()
};

let agent = CodingAgent::new(config).await?;
let observer = Arc::new(HTMLReporter::new(
    "report.html".into(),
    "My Task".to_string(),
));
let agent = agent.with_observer(observer);

let result = agent.run("Analyze this codebase").await?;
```

The HTML report includes:
- Task summary with duration, iterations, and tool call count
- Timeline of all events (thinking, tool calls, iterations)
- Color-coded event types for easy scanning

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     CodingAgent                              │
│  (vol-llm-agents/coding)                                     │
└─────────────────────────────────────────────────────────────┘
         │
         │ uses
         ▼
┌─────────────────────────────────────────────────────────────┐
│                    ReActAgent                                │
│  (vol-llm-agent)                                             │
└─────────────────────────────────────────────────────────────┘
         │
         │ emits
         ▼
┌─────────────────────────────────────────────────────────────┐
│              AgentStreamEvent (broadcast channel)            │
└─────────────────────────────────────────────────────────────┘
         │
         │ observes
         ▼
┌─────────────────────────────────────────────────────────────┐
│              EventObserver (trait + implementations)         │
│  ┌─────────────────┐                                        │
│  │ HTMLReporter    │  (MVP implementation)                  │
│  └─────────────────┘                                        │
└─────────────────────────────────────────────────────────────┘
```

## Error Handling

The CodingAgent uses a unified error type `CodingAgentError`:

```rust
pub enum CodingAgentError {
    Agent(#[from] vol_llm_agent::AgentError),
    Tool(#[from] vol_llm_tool::ToolError),
    Observer(#[from] ObserverError),
    HITL(#[from] HITLError),
    Io(#[from] std::io::Error),
    Config(String),
    TaskFailed(String),
}
```

## Example

Run the included example:

```bash
ANTHROPIC_AUTH_TOKEN=your-api-key cargo run -p vol-llm-agents --example coding_agent_basic
```

## License

MIT
