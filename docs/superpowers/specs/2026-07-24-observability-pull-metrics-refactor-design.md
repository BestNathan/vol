# Observability Refactor: Pull-based Metrics & Crate Consolidation

**Date**: 2026-07-24
**Status**: Draft
**Author**: Claude (brainstorming session with Nathan)

## Overview

Refactor the agent observability stack from an **OTLP-push + TDengine-ingest** model to
an **Alloy stdout log discovery + Prometheus pull metrics** model, and consolidate the
two observability crates into one. Agent-business logging (JSONL run logs) is moved back
into the agent crate where it belongs.

The current architecture has three defects that motivate this work:

1. **Metrics are half-empty**: `MetricsPlugin` depends on `LLMCallStart/Complete/Error`
   events that the ReAct agent loop never emits. TTFT, token-usage, and LLM-error metrics
   are therefore always zero.
2. **Wrong metrics backend**: Metrics are written to TDengine via a bespoke HTTP ingest
   service (`vol-observability` binary). This duplicates what the OTel/Prometheus
   ecosystem already provides.
3. **Muddled crate boundaries**: Two crates (`vol-llm-observability`, `vol-observability`)
   with overlapping responsibilities, and `LoggerPlugin` (JSONL file logging — an agent
   concern) lives in the observability crate but is never registered on the server.

## Goals

- Metrics exposed via a standard `/metrics` endpoint (Prometheus pull), no push.
- Logs discovered by Alloy from process stdout (unchanged tracing path).
- A single observability crate (`vol-observability`) owning logging plugin, metrics
  plugin, OTel init, and config definitions.
- JSONL run logging moved into `vol-llm-agent` as agent business logic.
- `LLMCallStart/Complete/Error` events actually emitted, activating the dormant metrics.

## Non-Goals

- Adding INFO-level structured logging inside the agent main loop (explicitly rejected —
  would pollute run-level logs and be redundant with the event stream).
- Keeping TDengine, the ingest HTTP API, or Loki-writer code in any form.
- Grafana dashboards / alerting rules (separate concern).
- Changing the traces/logs OTLP-push path (only metrics change to pull).

## Configuration

Metrics configuration follows **standard OTel environment variables**; no custom config
struct is added for metrics:

- `OTEL_SERVICE_NAME`
- `OTEL_EXPORTER_OTLP_ENDPOINT` (traces + logs push only)
- `OTEL_SAMPLE_RATE`
- `OTEL_METRICS_EXPORTER=prometheus` (selects the Prometheus pull exporter)

The existing `[opentelemetry]` section in `agent-server.toml` continues to drive
**traces + logs** OTLP push. Metrics no longer use `endpoint`/`batch_*` settings.

The `/metrics` HTTP route is mounted on the agent-server's **existing HTTP port**
(alongside `/health`, `/ws`), so no host/port config is needed.

## Architecture

```
┌──────────────────────── agent-server process ────────────────────────┐
│                                                                       │
│  ReActAgent loop (vol-llm-agent)                                      │
│    emits AgentStreamEvent (incl. LLMCallStart/Complete/Error) ──┐     │
│                                                                 │     │
│  Plugins (registered on data-plane):                            │     │
│    ├─ LoggingPlugin   (vol-observability) ── tracing::info! ──► stdout│
│    ├─ MetricsPlugin   (vol-observability) ── OTel Meter ──┐     │     │
│    └─ RunLogPlugin    (vol-llm-agent)     ── JSONL file        │     │
│                                                           │           │
│  HTTP router (existing port):                             │           │
│    ├─ /health                                             ▼           │
│    ├─ /ws                                    opentelemetry-prometheus  │
│    └─ /metrics  ◄──────────────────────────  registry → text encode   │
│                                                                       │
└───────────┬───────────────────────────────────────────┬─────────────┘
            │ stdout                                      │ scrape /metrics
            ▼                                             ▼
        ┌────────┐                                   ┌──────────┐
        │ Alloy  │ ── logs ──► Loki                  │Prometheus│ (pull)
        └────────┘                                   └──────────┘
```

