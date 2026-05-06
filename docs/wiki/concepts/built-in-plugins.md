---
type: concept
category: framework
tags: [plugins, built-in, cross-cutting]
created: 2026-05-04
updated: 2026-05-06
source_count: 2
---

# Built-in Plugins

**Category:** Plugin catalog
**Related:** [[agent-plugin-system]], [[plugin-actions]], [[rate-limiting]], [[agent-observability]], [[semantic-caching]], [[human-in-the-loop]], [[retry-with-backoff]], [[otel-log-routing]]

## Definition

The plugins shipped with the ReAct Agent system, each handling a specific cross-cutting concern.

## Key Points
- Each plugin is focused on a single responsibility [[react-agent-docs]]
- Priority ordering ensures correct execution sequence [[react-agent-docs]]
- All plugins are thread-safe and async-compatible [[react-agent-docs]]

## Plugin Catalog

| Plugin | Priority | Purpose | Action Used |
|--------|----------|---------|-------------|
| [[rate-limiting]] | 5 | Semaphore-based concurrency control | Abort on limit exceeded |
| [[agent-observability]] | 10 | JSONL logging, tracing, audit | Continue (pass-through) |
| [[otel-log-routing]] | 20 | Structured OTel logs via `tracing::info!` | Continue (pass-through) |
| [[semantic-caching]] | 20 | Semantic cache with TTL | ShortCircuit on hit |
| [[human-in-the-loop]] | 25 | Human approval for tool execution | Abort/Continue based on approval |
| [[retry-with-backoff]] | 30 | Automatic retry with exponential backoff | Retry on error |

## How They Compose

Plugins stack via priority ordering. A typical configuration might include:
- Rate limiter (gate access)
- Observability (log everything)
- LokiPlugin (OTel structured logs)
- Caching (skip LLM for repeated queries)
- HITL (require approval for sensitive tools)

The retry plugin runs last to catch errors from any upstream plugin or the agent itself.

## Related Concepts
- [[agent-plugin-system]]: The architecture these plugins implement
- [[plugin-actions]]: The return types each plugin uses
- [[react-pattern]]: The execution flow they intercept
- [[otel-log-routing]]: LokiPlugin structured logging approach
