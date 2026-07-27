---
type: entity
category: infrastructure
tags: [observability, metrics, prometheus, otel, logging, crate]
created: 2026-07-24
updated: 2026-07-24
source_count: 1
---

# vol-observability (crate)

**Category:** Observability library crate
**Related:** [[agent-observability]], [[pull-based-metrics]], [[otel-log-routing]], [[built-in-plugins]], [[vol-llm-agent-crate]], [[vol-agent-server-crate]]

## Overview

The single consolidated observability library for the LLM agent stack. Created by the
[[observability-pull-metrics-refactor]] which merged the former `vol-llm-observability`
plugin crate and the former `vol-observability` ingest binary into one library crate.

## Key Facts
- **Library crate** (formerly a binary that ran an ingest HTTP service).
- Exports: `LoggingPlugin`, `MetricsPlugin`, `build_metrics_router`, and OTel
  `init` / `OtelConfig` / `OtelGuards`.
- Depends on `opentelemetry-prometheus = "0.29"` and `prometheus = "0.14"` (pinned directly,
  not the workspace 0.13, to match the exporter's registry type).
- Removed dependencies: `vol-tdengine`, `reqwest` (the old ingest/TDengine pipeline).

## Module Structure
- `logging_plugin.rs` — `LoggingPlugin`: structured JSON events → `tracing::info!` → stdout
  (Alloy discovery). Merges the old `LokiPlugin` + `LoggerPlugin` formatting.
- `metrics_plugin.rs` — `MetricsPlugin`: OTel Meter instruments (tool calls, TTFT, tokens,
  LLM errors, run count/duration), keyed by `(agent_id, run_id, iteration)`.
- `metrics_router.rs` — shared `OnceLock<prometheus::Registry>` + `build_metrics_router()`
  serving `GET /metrics`.
- `otel_init.rs` — full OTel init: traces + logs via OTLP push, metrics via Prometheus pull.

## What Moved Out
- `RunLogPlugin` (JSONL file writer, formerly `LoggerPlugin`) → [[vol-llm-agent-crate]]
  (`run_log_plugin` module); `LogEntry` + `append_log` → `vol_llm_agent::run_log`.

## Consumers
- [[vol-agent-server-crate]]: registers `LoggingPlugin` + `MetricsPlugin`, mounts `/metrics`.
- [[vol-mcp-servers-crate]]: uses `OtelConfig` + `init` for tracing.
- [[vol-llm-agents-crate]], `vol-llm-yaml-agent`: register logging/metrics plugins.

## Timeline
- **2026-07-24**: Created via consolidation; pull-based metrics, `/metrics` endpoint, run-level
  metrics, MetricsPlugin concurrency fix, 87.6% coverage. [[observability-pull-metrics-refactor]]
