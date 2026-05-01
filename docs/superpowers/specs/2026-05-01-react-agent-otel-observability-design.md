# Design: ReAct Agent Observability via OpenTelemetry

**Created**: 2026-05-01
**Status**: Draft
**Based on**: `docs/superpowers/requirement/2026-05-01-react-agent-observability-requirement.md`

## Overview

Extend `vol-llm-observability` to provide OpenTelemetry metrics, traces, and log correlation for the ReAct Agent system. Metrics are recorded directly in `vol-llm-agent` via the OTEL global meter; OTLP export is configured and initialized by vol-llm-observability. When OTEL is disabled, global meter/tracer return no-op providers with zero overhead.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        ReActAgent (vol-llm-agent)               │
│  - Directly calls global::meter() for metrics                   │
│  - Directly calls global::tracer() for spans                    │
│  - Emits 3 new AgentStreamEvent variants for observability      │
└────────────────────────┬────────────────────────────────────────┘
                         │ AgentStreamEvent (broadcast channel)
          ┌──────────────┴──────────────┐
          ▼                             ▼
┌───────────────────┐       ┌─────────────────────────┐
│   LoggerPlugin    │       │      OtelPlugin         │
│  (existing)       │       │  (new)                  │
│  JSONL + stdout   │       │  - OTEL Log enrichment  │
│  + trace_id field │       │  - Metrics aggregation  │
│                   │       │  - Run-level summaries  │
└───────────────────┘       └───────────┬─────────────┘
                                        │
                              ┌─────────▼──────────┐
                              │   OTLP Exporter     │
                              │   (gRPC)           │
                              └────────────────────┘
```

**Key principles:**
- vol-llm-agent directly depends on `opentelemetry`, records metrics via `global::meter()` / `global::tracer()`
- vol-llm-observability owns OTLP SDK initialization, OtelPlugin, and config parsing
- When OTEL is not initialized, global providers are no-op — zero overhead, zero branching

## New Event Types

Three new `AgentStreamEvent` variants in `vol-llm-core/src/stream.rs`:

```rust
LLMFirstTokenReceived {
    timestamp: DateTime<Utc>,
    iteration: u32,
    token_type: LLMTokenType, // Thinking or Content
}

ContextAssemblyComplete {
    timestamp: DateTime<Utc>,
    iteration: u32,
    duration_ms: u64,
    message_count: usize,
}

