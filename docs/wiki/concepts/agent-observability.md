---
type: concept
category: framework
tags: [observability, logging, jsonl, tracing, otel]
created: 2026-05-04
updated: 2026-05-06
source_count: 2
---

# Agent Observability

**Category:** Observability framework
**Related:** [[agent-plugin-system]], [[agent-event-stream]], [[built-in-plugins]], [[otel-log-routing]]

## Definition

The observability layer provides comprehensive logging of agent execution events through two complementary mechanisms:

1. **Local JSONL logging**: The observability plugin writes events to JSONL files and stdout for local debugging.
2. **OTel log routing**: LokiPlugin sends events to the OTel Collector via `tracing::info!` macros with structured fields [[loki-plugin-otel-migration-tasks-3-4]].

## Key Points
- Dual output: JSONL files (complete) + human-readable stdout [[react-agent-docs]]
- Agent-centric directory structure: `logs/agents/{agent_id}/{sessions,runs}/` [[react-agent-docs]]
- Retention policy: session logs 7 days, run logs last 10 [[react-agent-docs]]
- Non-blocking: logging failures never crash the agent [[react-agent-docs]]
- OTel integration: structured logs forwarded to OTel Collector via gRPC [[otel-log-routing]]
- Stateless LokiPlugin: no HTTP, no endpoint, just `tracing::info!` calls [[loki-plugin-otel-migration-tasks-3-4]]

## How It Works

### Local Logging

The observability plugin runs at priority 10 (early in the chain). It uses a `RunLogLogger` that writes to two locations:

**Run logs** (`run_{run_id}.jsonl`): All events for a single agent run, useful for debugging a specific execution.

**Session logs** (`session_{session_id}_{YYYYMMDD}.jsonl`): All events grouped by session and date, useful for cross-run analysis.

Cleanup happens at agent startup in a background task:
- Session logs older than 7 days are deleted
- Only the 10 most recent run logs are kept

Log format:
```json
{"timestamp":"2026-04-10T12:34:56.789Z","run_id":"run_abc123","agent_id":"vol_advice","event":"ToolCallBegin","data":{"tool_name":"market_data"}}
```

### OTel Log Routing

LokiPlugin (priority 20) emits structured `tracing::info!` calls with fields: `namespace`, `session_id`, `agent_id`, `agent_type`, `run_id`, `model`, `event`. The tracing-subscriber stack routes these through the OTel SDK's `BatchLogProcessor` to the OTel Collector [[otel-log-routing]].

High-frequency streaming delta events (`ThinkingDelta`, `ContentDelta`, `ToolCallArgumentDelta`) are filtered out to reduce noise.

## Related Concepts
- [[agent-plugin-system]]: The plugin architecture it implements
- [[agent-event-stream]]: The events it records
- [[built-in-plugins]]: Its place in the built-in plugin set
- [[otel-log-routing]]: OTel Collector integration via tracing macros
- [[loki-plugin-otel-migration-tasks-3-4]]: LokiPlugin rewrite implementation
