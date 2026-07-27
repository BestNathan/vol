---
type: source
source_type: design
date: 2026-07-24
ingested: 2026-07-24
tags: [observability, metrics, prometheus, otel, logging, refactor]
---

# Observability Pull-Metrics Refactor & Crate Consolidation

**Authors/Creators:** Nathan (with Claude, subagent-driven execution)
**Date:** 2026-07-24
**Link:** docs/superpowers/specs/2026-07-24-observability-pull-metrics-refactor-design.md, docs/superpowers/plans/2026-07-24-observability-pull-metrics-refactor.md

## TL;DR
Reworked agent observability from an OTLP-push + TDengine-ingest model to an
**Alloy-discovers-stdout logs + Prometheus-pull metrics** model, and merged the two
observability crates into one. JSONL run logging moved back into the agent crate as
business logic. The dormant `LLMCallStart/Complete/Error` events are now actually emitted,
activating the previously-empty TTFT/token/error metrics.

## Key Takeaways
- `vol-llm-observability` crate deleted; all functionality merged into `vol-observability`,
  which changed from a binary (ingest service) to a library crate.
- The bespoke ingest HTTP API + TDengine writer + Loki writer pipeline was removed entirely.
- `LokiPlugin` + the event-formatting half of `LoggerPlugin` merged into a single
  `LoggingPlugin` (id `"logging"`, priority 20) that emits one structured JSON line per
  non-delta event via `tracing::info!` — Alloy discovers it from stdout.
- `LoggerPlugin` (JSONL file writer) became `RunLogPlugin`, moved to `vol-llm-agent`
  (`run_log_plugin` module) since file logging is agent business, not observability. Its
  `LogEntry` gained the previously-missing `session_id` field.
- Metrics switched from OTLP push to Prometheus pull: `opentelemetry-prometheus` exporter
  writes to a shared `OnceLock<prometheus::Registry>`; agent-server exposes `GET /metrics`
  on its existing HTTP port (3001) via `vol_observability::build_metrics_router()`.
- Traces + logs still push OTLP to the OTel Collector (unchanged); only metrics flipped to pull.
- `agent.rs` main loop now emits `LLMCallStart` (before the call), `LLMCallComplete`
  (with model + token usage, previously discarded), and `LLMCallError` (both failure paths).
- New run-level metrics: `agent_runs_total` (Counter) + `agent_run_duration_seconds`
  (Histogram), recorded on `AgentComplete`/`AgentAborted`.
- `MetricsPlugin` concurrency bug fixed: TTFT correlation keyed by
  `(agent_id, run_id, iteration)` and entries removed by exact match instead of blind `pop()`,
  so concurrent agents sharing the global meter no longer cross-contaminate.

## Detailed Summary

### Crate topology change
`vol-observability` (was: ingest binary depending on `vol-tdengine`, `reqwest`) is now a lib
crate exporting `LoggingPlugin`, `MetricsPlugin`, `build_metrics_router`, and the OTel
`init`/`OtelConfig`/`OtelGuards`. Deleted files: `main.rs`, `ingest.rs`, `tdengine_writer.rs`,
`loki_writer.rs`, `event.rs`, `config.rs`. Downstream crates (`vol-agent-server`,
`vol-mcp-servers`, `vol-llm-agents`, `vol-llm-yaml-agent`, `vol-llm-tui`, `vol-llm-ui`) were
repointed from `vol_llm_observability::*` to `vol_observability::*` / `vol_llm_agent::run_log*`.

### Prometheus registry version pitfall
`opentelemetry-prometheus` 0.29 depends on `prometheus` 0.14, but the workspace pins
`prometheus = "0.13"`. The exporter registers on a 0.14 registry, so a handler reading a 0.13
registry would return empty. Resolved by pinning `vol-observability` directly to
`prometheus = "0.14"` and sharing one `OnceLock<Registry>` between the exporter (writer) and
the `/metrics` handler (reader).

### Label cardinality
Metric labels are limited to `agent_id`, `agent_type`, `tool_name`, `model`, `status`,
`token_type`. `run_id`/`session_id` are used only as internal correlation keys, never emitted
as labels (high-cardinality hazard avoided).

### Deployment
`deploy/kustomize/base/deployment.yaml` gained `prometheus.io/scrape`, `prometheus.io/path:
/metrics`, `prometheus.io/port: "3001"` pod annotations and a named `http` containerPort.

### Coverage
`vol-observability` reached 87.6% line coverage (logging_plugin ~99%, metrics_plugin ~98%,
metrics_router 100%; `otel_init.rs` left at 0% as global-init infrastructure, analogous to the
`main.rs`/`app.rs` coverage exception).

## Entities Mentioned
- [[vol-observability-crate]]: newly consolidated observability library (this refactor created its wiki page)
- [[vol-agent-server-crate]]: mounts `/metrics` on its HTTP router
- [[vol-llm-agent-crate]]: now hosts `RunLogPlugin` + `run_log` module; emits LLMCall events
- [[vol-repository]]: workspace member list updated (removed `vol-llm-observability`)

## Concepts Covered
- [[pull-based-metrics]]: new — Prometheus pull via shared registry + `/metrics` endpoint
- [[agent-observability]]: updated — dual-crate model replaced by consolidated crate + pull metrics
- [[otel-log-routing]]: still valid — logs remain `tracing::info!` → stdout (Alloy) and OTLP
- [[built-in-plugins]]: LokiPlugin → LoggingPlugin rename; LoggerPlugin → RunLogPlugin move
- [[agent-event-stream]]: LLMCall events now emitted in the loop

## Notes
- The refactor was executed subagent-driven across 9 tasks on branch
  `refactor/observability-pull-metrics`; a concurrent process rebased the branch mid-run, so
  the work was completed in an isolated worktree from a clean Task-3 base.
- Follow-up nicety (not done): `agent_type` label is present on tool-call metrics but not on
  TTFT/token/error/run-duration metrics — an inconsistency worth unifying later.
