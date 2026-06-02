# Trace ID Injection into Logs Design

## Overview

Inject trace_id into all log output (console + file) to enable correlation of related logs across the entire data processing pipeline.

## Current State

- Logs do not contain trace_id, making it impossible to correlate with Jaeger traces
- trace_id uses custom `u64` format, not OpenTelemetry standard
- Each processing stage creates new span but does not inherit trace_id from upstream

## Desired State

- **Console**: `INFO [tr_0000018c9a62f3d0] vol_monitor::datasource: received ticker symbol=BTC`
- **File JSON**: `{"trace_id":"0000018c9a62f3d0000000000000000","span":{"name":"datasource_receive"},"fields":{...}}`
- All stages of a market data update (datasource → engine → rule → alert → notification) share the same trace_id

## Architecture

### Trace Model

One market data update = One trace

```
Trace: tr_0000018c9a62f3d0 (one ticker update)
│
├─ Span 1: datasource_receive (root span, in vol-datasource/volatility.rs)
│   └─ Generate trace_id at ticker receipt
│   └─ Record: symbol, instrument_name, iv, dte, index_price
│
├─ Span 2: datasource_event (in vol-engine/engine.rs, follows_from Span 1)
│   └─ Inherit trace_id from parent
│   └─ Record: source, event_type
│
├─ Span 3: rule_evaluate (in vol-engine/engine.rs, follows_from Span 2)
│   └─ Inherit trace_id from parent
│   └─ Record: rule_id, rule_type, tenor
│
├─ Span 4: alert_triggered (conditional, in vol-alert)
│   └─ Inherit trace_id from parent
│   └─ Record: alert_type, threshold, actual_value
│
└─ Span 5: notification_send (in vol-notification)
    └─ Inherit trace_id from parent
    └─ Record: notification_type, recipient, status
```

### Key Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| trace_id generation point | DataSource (volatility.rs) | Earliest point of contact with market data |
| trace_id format | OpenTelemetry TraceId (128-bit) | Native Jaeger compatibility |
| span relationship | `follows_from()` | Causal relationship across channel boundaries |
| trace_id propagation | Via `WithSpan<T>` carrying span context | Reuse existing mechanism |

## Implementation Details

### Dependencies

Add `opentelemetry` to affected crates:

```toml
# crates/vol-datasource/Cargo.toml
# crates/vol-engine/Cargo.toml
[dependencies]
opentelemetry = "0.21"  # Align with existing version
```

### File Changes

| File | Changes | Description |
|------|---------|-------------|
| `crates/vol-datasource/src/volatility.rs` | ~30 lines | Create root span at ticker handling, record business attributes |
| `crates/vol-engine/src/engine.rs` | ~50 lines | Inherit span context, create follows_from relationships |
| `crates/vol-alert/src/**/*.rs` | ~20 lines | Record span attributes in alert handlers |
| `crates/vol-notification/src/feishu.rs` | ~20 lines | Output trace_id in messages |
| `crates/vol-notification/src/stdout.rs` | ~10 lines | Output trace_id in messages |
| `crates/vol-monitor/src/tracing_setup.rs` | ~40 lines | Configure tracing-opentelemetry layer for log injection |

### Trace ID Generation

Use OpenTelemetry standard `TraceId` type:

```rust
use opentelemetry::trace::TraceId;

// Option A: Random generation
fn generate_trace_id() -> TraceId {
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes).expect("Failed to generate random bytes");
    TraceId::from_bytes(bytes)
}

// Option B: Let tracing-opentelemetry auto-manage
// Create span and extract trace_id from current context
let span = info_span!("datasource_receive", source = "deribit");
let _guard = span.enter();
let trace_id = tracing::Span::current()
    .context()
    .span()
    .span_context()
    .trace_id();
```

### Log Format Configuration

**Console layer** (modified):
```rust
fmt::layer()
    .with_target(true)
    .with_thread_ids(false)
    .with_file(true)
    .with_line_number(true)
    .with_span_events(FmtSpan::NEW)
```

