# Tracing and Logging Architecture

## Overview

vol-monitor uses the `tracing` crate for structured logging and distributed tracing.
This document covers:
- Logging output (console + file)
- OpenTelemetry integration with Jaeger
- Span context propagation across channels
- Querying traces in Jaeger UI

## Architecture

### Data Flow with Tracing

```
Deribit WebSocket → DataSource (datasource_receive span)
                              ↓
                    [WithSpan wrapper carries span context]
                              ↓
                    Rule Engine (rule_evaluate span, follows_from DataSource)
                              ↓
                    Alert (alert_triggered span)
                              ↓
                    Notification (notification_send span)
                              ↓
                    Feishu message with [tr_xxx] prefix
```

### Key Components

1. **vol-tracing crate** (`crates/vol-tracing/`)
   - `WithSpan<T>` - Wrapper for cross-channel span propagation
   - `record_tags!` - Macro for injecting span attributes

2. **tracing_setup module** (`crates/vol-monitor/src/tracing_setup.rs`)
   - Console layer (compact format, colored)
   - File layer (JSON format, daily rolling, 7-day retention)
   - Error file layer (ERROR level only)
   - OpenTelemetry layer (OTLP gRPC to Jaeger)

## Configuration

### config.toml

```toml
[tracing]
[tracing.logging]
log_dir = "logs"
log_prefix = "vol-monitor"
retention_days = 7
json_format = true
console_level = "info"
file_level = "debug"
error_file = true

[tracing.opentelemetry]
enabled = true
endpoint = "http://localhost:4317"
service_name = "vol-monitor"
service_namespace = "deribit"
deployment_environment = "production"
sample_rate = 1.0

[tracing.opentelemetry.batch]
max_queue_size = 2048
max_batch_size = 512
scheduled_delay_millis = 5000
max_export_timeout_millis = 30000
```

### Environment Variables

| Variable | Purpose | Example |
|----------|---------|---------|
| `OTEL_ENDPOINT` | Override Jaeger endpoint | `http://jaeger:4317` |
| `RUST_LOG` | Set log level filter | `info` or `vol_monitor=debug` |

Priority: Environment variables > config.toml > defaults

## Log Output

### Console Output

Compact format with colors, suitable for development:
```
INFO vol_monitor::engine: rule evaluated crates/vol-engine/src/engine.rs:45
```

### File Output

JSON format in `logs/vol-monitor.log` (daily rolling):
```json
{"timestamp":"2026-04-05T12:00:00.000000Z","level":"INFO","target":"vol_monitor::engine","span":{"trace_id":"tr_abc1234567890"},"fields":{"message":"rule evaluated","rule_id":"absolute_iv_btc"}}
```

### Error File

Separate file for ERROR level only: `logs/vol-monitor.error.log`

## Distributed Tracing

### Span Context Propagation

vol-monitor uses `WithSpan<T>` wrapper to propagate span context across tokio mpsc channels:

```rust
// Sender side (DataSource)
let span = info_span!("datasource_receive");
record_tags!(span, data, iv, symbol, dte);
let traced = WithSpan::new(data, span);
tx.send(traced).await?;

// Receiver side (Rule Engine)
let traced = rx.recv().await?;
traced.enter_span("rule_evaluate", |span| {
    span.record("rule_id", &rule_id);
    // Rule evaluation happens inside span context
});
```

The `follows_from()` relationship establishes causal links between spans across channel boundaries.

### Trace ID Format

Trace IDs are generated as `tr_` + 16 hex characters: `tr_0000018c9a62f3d0`

## Jaeger UI

### Accessing Jaeger

Local development (Docker):
```bash
docker run --rm -d --name jaeger \
  -p 16686:16686 \  # UI
  -p 4317:4317 \  # OTLP gRPC
  jaegertracing/all-in-one:latest

# Open http://localhost:16686
```

Production (K8s):
```bash
kubectl port-forward -n observability svc/jaeger-query 16686:16686
```

### Querying Traces

1. **By Service Name**: Select `vol-monitor` to see all traces
2. **By Trace ID**: Enter `tr_abc1234` to find specific trace
3. **By Tags**: Filter by business attributes:
   - `market.symbol = BTC`
   - `alert.tenor = short`
   - `alert.iv > 0.7`

### Reverse Tracing from Feishu

Feishu messages include trace ID prefix: `[tr_abc1234] 🚨 BTC IV=0.72`

1. Copy the trace ID from Feishu message
2. Search in Jaeger UI: `tr_abc1234`
3. View complete waterfall of spans from DataSource → Rule → Notification

## Testing

### Unit Tests

```bash
cargo test -p vol-config tracing  # Test config parsing
cargo test -p vol-tracing         # Test WithSpan wrapper
```

### Integration Test: Span Propagation

```bash
# Start Jaeger
docker run --rm -d --name jaeger -p 4317:4317 -p 16686:16686 jaegertracing/all-in-one:latest

# Run vol-monitor (generate some traces)
./target/release/vol-monitor

# Check Jaeger UI at http://localhost:16686
# Search for service: vol-monitor
```

## Troubleshooting

### Traces not appearing in Jaeger

1. Check `OTEL_ENDPOINT` environment variable
2. Verify Jaeger service is reachable
3. Check logs for OTLP export errors
4. Verify `sample_rate` is not too low

### Log files growing too large

1. Check `retention_days` config (default: 7)
2. Ensure log directory has cleanup script or systemd timer

### Span context lost across channel

1. Verify `WithSpan` wrapper is used when sending
2. Verify `enter_span()` is called on receiver
3. Check span names in Jaeger show `follows_from` relationship
