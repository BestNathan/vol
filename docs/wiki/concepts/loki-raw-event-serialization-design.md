---
type: concept
category: design
tags: [loki, serialization, otel, events]
created: 2026-05-14
updated: 2026-05-14
source_count: 1
---

# Loki Raw Event Serialization Design

**Category:** Design specification

**Related:** [[loki-plugin-otel-migration-design]], [[agent-event-stream]], [[agent-observability]]

## Definition

Design specification for the JSON serialization format of raw agent events before they are sent to Loki via OTel. Events are flattened into a single JSON object with structured fields.

## Key Points

- Events are serialized as flat JSON strings (not nested `LokiEntry` objects)
- Each event includes: `event_type`, `timestamp`, `namespace`, `session_id`, `agent_id`, `agent_type`, `run_id`, `model`
- Event payload is the `event` field containing serialized JSON of the specific event (ToolCallBegin, ContentDelta, etc.)
- High-frequency delta events (`ThinkingDelta`, `ContentDelta`, `ToolCallArgumentDelta`) are filtered out by `should_send()`
- Serialization uses `serde_json` for consistent JSON output

## Related Concepts
- [[agent-event-stream]]: Source of events being serialized
- [[otel-log-routing]]: Routing layer that receives serialized events
- [[loki-plugin-otel-migration-design]]: Parent migration design
