# ReAct Agent Plugin System

A powerful plugin system for extending ReAct Agent functionality through event stream interception.

## Overview

The plugin system allows you to inject cross-cutting concerns into the agent execution flow without modifying core agent logic. Plugins can:

- **Intercept events** in the agent event stream
- **Short-circuit execution** to return cached responses
- **Skip events** to filter output
- **Abort execution** on errors or policy violations
- **Modify events** to transform data in-flight

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                      ReActAgent                                 │
│                                                                 │
│  ┌─────────────┐     ┌──────────────┐     ┌─────────────┐     │
│  │   LLM       │────▶│ Agent Core   │────▶│ PluginStream│     │
│  └─────────────┘     └──────────────┘     └─────────────┘     │
│                                              │                  │
│                    ┌─────────────────────────┤                  │
│                    │                         │                  │
│              ┌─────▼─────┐           ┌──────▼──────┐          │
│              │ Plugin 1  │           │ Plugin 2    │          │
│              │ (Rate     │           │ (HITL)      │          │
│              │ Limiter)  │           │             │          │
│              └───────────┘           └─────────────┘          │
└─────────────────────────────────────────────────────────────────┘
```

## Core Components

### `AgentPlugin` Trait

All plugins implement the `AgentPlugin` trait:

```rust
#[async_trait]
pub trait AgentPlugin: Send + Sync {
    fn id(&self) -> PluginId;
    fn priority(&self) -> u32 { 100 }
    
    async fn on_start(&self, ctx: &mut PluginContext) -> PluginAction<()>;
    async fn intercept(&self, event: StreamEvent, ctx: &PluginContext) 
        -> PluginAction<Option<StreamEvent>>;
    async fn on_complete(&self, ctx: &PluginContext, response: Option<&AgentResponse>) 
        -> PluginAction<()>;
    async fn on_error(&self, ctx: &PluginContext, error: &AgentError) 
        -> PluginAction<()>;
}
```

### `PluginAction` Return Types

| Action | Behavior |
|--------|----------|
| `Continue(event)` | Pass event to next plugin |
| `Continue(None)` | Drop event, get next |
| `ShortCircuit(response)` | Return response immediately |
| `Skip` | Skip this event |
| `Abort(error)` | Abort agent execution |

### Plugin Priority

Lower priority number = higher priority = executed first.

Built-in priority levels:
- `5` - Rate limiter (execute first)
- `10` - Observability
- `20` - Caching
- `25` - Human-in-the-Loop
- `30` - Retry (execute last)

## Usage

### Basic Plugin Registration

```rust
use vol_llm_agent::react::*;
use vol_llm_agent::plugins::CliApprovalChannel;
use std::sync::Arc;

// Create plugin
let channel = Arc::new(CliApprovalChannel);
let config = HitlConfig {
    triggers: vec![ApprovalTrigger::ToolExecution { tools: None }],
    timeout_secs: 60,
    on_timeout: TimeoutBehavior::Reject { reason: "Timeout".into() },
    ..Default::default()
};
let hitl_plugin = HitlPlugin::new(config, channel);

// Register with agent
let agent = ReActAgent::builder()
    .with_llm(llm)
    .with_tool(tool)
    .with_plugin(hitl_plugin)
    .build()?;
```

### Custom Plugin Example

```rust
use vol_llm_agent::react::*;
use async_trait::async_trait;

struct LoggingPlugin;

#[async_trait]
impl AgentPlugin for LoggingPlugin {
    fn id(&self) -> PluginId {
        "logging".to_string()
    }
    
    fn priority(&self) -> u32 {
        50 // After built-in plugins
    }
    
