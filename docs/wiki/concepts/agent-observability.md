---
type: concept
category: framework
tags: [observability, logging, jsonl, tracing, otel, prometheus, metrics]
created: 2026-05-04
updated: 2026-08-21
source_count: 4
---

# Agent Observability

**Category:** Observability framework
**Related:** [[agent-plugin-system]], [[agent-event-stream]], [[built-in-plugins]], [[otel-log-routing]], [[pull-based-metrics]], [[vol-llm-observability-crate]]

## Definition

The observability layer provides comprehensive logging and instrumentation of agent execution
events through two complementary mechanisms:

1. **Stdout structured logging**: `LoggingPlugin` (id `"logging"`, priority 20) emits one
   structured JSON line per non-delta event via `tracing::info!`. Alloy discovers these from
   the process stdout and forwards them to Loki. High-frequency delta events
   (`ThinkingDelta`, `ContentDelta`, `ToolCallArgumentDelta`) are filtered out.
2. **Pull-based Prometheus metrics**: `MetricsPlugin` records OTel instruments backed by an
   `opentelemetry-prometheus` exporter. Agent-server exposes `GET /metrics` on port 3001
   for Prometheus/Alloy to scrape.
3. **Local JSONL run logging**: `RunLogPlugin` (in `vol-llm-agent::run_log_plugin`, id
   `"run_log"`, priority 10) appends events to `{base_dir}/logs/{run_id}.jsonl` for
   debugging. This is agent business logic, not observability infrastructure.

## Key Points
- Single `vol-llm-observability` crate (merged from the former `vol-llm-observability` plugin crate + `vol-observability` ingest binary; renamed 2026-08-21).
- Metrics are pull-based (Prometheus scrape), not push-based (OTLP).
- Traces and logs still push OTLP to the OTel Collector (unchanged).
- LLMCallStart/Complete/Error events are now emitted in the agent loop, activating
  previously-dormant metrics (TTFT, token usage, LLM errors).
- JSONL run logging moved to `vol-llm-agent` with `session_id` added to `LogEntry`.
- IDEMPOTENT: logging failures never crash the agent.
- Coverage: 87.6% on `vol-llm-observability` (otel_init excluded as init infrastructure).

## How It Works

### LoggingPlugin
Replaces the former `LokiPlugin` + `LoggerPlugin` formatting. Stateless, listen-only
(always returns `PluginDecision::Continue`). Each event is flattened into a JSON object
with `run_id`, `session_id`, `agent_id`, `agent_type`, `model`, `event`, and
event-specific fields, then emitted via `tracing::info!`. The tracing-subscriber layer
routes to stdout (Alloy discovery) and optionally to the OTel Collector.

### MetricsPlugin
Records OTel Metrics using the `global::meter("vol-llm-agent")`. Instruments:
`agent_tool_calls_total`, `agent_tool_call_duration_seconds`, `agent_ttft_seconds`,
`agent_tokens_used_total`, `agent_llm_call_errors_total`, `agent_runs_total`,
`agent_run_duration_seconds`. Labels are low-cardinality only
(`agent_id`, `agent_type`, `tool_name`, `model`, `status`, `token_type`).

The exporter is registered against a shared `OnceLock<prometheus::Registry>` so the
`/metrics` handler reads the same registry. `opentelemetry-prometheus` 0.29 uses
`prometheus` 0.14; `vol-llm-observability` pins this directly (not the workspace 0.13).

### RunLogPlugin
Moved from `vol-llm-observability` to `vol-llm-agent::run_log_plugin` as agent business.
Writes JSONL files to `{base_dir}/logs/{run_id}.jsonl`. `LogEntry` now includes `session_id`.

## Related Concepts
- [[agent-plugin-system]]: The plugin architecture these plugins implement
- [[agent-event-stream]]: The events they record
- [[built-in-plugins]]: Their place in the built-in plugin set
- [[otel-log-routing]]: OTel Collector integration (traces + logs)
- [[pull-based-metrics]]: Prometheus pull architecture via shared registry
- [[vol-llm-observability-crate]]: The consolidated crate
- [[observability-pull-metrics-refactor]]: The source document for this refactor
