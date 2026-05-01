# Requirements: ReAct Agent Observability via OpenTelemetry

## Background

The ReAct Agent system (`vol-llm-agent`, `vol-llm-observability`) currently logs events to JSONL files via `LoggerPlugin`. This provides raw event data but lacks:

1. **Computed metrics** (TTFT, latency percentiles, token rates) — must be manually derived from event timestamps
2. **Standard observability primitives** — no histograms, counters, or trace spans for dashboards
3. **Cross-run aggregation** — JSONL is per-run, making it hard to answer "what's the p95 TTFT over the last hour?"

The monitoring system (`vol-monitor`, `vol-tracing`) already has OTEL 0.21 tracing configured. The LLM agent crates need equivalent OTEL integration for metrics, traces, and logs — configurable and following Rust best practices.

## Goals

1. **Debugging slow responses**: Capture LLM TTFT, per-LLM-call latency, per-tool latency, plugin intercept latency, context assembly time, and generation speed (tokens/sec). Each must be measurable as individual values and aggregatable as percentiles.

2. **Cost monitoring**: Track cumulative tokens per run (prompt, completion, cached, total), cache hit rate, token usage per iteration, and estimated cost per run based on configurable model pricing.

3. **Operational dashboarding**: Provide run success/failure rate, avg/max iterations per run, tool error rate by name, p50/p95/p99 TTFT and latency, runs per hour throughput, and abort reasons breakdown.

4. **Agent quality evaluation**: Track tool success rate, repeated tool call detection (same tool+args across iterations), thinking length vs answer length ratio, final answer presence, and iterations to answer.

## Non-Goals

- Building a UI dashboard (this is about data collection, not visualization)
- Replacing the existing JSONL logging — OTEL enriches and complements it
- Modifying the monitoring system's existing OTEL tracing (separate domain)
- Real-time alerting on metrics thresholds

## Scope

**Included:**

- OTEL Metrics: Histograms, counters, and gauges for all metrics listed above
- OTEL Traces: Span-based timing for agent runs, LLM calls, tool executions, and plugin hooks
- OTEL Logs: Enrich existing JSONL events with `trace_id`/`span_id` correlation
- Configuration: TOML-based config for enabling/disabling each signal type, OTLP endpoint, exporter selection, and sampling rate
- Integration: Wire OTEL into `vol-llm-agent`'s ReAct loop and `vol-llm-observability`'s plugin system

**Excluded:**

- Custom dashboard or visualization code
- Migration of existing monitoring system OTEL config
- Non-OTEL exporters (Prometheus, Jaeger SDK directly — only OTLP exporter)

## Constraints

- Use the existing `opentelemetry` 0.21 workspace dependency. Add `metrics` and `logs` features where needed.
- Must use standard Rust OTEL crates: `opentelemetry`, `opentelemetry_sdk`, `opentelemetry-otlp`, `tracing-opentelemetry`. No custom exporters.
- All three signals (metrics, traces, logs) must be independently configurable via TOML.
- OTEL initialization must not block agent startup. Failed OTEL exports must not crash the agent loop.
- The existing `LoggerPlugin` JSONL output continues to work unchanged, enriched with trace/span IDs.

## Configuration Design

New config section (extending or alongside existing `OpenTelemetryConfig`):

```toml
[llm_observability.otel]
enabled = true                        # master switch
signals = ["metrics", "traces", "logs"]  # which signals to enable
endpoint = "http://localhost:4317"    # OTLP gRPC endpoint
service_name = "vol-llm-agent"

[llm_observability.otel.metrics]
enabled = true
export_interval_millis = 10000        # push frequency

[llm_observability.otel.traces]
enabled = true
sample_rate = 1.0                     # 0.0-1.0

[llm_observability.otel.logs]
enabled = true                        # enrich JSONL logs with trace correlation
```

## Success Criteria

1. **TTFT measurable**: `LLMCallStart` to first `ContentStart`/`ThinkingStart` is captured as a histogram metric `llm.ttft.milliseconds`, visible via OTLP.

2. **Token usage per run**: A counter `llm.tokens.total` and `llm.tokens.prompt`, `llm.tokens.completion`, `llm.tokens.cached` accumulates across all LLM calls in a single run, with the run_id as a label.

3. **Tool latency measurable**: Histogram `llm.tool.execution_time.milliseconds` labeled by `tool_name` with p50/p95/p99 queryable.

4. **Trace spans visible**: A `vol-llm-agent` trace shows a span hierarchy: `agent.run` → `agent.iteration.N` → `llm.call` → `tool.execute`. Each span has attributes for iteration number, model, token usage, and tool name.

5. **Log correlation**: JSONL log entries contain `trace_id` and `span_id` fields, enabling cross-referencing between JSONL logs and OTEL trace viewer.

6. **Configurable**: Disabling `signals = ["metrics"]` results in no metrics being initialized; the agent still runs. Same for traces and logs independently.

7. **No regression**: Existing JSONL log format and LoggerPlugin behavior is unchanged (new fields are additive only).

## Edge Cases

1. **Non-streaming LLM calls**: If a future provider does not stream, TTFT = total latency. The metric should still be recorded with a `streaming: false` attribute.

2. **Aborted runs mid-LLM-call**: Emit a partial metric snapshot on abort. The span should be marked with error status and whatever data was collected.

3. **Long runs with many iterations**: Token usage counters should accumulate per iteration so the growth curve is visible. Use attributes like `iteration: N` on the counter.

4. **OTLP endpoint unreachable**: Export failures are logged as warnings; the agent loop continues. Metrics are buffered up to the SDK's queue limit then dropped (standard OTEL behavior).

5. **Repeated tool calls**: Detect when the same tool_name + arguments hash appears in multiple iterations. Record as an event/attribute `llm.tool.repeated` counter.

6. **Zero tool calls in a run**: The agent answered directly. This is a valid scenario — tool metrics simply don't fire.

## Open Questions

1. **Model pricing for cost estimates**: Should we hardcode prices for known models (e.g., `qwen3.5-plus`), or accept a configurable `cost_per_1m_input_tokens` / `cost_per_1m_output_tokens` in TOML? (Recommendation: configurable pricing table keyed by model name.)

2. **OTLP protocol**: Use gRPC (existing `grpc-tonic` feature) or HTTP protobuf? gRPC is already wired up in the workspace. (Recommendation: gRPC, matching existing setup.)
