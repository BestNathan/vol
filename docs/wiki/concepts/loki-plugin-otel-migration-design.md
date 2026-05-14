---
type: concept
category: design
tags: [otel, loki, migration, observability]
created: 2026-05-14
updated: 2026-05-14
source_count: 1
---

# Loki Plugin OTel Migration Design

**Category:** Design specification

**Related:** [[otel-log-routing]], [[loki-plugin-otel-migration-tasks-3-4]], [[agent-observability]]

## Definition

Design specification for migrating `LokiPlugin` from direct HTTP POST to Loki to using OTel SDK via `tracing::info!` macros with structured fields.

## Key Points

- **Before**: LokiPlugin held `Arc<LokiWriter>`, constructed HTTP POST requests to Loki endpoint
- **After**: Stateless LokiPlugin calls `tracing::info!` with structured fields (`namespace`, `session_id`, `agent_id`, `agent_type`, `run_id`, `model`, `event`)
- OTel Collector receives logs via gRPC, routes to Loki backend
- Migration done in phases: Task 1-2 (delete Loki types), Task 3-4 (rewrite plugin + model field), Task 8 (OTel 0.29 init)
- Event filtering: high-frequency delta events (`ThinkingDelta`, `ContentDelta`, `ToolCallArgumentDelta`) are skipped

## Migration Tasks

1. **Task 1-2**: Delete `LokiEntry`, `LokiWriter`, `LokiConfig`, `LokiLabels`
2. **Task 3-4**: Rewrite LokiPlugin to stateless, add `model` field to `RunContext`
3. **Task 8**: Update vol-monitor tracing_setup.rs to OTel 0.29 APIs [[otel-029-log-init]]

## Related Concepts
- [[otel-log-routing]]: Architecture for OTel log routing via tracing
- [[agent-observability]]: Observability system this migration supports
- [[otel-029-log-init]]: Implementation task for OTel 0.29 API migration