PluginInterceptComplete {
    timestamp: DateTime<Utc>,
    plugin_id: String,
    event_name: String,
    duration_ms: u64,
    decision: String, // Continue, Skip, Abort
}
```

**Where they fire in agent.rs:**
- `ContextAssemblyComplete`: after `run_ctx.get_context()` returns
- `LLMFirstTokenReceived`: in `consume_llm_stream()`, on the first `ContentDelta` or `ThinkingDelta`
- `PluginInterceptComplete`: after each `run_ctx.intercept()` returns (measures plugin overhead in `plugin_stream.rs`)

## Metric Instruments

All instruments are registered on `init_otel()` and accessed via `global::meter("vol-llm-agent")`.

### Histograms

| Name | Unit | Labels | Recorded at |
|------|------|--------|-------------|
| `llm.ttft.milliseconds` | ms | `model`, `agent_id` | First token from stream |
| `llm.call.duration.milliseconds` | ms | `model`, `agent_id`, `iteration` | LLMCallComplete |
| `llm.tool.execution_time.milliseconds` | ms | `tool_name`, `agent_id`, `success` | ToolCallComplete/Error |
| `llm.context.assembly_time.milliseconds` | ms | `agent_id`, `iteration` | ContextAssemblyComplete event |
| `llm.generation.tokens_per_second` | tokens/sec | `model`, `agent_id` | Derived: completion_tokens / (call_duration - ttft) |
| `llm.plugin.intercept_duration.milliseconds` | ms | `plugin_id`, `event_name` | PluginInterceptComplete event |

### Counters

| Name | Unit | Labels | Recorded at |
|------|------|--------|-------------|
| `llm.tokens.prompt` | tokens | `run_id`, `agent_id` | LLMCallComplete usage |
| `llm.tokens.completion` | tokens | `run_id`, `agent_id` | LLMCallComplete usage |
| `llm.tokens.cached` | tokens | `run_id`, `agent_id` | LLMCallComplete usage |
| `llm.tokens.total` | tokens | `run_id`, `agent_id` | Sum of above |
| `llm.agent.runs_total` | count | `agent_id` | AgentStart |
| `llm.agent.completed_total` | count | `agent_id` | AgentComplete |
| `llm.agent.errors_total` | count | `agent_id`, `reason` | AgentAborted, LLM error |
| `llm.tool.calls_total` | count | `agent_id`, `tool_name`, `status` | ToolCallComplete/Error/Skip |
| `llm.tool.repeated_calls` | count | `agent_id`, `tool_name` | Same tool+args hash across iterations |

### Gauges

| Name | Unit | Labels | Recorded at |
|------|------|--------|-------------|
| `llm.agent.current_iteration` | count | `run_id`, `agent_id` | Each iteration start |
| `llm.cache.hit_ratio` | ratio | `model`, `agent_id` | cached_tokens / prompt_tokens per call |

### Trace Spans

| Span | Parent | Attributes |
|------|--------|------------|
| `agent.run` | - | `agent_id`, `run_id`, `session_id`, `input_preview` |
| `agent.iteration` | `agent.run` | `iteration`, `tool_calls_count`, `has_final_answer` |
| `llm.call` | `agent.iteration` | `model`, `input_tokens`, `output_tokens`, `ttft_ms` |
| `tool.execute` | `agent.iteration` | `tool_name`, `success`, `error_type`, `duration_ms` |

## Configuration

Add to `vol-config/src/tracing.rs`:

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentObservabilityConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_signals")]
    pub signals: Vec<String>, // "metrics", "traces", "logs"
    #[serde(default)]
    pub metrics: AgentMetricsConfig,
    #[serde(default)]
    pub traces: AgentTracesConfig,
    #[serde(default = "default_true")]
    pub logs: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentMetricsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_export_interval")]
    pub export_interval_millis: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentTracesConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_sample_rate")]
    pub sample_rate: f64,
}
```

Example TOML:

```toml
[tracing.agent_observability]
enabled = true
signals = ["metrics", "traces", "logs"]

[tracing.agent_observability.metrics]
enabled = true
export_interval_millis = 10000

[tracing.agent_observability.traces]
enabled = true
sample_rate = 1.0
```

## Initialization

In `vol-llm-observability/src/otel/mod.rs`:

```rust
pub fn init_otel(config: &AgentObservabilityConfig) -> OtelHandles {
    let mut meter_provider = None;
    let mut tracer_provider = None;

    if config.signals.contains(&"metrics".to_string()) {
        let mp = build_meter_provider(config);
        global::set_meter_provider(mp.clone());
        meter_provider = Some(mp);
    }

    if config.signals.contains(&"traces".to_string()) {
        let tp = build_tracer_provider(config);
        global::set_tracer_provider(tp.clone());
        tracer_provider = Some(tp);
    }

    OtelHandles { meter_provider, tracer_provider }
}
```

Agent code uses `global::meter("vol-llm-agent")` — no-op when no provider is set.

## Error Handling

- OTEL init failure → `tracing::warn!`, agent continues normally
- Global meter returns no-op provider when not initialized — no branching needed in agent code
- OTLP export failures handled by SDK (buffer → drop when queue full)
- Shutdown: `shutdown_otel()` called in `tracing_setup::shutdown()` alongside existing `global::shutdown_tracer_provider()`

## JSONL Log Enrichment

When `signals` includes `"logs"`:
- `LoggerPlugin::create_log_entry()` adds `trace_id` and `span_id` to the data object
- Values read from `tracing::Span::current().context()` via `tracing-opentelemetry`
- Existing JSONL format unchanged — new fields are additive only

## Testing

- Unit: verify each metric recorded with correct labels using in-memory SDK assertions
- Integration: run agent with mock LLM, assert metrics are recorded
- Disabled path: agent runs normally with OTEL disabled, no panics
- Regression: JSONL format unchanged, LoggerPlugin behavior identical