Traces + logs still push OTLP to the OTel Collector (unchanged). Only metrics flip to
Prometheus pull via `/metrics`.

## Work Items

### 1. Crate consolidation

| Action | Target |
|---|---|
| **Delete** | `vol-llm-observability` crate (contents merged into `vol-observability`) |
| **Delete** | `vol-observability/src/main.rs`, `ingest.rs`, `tdengine_writer.rs`, `loki_writer.rs`, `event.rs` (the ingest/TDengine/Loki-writer pipeline) |
| **Move** | `RunLogPlugin` (formerly `LoggerPlugin`) + `run_log/` module → `vol-llm-agent` |
| **Convert** | `vol-observability` from binary → lib crate, exporting `LoggingPlugin`, `MetricsPlugin`, OTel `init`/`OtelConfig`/`OtelGuards`, and `build_metrics_router` |
| **Deps: remove** | `vol-tdengine`, `reqwest`, and the `opentelemetry-otlp` metrics exporter |
| **Deps: add** | `opentelemetry-prometheus` (+ its `prometheus` registry types as required) |

`vol-observability` keeps the OTLP push path for **traces + logs** (`otel_init.rs`).

### 2. RunLogPlugin (moved into vol-llm-agent)

- Writes JSONL run logs to `{working_dir}/logs/{run_id}.jsonl` (and
  `{working_dir}/logs/{plugin_name}/{run_id}.jsonl` for `PluginEvent`).
- **Adds `session_id`** to `LogEntry` (currently missing — only `run_id` is recorded).
- Keeps `should_log()` filtering of the three delta events.
- Registration remains a decision of whichever agent wants file logging (e.g. coding
  agent already registers it); no server-side change required.

### 3. LoggingPlugin (merge of LokiPlugin + LoggerPlugin formatting)

- Single plugin whose `listen()` emits one structured JSON line per non-delta event via
  `tracing::info!`, carrying `run_id / session_id / agent_id / agent_type / model`.
- Merges `LoggerPlugin::create_log_entry`'s field-expansion logic with
  `LokiPlugin::create_event_json`'s flattening so the emitted JSON is complete.
- `should_send()` continues to filter `ThinkingDelta / ContentDelta / ToolCallArgumentDelta`.
- Output flows through the existing tracing layers (console + file) to stdout; Alloy
  discovers it there. The tracing wrapper is acceptable (no need to bypass it).

### 4. MetricsPlugin + `/metrics` endpoint

- **OTel Meter API unchanged** — same instrument names:
  `agent_tool_calls_total`, `agent_tool_call_duration_seconds`, `agent_ttft_seconds`,
  `agent_tokens_used_total`, `agent_llm_call_errors_total`.
- Swap the exporter: remove `opentelemetry-otlp::MetricExporter` (push), register an
  `opentelemetry-prometheus` exporter against the global meter provider.
- `vol-observability::build_metrics_router()` returns an axum `Router` exposing
  `GET /metrics` that gathers the Prometheus registry and text-encodes it.
- agent-server mounts this router on its existing HTTP port.
- **Fix concurrency contamination**: `MetricsState::llm_call_starts` is shared across all
  agents (global meter). Key TTFT correlation by `(agent_id, run_id, iteration)` instead
  of `(run_id, iteration)`, and remove entries by exact key match rather than blind
  `pop()` on complete/error.

### 5. Run-level metrics (new)

Add two run-scoped instruments, recorded on `AgentComplete` / `AgentAborted`:

- `agent_runs_total` (Counter) — labelled by `agent_id`, `agent_type`, `status`
  (`completed` / `aborted`).
- `agent_run_duration_seconds` (Histogram) — requires a run-start `Instant`. Capture it
  in `MetricsPlugin` on `AgentStart` keyed by `(agent_id, run_id)`, and measure on
  `AgentComplete` / `AgentAborted`.

