---
type: concept
category: framework
tags: [tools, trait, execution]
created: 2026-05-04
updated: 2026-05-04
source_count: 1
---

# Tool Trait

**Category:** Tool definition framework
**Related:** [[tool-registry]], [[tool-context]], [[react-pattern]]

## Definition

The `Tool` trait that all tools must implement, defining the interface for tool execution in the ReAct Agent system. Includes `ToolResult` (execution result) and `ToolContext` (execution context).

## Key Points
- `Tool` trait requires: `name()`, `description()`, `parameters()`, `execute()` [[agent-tool-design]]
- `ToolResult` contains: `call_id`, `success`, `content`, `error`, `data` (structured) [[agent-tool-design]]
- `ToolContext` provides: `alert` (optional), `messages` (history), `metadata` (key-value map) [[agent-tool-design]]
- `to_definition()` converts tool to `ToolDefinition` for LLM function calling [[agent-tool-design]]
- Async execution: `execute()` returns `Result<ToolResult, Box<dyn Error + Send>>` [[agent-tool-design]]

## How It Works

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Option<serde_json::Value>;
    fn to_definition(&self) -> ToolDefinition;
    async fn execute(&self, args: &str, context: &ToolContext) -> Result<ToolResult, Box<dyn Error + Send>>;
}
```

Tools are registered in `ToolRegistry` by their unique name. The registry exports all tool definitions to the LLM for function calling, and dispatches tool calls by name during execution.

Built-in tools:
| Tool | Purpose | Parameters |
|------|---------|------------|
| `market_data` | Real-time market prices | symbol, data_type |
| `alert_history` | Volatility index history | symbol, tenor, alert_type |
| `iv_curve` | Implied volatility curves | symbol, tenor |
| `rule_info` | Alert rule definitions | alert_type |

## Related Concepts
- [[tool-registry]]: Manages Tool implementations
- [[tool-context]]: Context passed to execute()
- [[react-pattern]]: Tools invoked during Act phase
- [[skill-system]]: Skills also register as tools via SkillTool
- [[vol-llm-tool-crate]]: Where Tool trait is defined
