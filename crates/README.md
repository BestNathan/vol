# vol-llm Crates

AI Agent capabilities for vol-monitor, providing LLM integration and ReAct-style tool execution.

## Overview

This workspace contains 4 crates that implement the AI Agent architecture:

```
┌─────────────────────────────────────────────────────────────┐
│                    vol-llm-agent                            │
│              (ReAct workflow orchestration)                 │
├─────────────────────────────────────────────────────────────┤
│  vol-llm-core          │  vol-llm-provider                 │
│  - Protocol types     │  - Anthropic impl                  │
│  - Message types      │  - OpenAI impl                     │
│  - LLMClient trait    │  - Factory functions               │
├─────────────────────────────────────────────────────────────┤
│                    vol-llm-tool                             │
│              (Tool framework & built-in tools)              │
│  - ExecutableTool trait                                     │
│  - ToolRegistry                                             │
│  - alert_history, iv_curve, market_data, rule_info         │
└─────────────────────────────────────────────────────────────┘
```

## Crates

### vol-llm-core

Core protocol types for LLM interaction:
- `LLMProvider` - Provider enumeration (Anthropic, OpenAI)
- `Message`, `MessageRole`, `MessageContent` - Conversation messages
- `Tool`, `FunctionDefinition`, `ToolCall` - Tool calling types
- `ModelConfig`, `ModelInfo` - Model configuration
- `ConversationRequest`, `ConversationResponse` - Request/response types
- `TokenUsage`, `FinishReason` - Response metadata
- `StreamEvent`, `StreamReceiver` - Streaming types
- `LLMError`, `Result` - Error handling
- `LLMClient` - Main trait for LLM interaction

### vol-llm-provider

Provider implementations:
- `AnthropicProvider` - Anthropic Claude API
- `OpenAIProvider` - OpenAI GPT API
- `LLMConfig` - Configuration loading
- `create_provider`, `load_provider` - Factory functions

### vol-llm-tool

Tool framework:
- `ExecutableTool` - Tool trait
- `ToolContext` - Execution context
- `ToolResult` - Execution result
- `ToolRegistry` - Tool management and execution
- Built-in tools:
  - `AlertHistoryTool` - Query alert history
  - `IvCurveTool` - Get IV curve data
  - `MarketDataTool` - Real-time market data
  - `RuleInfoTool` - Rule configuration info

### vol-llm-agent

ReAct Agent orchestration:
- `ReActAgent` - Main agent implementation
- `AgentConfig` - Agent configuration
- `AgentResponse` - Agent response type
- `AgentError` - Error types
- `AgentBuilder` - Fluent builder
- `default_system_prompt`, `vol_analysis_prompt` - Prompt templates

## Usage

### Basic Example

```rust
use vol_llm_core::{LLMClient, ConversationRequest};
use vol_llm_provider::{LLMConfig, create_provider};
use vol_llm_tool::{ToolRegistry, ToolContext};
use vol_llm_agent::{ReActAgent, AgentConfig, AgentBuilder};

// Load configuration
let config = LLMConfig {
    provider: LLMProvider::Anthropic,
    model: "claude-sonnet-4-20250514".to_string(),
    api_key_env: "ANTHROPIC_API_KEY".to_string(),
    endpoint: None,
};

// Create provider
let llm = create_provider(&config)?;

// Set up tools
let mut tools = ToolRegistry::new();
tools.register(AlertHistoryTool::new(24));
tools.register(IvCurveTool);
tools.register(MarketDataTool);
tools.register(RuleInfoTool);

// Create agent
let agent = AgentBuilder::new()
    .with_llm(llm)
    .with_tools(tools)
    .with_max_iterations(5)
    .verbose()
    .build()
    .expect("LLM required");

// Run agent
let context = ToolContext {
    instrument: "BTC-PERP".to_string(),
    ..Default::default()
};

let response = agent.run("What's the current IV for BTC?", &context).await?;
println!("Response: {}", response.content);
```

### Configuration

See `config/llm.example.toml` for configuration template.

Environment variables:
- `ANTHROPIC_API_KEY` - Anthropic API key
- `OPENAI_API_KEY` - OpenAI API key

## Data Flow

```
User Input
    ↓
┌─────────────────┐
│  ReAct Agent    │
│  (vol-llm-agent)│
└────────┬────────┘
         │
    ┌────┴────┐
    │  Reason │ ← Call LLM via LLMClient trait
    └────┬────┘
         │
    ┌────┴────┐
    │   Act   │ ← Execute tools via ToolRegistry
    └────┬────┘
         │
    ┌────┴─────┐
    │ Observe  │ ← Add tool results to conversation
    └────┬─────┘
         │
    ┌────┴─────┐
    │  Repeat  │ ← Loop until final response
    └────┬─────┘
         │
    ┌────┴─────┐
    │ Response │
    └──────────┘
```

## Testing

```bash
# Test individual crates
cargo test -p vol-llm-core
cargo test -p vol-llm-provider
cargo test -p vol-llm-tool
cargo test -p vol-llm-agent

# Test all
cargo test --workspace
```

## Architecture Notes

- **No Memory/RAG**: Context is passed via `ToolContext`, not stored long-term
- **Provider Abstraction**: `LLMClient` trait unifies Anthropic/OpenAI APIs
- **Tool-First**: Agent uses tools for data, LLM for reasoning
- **Async**: All operations use tokio async runtime
