---
type: source
source_type: report
date: 2026-04-06
ingested: 2026-05-04
tags: [agent-tools, tool-design, vol-llm-tool]
---

# AI Agent Tool Design

**Authors/Creators:** vol-monitor team
**Date:** 2026-04-06
**Link:** `docs/ai-agent/03-agent-tool-design.md`

## TL;DR
Comprehensive design for the ReAct Agent tool layer, defining the `Tool` trait, `ToolRegistry`, `ToolContext`, `ToolResult`, and built-in tools (market_data, alert_history, iv_curve, rule_info) with full code examples and usage patterns.

## Key Takeaways
- Tools implement a `Tool` trait with `name()`, `description()`, `parameters()`, and `execute()` methods
- `ToolContext` provides alert info, message history, and custom metadata to tools
- `ToolRegistry` is a `HashMap<String, Box<dyn Tool>>` with registration, definition export, and execution
- 4 built-in tools designed: market_data, alert_history, iv_curve, rule_info
- Package structure: vol-llm-core (protocols), vol-llm-provider (implementations), vol-llm-tool (tools), vol-llm-agent (orchestration)
- Agent builder pattern for fluent configuration
- System prompt templates with tool descriptions injection
- Error hierarchy: AgentError → Llm/ToolExecution/MaxIterationsReached/InvalidToolResponse/Context
- Retry strategy with exponential backoff for retryable LLM errors

## Detailed Summary

The document provides a complete design for the agent tool system. It defines the package dependency graph where vol-llm-agent depends on vol-llm-tool and vol-llm-core, and vol-llm-tool depends on vol-llm-core. The `Tool` trait requires name, description, JSON schema parameters, and async execute method returning `ToolResult`. `ToolContext` carries the current alert, message history, and metadata map.

The tool registry manages tool lifecycle: registration by unique name, export of all definitions for LLM function calling, and dispatch of tool calls with context. Four built-in tools are specified with their parameter schemas and descriptions, targeting TDengine data sources for market data, volatility indices, options IV curves, and realized volatility.

The ReAct Agent layer design includes `AgentState` enum (Init → Reasoning → ExecutingTool → AwaitingObservation → Completed/Error), `ReActOutcome` (ToolCall or FinalResponse), and the full `ReActAgent` loop implementation. The `AgentBuilder` pattern provides fluent construction with validation (LLM is required). System prompts include tool descriptions and custom instructions via `SystemPromptBuilder`.

Error handling follows a hierarchy with `thiserror` derives, and a retry strategy distinguishes retryable (Network, RateLimit, 5xx) from non-retryable errors. The design includes streaming agent extension with `AgentStreamEvent` enum for real-time monitoring.

## Entities Mentioned
- [[vol-llm-agent-crate]]: Agent layer with ReAct loop and builder
- [[vol-llm-tool-crate]]: Tool trait, registry, and built-in tools
- [[vol-llm-core-crate]]: Core protocols (Message, ToolDefinition, LLMClient)
- [[vol-llm-provider-crate]]: Anthropic/OpenAI provider implementations
- [[tdengine]]: Data source for tool queries

## Concepts Covered
- [[tool-registry]]: Tool registration and execution framework
- [[tool-trait]]: The Tool trait and ToolResult/ToolContext types
- [[tool-context]]: Tool execution context with alert, messages, metadata
- [[agent-builder-pattern]]: Fluent builder for agent configuration
- [[react-pattern]]: The ReAct loop that invokes tools
- [[agent-error-handling]]: Error hierarchy and retry strategy

## Notes
- This is the original design document; actual implementation has evolved significantly since (e.g., RunContext replaced PluginContext, Session became SSOT)
- The tool macro (`define_tool!`) was proposed as an optional convenience but may not be implemented
- Streaming agent design was marked as TODO
