---
type: concept
category: framework
tags: [otel, tracing, structured-logging, agent-observability]
created: 2026-05-06
updated: 2026-05-06
source_count: 1
---

# OTel Log Routing via Tracing

**Category:** Observability framework
**Related:** [[agent-observability]], [[agent-plugin-system]], [[loki-plugin-otel-migration-design]]

## Definition

Agent events are routed to the OTel Collector via `tracing::info!` macros with structured fields. The `tracing-subscriber` stack (extended with `opentelemetry-appender-tracing`) intercepts these macros and forwards them through the OTel SDK's `BatchLogProcessor` via gRPC.

## Architecture

```
tracing::info!(...) ─────┐
                         │
   tracing-subscriber    │
   ┌──────────────────┐  │
   │  fmt::layer      │──┼→ stdout + JSONL files (existing, unchanged)
   │                  │  │
   │  OTel log layer  │──┘
   └───────┬──────────┘
           │
   OTel SDK BatchLogProcessor
           │
   opentelemetry-otlp (gRPC)
           │
   OTel Collector (endpoint from env)
```

## Key Points

- **Stateless plugins**: LokiPlugin holds no state, no endpoint, no config. It only calls `tracing::info!`.
- **Structured fields**: Each log entry carries `namespace`, `session_id`, `agent_id`, `agent_type`, `run_id`, `model`, and `event` (serialized JSON).
- **Error resilience**: If the OTel Collector is unavailable, the `BatchLogProcessor` buffers and drops on timeout without blocking agent execution. If OTel is not initialized, `tracing::info!` falls through to console/file layers.
- **Empty model handling**: `RunContext::new()` normalizes empty `model` string to `"unknown"`.
- **Event filtering**: High-frequency streaming delta events (`ThinkingDelta`, `ContentDelta`, `ToolCallArgumentDelta`) are skipped to reduce noise.

## Structured Log Fields

| Field | Source | Example |
|-------|--------|---------|
| `namespace` | Fixed | `"agent"` |
| `session_id` | `RunContext.session_id` | `"sess-abc123"` |
| `agent_id` | `RunContext.config.def.name` | `"vol_advice"` |
| `agent_type` | `RunContext.config.def.type` | `"coding"` |
| `run_id` | `RunContext.run_id` | `"run-xyz789"` |
| `model` | `RunContext.model` | `"qwen3.5-plus"` |
| `event` | Serialized event JSON | `{"event":"ToolCallBegin",...}` |

## Initialization Flow

1. `vol-monitor` `tracing_setup.rs` initializes the tracing subscriber stack.
2. `init_otel_logs()` creates `OtlpLogExporter` with gRPC transport.
3. `LoggerProvider` is configured with `BatchLogProcessor` and resource attributes.
4. OTel log layer is integrated into the `tracing_subscriber` Registry.

## Related Concepts

- [[agent-observability]]: JSONL logging (existing file-based logging, complementary)
- [[agent-plugin-system]]: Plugin architecture LokiPlugin implements
- [[agent-event-stream]]: Events being routed to OTel
- [[built-in-plugins]]: LokiPlugin as a built-in plugin
- [[loki-plugin-otel-migration-design]]: Design specification
- [[run-context]]: Holds `model` field for log enrichment