**File layer** (modified):
```rust
fmt::layer()
    .json()
    .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
    .with_writer(file_appender)
```

## Log Format Examples

### Console Output (After)

```
2026-04-05T12:00:00.000Z INFO [tr_0000018c9a62f3d0] vol_datasource::volatility: received ticker crates/vol-datasource/src/volatility.rs:198 symbol=BTC instrument_name=BTC-5APR24-70000-C iv=0.65 dte=5
2026-04-05T12:00:00.001Z INFO [tr_0000018c9a62f3d0] vol_engine::engine: rule evaluated crates/vol-engine/src/engine.rs:163 rule_id=absolute-iv-btc rule_type=absolute-iv tenor=short
2026-04-05T12:00:00.002Z INFO [tr_0000018c9a62f3d0] vol_alert::absolute_iv: alert triggered crates/vol-alert/src/absolute_iv.rs:45 alert_type=absolute-iv symbol=BTC threshold=0.55 actual=0.65
2026-04-05T12:00:00.003Z INFO [tr_0000018c9a62f3d0] vol_notification::feishu: notification sent crates/vol-notification/src/feishu.rs:301 trace_id=tr_0000018c9a62f3d0 recipient=oc_example_chat_id
```

### File JSON Output (After)

```json
{
  "timestamp": "2026-04-05T12:00:00.000000Z",
  "level": "INFO",
  "target": "vol_datasource::volatility",
  "trace_id": "0000018c9a62f3d00000000000000000",
  "span": {
    "name": "datasource_receive",
    "id": "12345"
  },
  "fields": {
    "message": "received ticker",
    "symbol": "BTC",
    "instrument_name": "BTC-5APR24-70000-C",
    "iv": 0.65,
    "dte": 5,
    "source": "deribit"
  }
}
```

### Feishu Message (Unchanged)

```
[tr_0000018c9a62f3d0] 🚨 BTC Short IV Alert: 0.65 > 0.55
```

## Error Handling

| Scenario | Handling |
|----------|----------|
| OpenTelemetry exporter failure | Degrade to logging-only mode, trace_id still recorded but not exported |
| trace_id generation failure | Use fallback value (all zeros), log warning |
| span context lost | Record `parent_trace_id` field if available |
| Jaeger unreachable | OTel exporter caches/drops, does not block main flow |

## Testing

### Unit Tests

```rust
#[test]
fn test_trace_id_recorded() {
    let span = info_span!("test_span");
    let _guard = span.enter();
    let ctx = tracing::Span::current().context();
    assert!(ctx.span().span_context().trace_id().to_string().len() > 0);
}
```

### Integration Test

```bash
# 1. Start Jaeger
docker run --rm -d --name jaeger -p 4317:4317 -p 16686:16686 jaegertracing/all-in-one:latest

# 2. Run vol-monitor to generate traces
./target/release/vol-monitor --config config.toml

# 3. Verify logs contain trace_id
grep -o '"trace_id":"[^"]*"' logs/vol-monitor.log | head -3

# 4. Verify trace in Jaeger UI at http://localhost:16686
```

### Acceptance Criteria

- [ ] Every log line (console + file) contains trace_id field
- [ ] All logs from same data update share the same trace_id
- [ ] Complete span chain visible in Jaeger
- [ ] trace_id in Feishu messages is queryable in Jaeger

## Trade-offs

### One Ticker = One Trace

**Pros:**
- Complete visibility into data flow
- Can trace why an alert did NOT trigger
- Debugging data pipeline issues easier

**Cons:**
- High volume of traces (BTC/ETH tickers arrive multiple times per second)
- Increased storage/processing requirements for Jaeger

### Alternative Considered: One Alert = One Trace

**Pros:** Fewer traces, focused on important events
**Cons:** Cannot trace "no alert" scenarios, less visibility

We chose "one ticker = one trace" for full observability. Sampling can be used to reduce trace volume in production if needed.
