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

### Trace ID in Logs

All log output now includes trace_id for correlation:

**Console:**
```
INFO [tr_0000018c9a62f3d0] vol_datasource::volatility: received ticker symbol=BTC
```

**File JSON:**
```json
{
  "trace_id": "0000018c9a62f3d00000000000000000",
  "span": {"name": "datasource_receive"},
  "fields": {"message": "received ticker", "symbol": "BTC"}
}
```

**Querying by trace_id:**
```bash
# Find all logs for a specific trace
grep 'tr_0000018c9a62f3d0' logs/vol-monitor.log

# Extract trace_id from JSON logs
grep -o '"trace_id":"[^"]*"' logs/vol-monitor.log | sort -u
```

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

## TracedEvent: 跨 Channel Trace 上下文传递

`TracedEvent<T>` 用于在 channel 间传递事件时保持 trace 上下文一致：

```rust
use vol_tracing::{new_trace_id, TracedEvent};
use tracing::info_span;

// Datasource 生成 trace_id
let trace_id = new_trace_id();
let span = info_span!("datasource_receive", trace_id = %trace_id);
let traced = TracedEvent::new(data, span, trace_id);
channel.send(traced).await?;

// Rule 提取 trace_id
let traced = channel.recv().await?;
let (data, parent_span, trace_id) = traced.split();
let span = info_span!("rule_evaluate", trace_id = %trace_id);
```

关键点：
- trace_id 在 datasource 入口生成
- 通过 TracedEvent 贯穿整个处理链
- 各层创建新 span 时注入相同 trace_id
- 业务数据 (Alert) 不携带 trace_id

### TracedEvent vs WithSpan

| Feature | WithSpan<T> | TracedEvent<T> |
|---------|-------------|----------------|
| Span propagation | 完整 span 上下文 | 仅 trace_id |
| Memory overhead | 较高 (保存完整 span) | 较低 (仅 16 字节 trace_id) |
| Use case | 短生命周期 channel | 跨模块/跨线程长生命周期 |
| Business data coupling | 否 | 否 |

`TracedEvent` 的优势：
1. **低内存开销**：不持有完整 span，仅传递 trace_id
2. **避免 span 生命周期问题**：适合事件队列、持久化场景
3. **业务数据纯净**：泛型 `T` 不依赖 tracing 类型

### 使用示例

```rust
// 1. DataSource 入口生成 trace_id
async fn run(self, mut tx: Sender<TracedEvent<VolatilityData>>) {
    loop {
        let ticker = self.client.recv().await?;
        let vol_data = ticker.to_volatility_data();
        
        // 生成 trace_id
        let trace_id = new_trace_id();
        let span = info_span!("datasource_receive", 
            trace_id = %trace_id,
            symbol = %vol_data.symbol,
            iv = %vol_data.iv
        );
        
        // 包装为 TracedEvent
        let traced = TracedEvent::new(vol_data, span, trace_id);
        tx.send(traced).await?;
    }
}

// 2. Rule Engine 提取 trace_id 创建新 span
async fn evaluate(&self, event: TracedEvent<VolatilityData>) {
    let (data, _parent_span, trace_id) = event.split();
    
    // 创建新 span，注入相同 trace_id
    let span = info_span!("rule_evaluate",
        trace_id = %trace_id,
        rule_id = %self.id,
        tenor = %self.tenor
    );
    
    // 在 span 内执行规则评估
    let _guard = span.enter();
    if let Some(alert) = self.check(&data) {
        // Alert 不携带 trace_id，由上层处理
        tx.send((alert, trace_id)).await?;
    }
}

// 3. Alert 处理时生成通知 span
async fn handle_alert(&self, alert: Alert, trace_id: TraceId) {
    let span = info_span!("alert_handle",
        trace_id = %trace_id,
        alert_type = %alert.alert_type,
        symbol = %alert.symbol
    );
    
    let _guard = span.enter();
    self.notify.send(&alert).await?;
}
```

### 完整数据流

```
Deribit WebSocket
       ↓
DataSource: new_trace_id() → TracedEvent::new(data, span, trace_id)
       ↓ (mpsc channel)
Rule Engine: event.split() → info_span!(trace_id = %trace_id)
       ↓
Alert + trace_id (业务数据与 trace 分离)
       ↓
Notification: info_span!(trace_id = %trace_id)
       ↓
Feishu: [tr_xxx] 消息前缀
```

## Recommended Patterns

### Using .instrument() for Async Operations

For async operations that cross `.await` points, use the `.instrument()` trait:

```rust
use vol_tracing::{new_trace_id, Instrument};
use tracing::info_span;

// Generate trace_id at entry point
let trace_id = new_trace_id();
let span = info_span!("datasource_receive",
    source = "deribit",
    trace_id = %trace_id,
    iv = %vol_data.iv,
    symbol = %vol_data.symbol,
);

// Use .instrument() for async operations
tx.send(event).instrument(span).await?;
```

### Extracting trace_id for Messages

```rust
use vol_tracing::current_trace_id;

// Extract trace_id for Feishu message prefix
let trace_id_prefix = format!("[tr_{}]", &current_trace_id()[..8]);
```

### Cross-Channel Span Propagation

```rust
use vol_tracing::WithSpan;

// Sender side
let span = info_span!("datasource_receive", ...);
let traced = WithSpan::new(event, span);
tx.send(traced).await?;

// Receiver side
let traced = rx.recv().await?;
traced.enter_span(info_span!("rule_evaluate"), |span| {
    span.follows_from(parent_span.id());
    // Process event
});
```

### Trace ID Format

Trace IDs follow OpenTelemetry standard: 128-bit (16 bytes) represented as 32 hex characters.

- **Format**: `tr_` + 32 hex chars (e.g., `tr_0000018c9a62f3d0a1b2c3d4e5f6g7h8`)
- **Generation**: Created at DataSource when ticker is received
- **Propagation**: Via span context through WithSpan wrapper
- **Jaeger compatibility**: Native 128-bit TraceId type

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