    async fn intercept(
        &self,
        event: Result<AgentStreamEvent, AgentError>,
        ctx: &PluginContext,
    ) -> PluginAction<Option<Result<AgentStreamEvent, AgentError>>> {
        match &event {
            Ok(AgentStreamEvent::ToolCallBegin { tool_name, .. }) => {
                tracing::info!(run_id = %ctx.run_id, "Calling tool: {}", tool_name);
            }
            Ok(AgentStreamEvent::AgentComplete { response }) => {
                tracing::info!(
                    run_id = %ctx.run_id,
                    iterations = response.iterations,
                    "Agent completed"
                );
            }
            _ => {}
        }
        
        PluginAction::Continue(Some(event))
    }
    
    async fn on_complete(
        &self,
        _ctx: &PluginContext,
        _response: Option<&AgentResponse>,
    ) -> PluginAction<()> {
        PluginAction::Continue(())
    }
}
```

### Plugin Context

`PluginContext` provides shared state across plugins:

```rust
let mut ctx = PluginContext::new(run_id, user_input, session_id);

// Store custom data
ctx.set("retry_count", 3)?;

// Retrieve data
let count: Option<i32> = ctx.get("retry_count");
```

## Built-in Plugins

### Human-in-the-Loop (HITL)

Requires human approval for tool execution or iteration continuation.

```rust
use vol_llm_agent::react::hitl::*;
use vol_llm_agent::plugins::CliApprovalChannel;

// Config: require approval for all tools
let config = HitlConfig {
    triggers: vec![ApprovalTrigger::ToolExecution { tools: None }],
    timeout_secs: 300,
    on_timeout: TimeoutBehavior::Stop,
    ..Default::default()
};

// CLI channel (prompts in terminal)
let channel = Arc::new(CliApprovalChannel);
let plugin = HitlPlugin::new(config, channel);
```

### HTTP Approval Channel

For remote approval via HTTP callbacks:

```rust
use vol_llm_agent::plugins::SimpleHttpApprovalChannel;

let channel = SimpleHttpApprovalChannel::new();

// Get handle for HTTP handler
let pending = channel.pending_requests();

// Create axum router (requires 'http' feature)
#[cfg(feature = "http")]
let router = channel.create_router();
```

## Event Flow

```
AgentStart
    │
    ▼
┌─────────────────┐
│ on_start hooks  │
└─────────────────┘
    │
    ▼
[Loop: Agent events]
    │
    ▼
┌─────────────────┐
│ intercept hooks │◀── Plugin 1, Plugin 2, ...
└─────────────────┘
    │
    ▼
ThinkingComplete / ToolCallBegin / ToolCallComplete / IterationComplete
    │
    ▼
[End: AgentComplete or Error]
    │
    ▼
┌──────────────────┐
│ on_complete hooks│
└──────────────────┘
```

## Features

- **Feature-gated HTTP support**: Enable with `--features http`
- **Async-first**: All plugin hooks are async
- **Thread-safe**: Plugins implement `Send + Sync`
- **Composable**: Stack multiple plugins with priority ordering

## Examples

Run the CLI approval example:

```bash
cargo run --example agent_cli_approval
```

Run integration tests:

```bash
cargo test -p vol-llm-agent --test plugin_test
```

## Error Handling

Plugins can abort agent execution on errors:

```rust
async fn intercept(&self, event: StreamEvent, _ctx: &PluginContext) 
    -> PluginAction<Option<StreamEvent>> 
{
    match event {
        Err(AgentError::ToolExecution { tool, error }) => {
            tracing::error!("Tool {} failed: {}", tool, error);
            PluginAction::Abort(AgentError::Context(
                format!("Tool {} failed critically", tool)
            ))
        }
        _ => PluginAction::Continue(Some(event)),
    }
}
```

## Best Practices

1. **Keep plugins focused**: Each plugin should handle one concern
2. **Use appropriate priority**: Critical checks first, cleanup last
3. **Handle errors gracefully**: Return `Abort` for unrecoverable errors
4. **Document side effects**: Make plugin behavior clear to users
5. **Test in isolation**: Unit test each plugin independently

## See Also

- [API Documentation](https://docs.rs/vol-llm-agent)
- [Example: CLI Approval](examples/agent_cli_approval.rs)
- [Integration Tests](tests/plugin_test.rs)
