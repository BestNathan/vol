---
type: concept
category: architecture
tags: [observability, metrics, prometheus, otel, pull]
created: 2026-07-24
updated: 2026-08-21
source_count: 2
---

# Pull-Based Metrics

**Category:** Observability architecture
**Related:** [[agent-observability]], [[otel-log-routing]], [[built-in-plugins]], [[vol-llm-observability-crate]]

## Definition

Agent metrics are exposed via a standard Prometheus `/metrics` HTTP endpoint scraped by
Prometheus/Alloy, rather than pushed over OTLP to a collector. Introduced by the
[[observability-pull-metrics-refactor]].

## Key Points
- `MetricsPlugin` records OTel `Meter` instruments unchanged; only the exporter changed from
  OTLP push to an `opentelemetry-prometheus` pull exporter.
- Exporter and `/metrics` handler share ONE `OnceLock<prometheus::Registry>` inside
  `vol_llm_observability::metrics_router` — writer and reader must be the same registry instance.
- The endpoint is `vol_llm_observability::build_metrics_router()`, merged into agent-server's
  existing HTTP router (port 3001) alongside `/health`.
- Traces + logs still push OTLP; only metrics are pull-based.
- Pod annotations `prometheus.io/scrape=true`, `prometheus.io/path=/metrics`,
  `prometheus.io/port=3001` drive scrape discovery.

## How It Works

`otel_init` builds the meter provider with
`opentelemetry_prometheus::exporter().with_registry(metrics_router::registry().clone()).build()`.
The `/metrics` handler calls `registry().gather()` and encodes with `prometheus::TextEncoder`.
Because `prometheus::Registry` is an `Arc`-backed shared handle, both sides see the same
metric families.

### Version pitfall
`opentelemetry-prometheus` 0.29 depends on `prometheus` 0.14. `vol-llm-observability` must pin
`prometheus = "0.14"` (not the workspace 0.13) so the exporter and handler share compatible
registry types; otherwise `/metrics` returns an empty, disconnected registry.

## Instruments

Recorded by [[built-in-plugins]]' `MetricsPlugin`:
- `agent_tool_calls_total` (Counter), `agent_tool_call_duration_seconds` (Histogram)
- `agent_ttft_seconds` (Histogram) — time to first token
- `agent_tokens_used_total` (Counter), `agent_llm_call_errors_total` (Counter)
- `agent_runs_total` (Counter), `agent_run_duration_seconds` (Histogram) — run-level, new

Labels are low-cardinality only: `agent_id`, `agent_type`, `tool_name`, `model`, `status`,
`token_type`. `run_id`/`session_id` are internal correlation keys, never metric labels.

## Related Concepts
- [[agent-observability]]: the broader logging + metrics layer this belongs to
- [[otel-log-routing]]: the complementary logs path (stdout → Alloy, OTLP for traces/logs)
- [[built-in-plugins]]: MetricsPlugin and LoggingPlugin
- [[observability-pull-metrics-refactor]]: the source that introduced this