Label cardinality note: `run_id` / `session_id` are **not** used as metric labels (high
cardinality). Only `agent_id`, `agent_type`, `tool_name`, `model`, `status` appear as
labels; `agent_id` is `def.name` (bounded), consistent with the logging plugin.

### 6. Emit LLMCall events in the agent loop (P0 fix)

In `vol-llm-agent/src/react/agent.rs` main loop:

- Before the LLM stream call: `emit(AgentStreamEvent::llm_call_start(iteration, messages))`.
- After `consume_llm_stream` returns `(.., model, usage)` (currently discarded):
  `emit(AgentStreamEvent::llm_call_complete(model, usage))`.
- On LLM request / stream failure: `emit(AgentStreamEvent::llm_call_error(error))` before
  the existing `agent_aborted`.

This activates `agent_ttft_seconds`, `agent_tokens_used_total`, and
`agent_llm_call_errors_total`, and makes `LLMCallComplete` / `LLMCallError` visible in the
Logging plugin's stdout stream.

## Data Flow (per agent run)

1. `run_input` starts → `AgentStart` emitted → MetricsPlugin records run-start `Instant`.
2. Each iteration: `LLMCallStart` → (thinking/content deltas) → `LLMCallComplete`;
   MetricsPlugin measures TTFT on first token, token usage on complete.
3. Tool calls: `ToolCallBegin` → `ToolCallComplete/Error/Skipped`; MetricsPlugin records
   count + duration.
4. `AgentComplete` / `AgentAborted` → MetricsPlugin records `agent_runs_total` +
   `agent_run_duration_seconds`, then cleans per-run state.
5. LoggingPlugin logs every non-delta event to stdout throughout.
6. RunLogPlugin (if registered) appends every non-delta event to the run's JSONL file.
7. Prometheus scrapes `/metrics` independently on its own schedule.

## Error Handling

- `/metrics` gather/encode failure → return HTTP 500 with an empty body; never panic the
  server (metrics are best-effort).
- Prometheus exporter registration failure at init → log an error and continue without
  metrics (agent execution must not depend on metrics availability).
- LLMCall event emission failures follow the existing fire-and-forget `emit()` semantics
  (broadcast send errors are ignored).

## Testing

- **LoggingPlugin**: unit test that a representative event produces a JSON line containing
  `run_id`, `session_id`, `agent_id`, and the flattened event fields; delta events skipped.
- **RunLogPlugin** (in vol-llm-agent): existing JSONL tests + new assertion that
  `session_id` is present in the written entry.
- **MetricsPlugin**: unit tests for `(agent_id, run_id, iteration)` keying — two agents
  with the same iteration number do not cross-contaminate TTFT; run-level counter/histogram
  recorded on complete and abort.
- **agent.rs**: extend `agent_run_tests.rs` to assert `LLMCallStart` and `LLMCallComplete`
  appear in the emitted event stream for a normal run, and `LLMCallError` on LLM failure.
- **/metrics endpoint**: integration test that `GET /metrics` returns 200 with Prometheus
  text format after a run records at least one metric.
- **Coverage ≥ 80%** on `vol-observability` (per CLAUDE.md), excluding trivial glue.

## Migration / Cleanup Impact

- Remove `vol-observability` from any k8s Deployment / ArgoCD manifest that ran it as a
  standalone service; drop its Dockerfile and image references.
- Remove TDengine `agent_run` / `llm_call` / `tool_call` table provisioning tied to the
  ingest pipeline.
- Add `prometheus.io/scrape` + `prometheus.io/path: /metrics` + port annotations to the
  agent-server Deployment (or the Prometheus scrape config) so the endpoint is collected.
- Update workspace `Cargo.toml` members to drop `vol-llm-observability` and the
  `vol-observability` binary target.
- `wiki-ingest` the result to `docs/wiki` per project convention.
