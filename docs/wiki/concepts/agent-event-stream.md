---
type: concept
category: framework
tags: [event-stream, lifecycle, agent]
created: 2026-05-04
updated: 2026-05-06
source_count: 2
---

# Agent Event Stream

**Category:** Event lifecycle
**Related:** [[react-pattern]], [[agent-plugin-system]], [[agent-observability]], [[otel-log-routing]]

## Definition

The stream of events emitted during ReAct Agent execution, which plugins can intercept, transform, or filter.

## Key Points
- Events flow through a `PluginStream` after the Agent Core produces them [[react-agent-docs]]
- Each event type represents a specific phase in the agent lifecycle [[react-agent-docs]]
- Plugins can modify, drop, or abort based on event content [[react-agent-docs]]

## Event Types

| Event | When Emitted | Data |
|-------|-------------|------|
| `AgentStart` | Agent begins execution | user_input |
| `ThinkingComplete` | LLM reasoning finished | thinking_length |
| `ToolCallBegin` | Tool execution starts | tool_name, arguments |
| `ToolCallComplete` | Tool execution finishes | tool_name, result |
| `IterationComplete` | One ReAct cycle completes | iteration count, tool_calls_count |
| `AgentComplete` | Agent finishes successfully | iterations, tool_calls_count |
| `AgentAborted` | Agent execution was aborted | reason |
| `PluginEvent` | Custom plugin-generated event | name, data |

## How It Works

Events are produced by the Agent Core and consumed by the PluginStream. Each plugin's `intercept` method receives events in order, with lower-priority plugins seeing the event first. The event carries the `run_id`, `agent_id`, and timestamp for correlation.

## Examples / Applications

- **Observability plugin**: Logs every event to JSONL files
- **LokiPlugin**: Sends events to OTel via `tracing::info!` (filters out delta events) [[otel-log-routing]]
- **HITL plugin**: Intercepts `ToolCallBegin` to request approval before proceeding
- **Retry plugin**: Intercepts errors to decide whether to retry

## Related Concepts
- [[react-pattern]]: Produces the events
- [[agent-plugin-system]]: Intercepts the events
- [[agent-observability]]: Records the events to disk
- [[otel-log-routing]]: OTel structured log routing via LokiPlugin
- [[session-as-ssot]]: Events now carry references instead of message copies
- [[run-context]]: Context available to plugin intercept/listen hooks
- [[connection-holder]]: Forwards events to active transport connection
- [[vol-llm-agent-channel-crate]]: Channel layer consuming the event stream
