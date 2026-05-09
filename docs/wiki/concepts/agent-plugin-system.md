---
type: concept
category: framework
tags: [plugin-system, agent, cross-cutting]
created: 2026-05-04
updated: 2026-05-06
source_count: 2
---

# Agent Plugin System

**Category:** Extension framework
**Related:** [[react-pattern]], [[plugin-actions]], [[agent-event-stream]], [[built-in-plugins]], [[otel-log-routing]], [[connection-holder-clone-limitation]], [[jsonrpc-transport-refactoring]]

## Definition

An event stream interception architecture that allows injecting cross-cutting concerns into the ReAct Agent execution flow without modifying core agent logic.

## Key Points
- Plugins implement the `AgentPlugin` trait with 4 hook points [[react-agent-docs]]
- Executed in priority order (lower number = higher priority) [[react-agent-docs]]
- `PluginContext` provides shared key-value state across plugins [[react-agent-docs]]
- Async-first, thread-safe (Send + Sync), composable [[react-agent-docs]]

## How It Works

The architecture places a `PluginStream` between the Agent Core and the output:

```
LLM → Agent Core → PluginStream → Output
                        │
                  Plugin 1, Plugin 2, ... (in priority order)
```

Each plugin implements these hooks:
- `on_start`: Called when agent begins, can abort immediately
- `intercept`: Called for each event, can transform/drop/abort
- `on_complete`: Called when agent finishes, can log/audit
- `on_error`: Called on errors, can handle or escalate

Plugin priority levels:
| Priority | Plugin | Rationale |
|----------|--------|-----------|
| 5 | [[rate-limiting]] | Execute first, gate access |
| 10 | [[agent-observability]] | Early logging for debugging |
| 20 | [[semantic-caching]] / [[otel-log-routing]] | Before expensive operations |
| 25 | [[human-in-the-loop]] | Before tool execution |
| 30 | [[retry-with-backoff]] | Execute last, after all else fails |

## Examples / Applications

- **Rate limiting**: Prevent more than N concurrent agent runs
- **Caching**: Return cached responses for semantically similar queries
- **HITL**: Require human approval before executing tools
- **Observability**: Log all events to JSONL files for audit
- **LokiPlugin (OTel)**: Stateless plugin emitting `tracing::info!` with structured fields [[otel-log-routing]]
- **Retry**: Automatically retry failed operations with backoff

## Related Concepts
- [[react-pattern]]: The loop that plugins intercept
- [[plugin-actions]]: Return types that control flow
- [[built-in-plugins]]: The plugins shipped with the system
- [[run-context]]: Context passed to plugin hooks (replaced PluginContext)
- [[plugin-context-migration]]: Migration from PluginContext to RunContext
- [[otel-log-routing]]: LokiPlugin structured OTel logging
- [[vol-llm-agent-crate]]: Where the plugin system is implemented
- [[connection-holder]]: Transport bridge plugin forwarding events to active connection
- [[vol-llm-agent-channel-crate]]: Channel crate implementing ConnectionHolder
