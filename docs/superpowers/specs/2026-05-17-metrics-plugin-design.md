# Design: Metrics Plugin + ObservabilityPlugin Cleanup

## Summary

Add a `MetricsPlugin` to the `vol-llm-observability` crate that implements the `AgentPlugin` trait, recording OTel metrics by listening to `AgentStreamEvent`s. Simultaneously remove the unused `ObservabilityPlugin` code (`agent_plugin.rs`, `agent_client.rs`, `agent_config.rs`) which has zero external consumers.

## Cleanup

### Files to delete
- `crates/vol-llm-observability/src/agent_plugin.rs`
- `crates/vol-llm-observability/src/agent_client.rs`
- `crates/vol-llm-observability/src/agent_config.rs`

### Files to modify
- `crates/vol-llm-observability/src/lib.rs` — remove `pub mod` and `pub use` for deleted modules
- `crates/vol-llm-observability/Cargo.toml` — remove `reqwest` dependency (only used by `agent_client`)

## Architecture

```
┌─────────────────────────────────────┐
│  ReAct Agent                        │
│  ┌──────────┐                       │
│  │ Plugin   │                       │
│  │ Registry │                       │
│  │          │  listen() calls       │
│  │  Logger  │◄──┐                   │
│  │  Loki    │   │  parallel          │
│  │  Metrics │◄──┘                   │
│  └──────────┘                       │
└─────────────────────────────────────┘
         │
         │ AgentStreamEvent
         ▼
┌─────────────────────────────────────┐
│  MetricsPlugin                      │
│  ┌───────────────────────────────┐  │
│  │  State:                       │  │
│  │  - llm_call_starts: HashMap   │  │
│  │    (correlation_id → Instant) │  │
│  │  - ttft_measured: HashSet     │  │
│  └───────────────────────────────┘  │
│  ┌───────────────────────────────┐  │
│  │  OTel Meter (global):         │  │
│  │  - tool_calls_total (Counter) │  │
│  │  - tool_call_duration (Histo) │  │
│  │  - tool_call_success (Counter)│  │
│  │  - ttft_seconds (Histo)       │  │
│  │  - tokens_used (Counter)      │  │
│  └───────────────────────────────┘  │
└─────────────────────────────────────┘
         │
         │ OTel SDK export
         ▼
   OTel Collector / Prometheus
```

State is held via `Arc<Mutex<MetricsState>>`. Correlation key for TTFT is `(run_id, iteration)`.

## Data Flow & Event → Metric Mapping

```
LLMCallStart {iteration}
  → record start_time[(run_id, iteration)] = Instant::now()

ThinkingStart | ContentStart (whichever first — race, first wins)
  → if start_time exists:
       ttft = now() - start_time
       ttft_seconds.record(ttft, labels)

LLMCallComplete {model, usage}
  → tokens_used{model, type="input"}.inc(usage.input_tokens)
  → tokens_used{model, type="output"}.inc(usage.output_tokens)
  → tokens_used{model, type="total"}.inc(usage.total_tokens)
  → cleanup start_time entry

LLMCallError {error}
  → agent_llm_call_errors_total{model, agent_id}.inc()
  → cleanup start_time entry

ToolCallBegin {tool_name}
  → record begin_time[tool_name] = Instant::now()

ToolCallComplete {tool_name, duration_ms}
  → agent_tool_calls_total{tool_name, status="success"}.inc()
  → agent_tool_call_duration{tool_name}.record(duration_ms)

ToolCallError {tool_name, duration_ms}
  → agent_tool_calls_total{tool_name, status="error"}.inc()
  → agent_tool_call_duration{tool_name}.record(duration_ms)

ToolCallSkipped {tool_name, reason}
  → agent_tool_calls_total{tool_name, status="skipped"}.inc()

AgentComplete / AgentAborted
  → cleanup all remaining state
```

## Metrics Registry (OTel SDK)

| Metric Name | Type | Labels | Description |
|---|---|---|---|
| `agent_tool_calls_total` | Counter | `tool_name`, `status`, `agent_id`, `agent_type` | Total tool call attempts |
| `agent_tool_call_duration_seconds` | Histogram | `tool_name`, `agent_id` | Tool call execution latency |
| `agent_ttft_seconds` | Histogram | `model`, `agent_id` | Time to first token (thinking or content, whichever first) |
| `agent_tokens_used_total` | Counter | `model`, `token_type`, `agent_id` | Token usage (input/output/total) |
| `agent_llm_call_errors_total` | Counter | `model`, `agent_id` | LLM call errors |

## Implementation Details

### MetricsPlugin Structure

```rust
pub struct MetricsPlugin {
    state: Arc<Mutex<MetricsState>>,
    metrics: Arc<Metrics>,
}

struct MetricsState {
    llm_call_starts: HashMap<(String, u32), Instant>,
    tool_call_starts: HashMap<String, Instant>,
}
```

- `MetricsPlugin::new()` initializes OTel meter and creates all instruments
- `listen()` matches event type, updates state, records metrics
- `intercept()` always returns `PluginDecision::Continue`
- Uses `Instant` for timestamping (higher precision than `chrono` for TTFT)
- Uses `opentelemetry::global::meter()` to get global meter (reuses existing OTel init)

### Error Handling

- OTel SDK instrument record operations are fire-and-forget (no error return)
- State cleanup uses `HashMap::remove`, silently handles missing keys (race condition safe)
- No error propagation to the agent — plugin failures must not affect agent execution
