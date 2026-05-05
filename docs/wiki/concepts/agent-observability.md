---
type: concept
category: framework
tags: [observability, logging, jsonl, tracing]
created: 2026-05-04
updated: 2026-05-04
source_count: 1
---

# Agent Observability

**Category:** Observability framework
**Related:** [[agent-plugin-system]], [[agent-event-stream]], [[built-in-plugins]]

## Definition

A built-in plugin that provides comprehensive logging of agent execution events in JSONL format to both files and stdout.

## Key Points
- Dual output: JSONL files (complete) + human-readable stdout [[react-agent-docs]]
- Agent-centric directory structure: `logs/agents/{agent_id}/{sessions,runs}/` [[react-agent-docs]]
- Retention policy: session logs 7 days, run logs last 10 [[react-agent-docs]]
- Non-blocking: logging failures never crash the agent [[react-agent-docs]]

## How It Works

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

## Related Concepts
- [[agent-plugin-system]]: The plugin architecture it implements
- [[agent-event-stream]]: The events it records
- [[built-in-plugins]]: Its place in the built-in plugin set
