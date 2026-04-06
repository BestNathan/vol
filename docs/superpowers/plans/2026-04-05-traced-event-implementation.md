# TracedEvent 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将现有 WithSpan 重命名为 TracedEvent，移除 Alert.trace_id 字段，实现 traceId 通过 TracedEvent 贯穿 datasource → engine → notification 链路

**Architecture:** TracedEvent<T>包装类型封装 value + parent_span + trace_id，在 channel 间传递时保持 trace 上下文一致，业务数据 Alert 不携带 trace_id

**Tech Stack:** Rust, tracing crate, tokio channels

---

## 1. TracedEvent 类型定义

**Files:**
- Create: `crates/vol-tracing/src/traced_event.rs`
- Modify: `crates/vol-tracing/src/lib.rs`

- [ ] **Step 1: 创建 traced_event.rs 文件**

```rust
//! TracedEvent - wrapper for propagating trace context across channel boundaries.
//!
//! Unlike WithSpan which only carries a span, TracedEvent explicitly stores
//! the trace_id to ensure consistent tracing across async boundaries.

use tracing::Span;

/// Wrapper for sending events across channel boundaries with explicit trace context.
///
/// # Type Parameters
/// * `T` - The wrapped event type (e.g., VolatilityData, MonitoringEvent, Alert)
#[derive(Clone)]
pub struct TracedEvent<T> {
    /// The wrapped event data
    value: T,
    /// Parent span for establishing causal relationships
    parent_span: Option<Span>,
    /// Explicit trace ID for distributed tracing
    trace_id: String,
}

impl<T> TracedEvent<T> {
    /// Create a new TracedEvent with the given value, span, and trace_id.
    ///
    /// # Arguments
    /// * `value` - The event data to wrap
    /// * `span` - The current span for context propagation
    /// * `trace_id` - The trace ID for this event chain
    pub fn new(value: T, span: Span, trace_id: String) -> Self {
        Self {
            value,
            parent_span: Some(span),
            trace_id,
        }
    }

    /// Create a TracedEvent without a parent span (generates new trace_id).
    ///
    /// Use this when starting a new trace chain.
    pub fn without_span(value: T) -> Self {
        Self {
            value,
            parent_span: None,
            trace_id: crate::new_trace_id(),
        }
    }

    /// Create a TracedEvent with explicit trace_id (for continuing a trace).
    pub fn with_trace_id(value: T, span: Option<Span>, trace_id: String) -> Self {
        Self {
            value,
            parent_span: span,
            trace_id,
        }
    }

    /// Split the wrapper to get the value, optional span, and trace_id.
    pub fn split(self) -> (T, Option<Span>, String) {
        (self.value, self.parent_span, self.trace_id)
    }

    /// Get a reference to the trace_id.
    pub fn trace_id(&self) -> &str {
        &self.trace_id
    }

    /// Get a reference to the wrapped value.
    pub fn value(&self) -> &T {
        &self.value
    }

    /// Unwrap and return the value, consuming the wrapper.
    pub fn into_value(self) -> T {
        self.value
    }
}
```

- [ ] **Step 2: 在 lib.rs 中导出 TracedEvent**

修改 `crates/vol-tracing/src/lib.rs`:

```rust
mod macros;
mod with_span;
mod traced_event;

use tracing::Span;
pub use with_span::WithSpan;
pub use traced_event::TracedEvent;
// Re-export tracing core types for downstream crates
pub use tracing::instrument;
pub use tracing::Instrument;
// macros are exported via #[macro_export] automatically

/// Generate a new trace_id (UUID v4, hyphenated format)
///
/// # Example
/// ```
/// let trace_id = vol_tracing::new_trace_id();
/// assert_eq!(trace_id.len(), 36); // 8-4-4-4-12 format
/// ```
pub fn new_trace_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub fn current_trace_id() -> String {
    Span::current()
        .field("trace_id")
        .unwrap_or_else(new_trace_id)
        .to_string()
}
```

- [ ] **Step 3: 验证编译**

```bash
cargo check -p vol-tracing
```

Expected: Compiles successfully with no errors

- [ ] **Step 4: 提交**

```bash
git add crates/vol-tracing/src/traced_event.rs crates/vol-tracing/src/lib.rs
git commit -m "feat(tracing): add TracedEvent wrapper for trace context propagation"
```

## 2. 移除 Alert.trace_id 字段

**Files:**
- Modify: `crates/vol-core/src/alert.rs:84-86`
- Modify: `crates/vol-core/src/alert.rs:109-139`
- Test: Existing tests in alert.rs

- [ ] **Step 1: 移除 Alert 结构体的 trace_id 字段**

修改 `crates/vol-core/src/alert.rs` 第 84-86 行：

```rust
/// Option mark price in coin units (e.g., 0.05 BTC or 0.5 ETH)
pub mark_price_coin: f64,
// trace_id 字段已移除 - 现在通过 TracedEvent<Alert> 传递
```

删除：
```rust
/// Trace ID for distributed tracing across the pipeline
pub trace_id: String,
```

- [ ] **Step 2: 更新 Alert::new() 构造函数**

修改 `crates/vol-core/src/alert.rs` 第 109-139 行：

```rust
pub fn new(
    alert_type: AlertType,
    tenor: Tenor,
    symbol: String,
    iv: f64,
    message: String,
    timestamp: u64,
    source: String,
    index_price: f64,
    dte: u32,
    option_type: OptionType,
    moneyness: f64,
    mark_price_coin: f64,
) -> Self {
    Self {
        alert_type,
        tenor,
        symbol,
        iv,
        message,
        timestamp,
        source,
        index_price,
        dte,
        option_type,
        moneyness,
        mark_price_coin,
    }
}
```

移除参数列表中的 `trace_id: String` 和结构体初始化中的 `trace_id`。

- [ ] **Step 3: 验证编译**

```bash
cargo check -p vol-core
```

Expected: Compilation errors showing Alert::new() calls that still pass trace_id parameter

- [ ] **Step 4: 提交**

```bash
git add crates/vol-core/src/alert.rs
git commit -m "refactor(core): remove trace_id field from Alert struct"
```

## 3. 更新 Alert 创建位置（移除 trace_id 参数）

**Files:**
- Modify: `crates/vol-alert/src/absolute_iv.rs:65-83`
- Modify: `crates/vol-alert/src/rate_change.rs:71-88`
- Modify: `crates/vol-alert/src/rate_change.rs:95-113`
- Modify: `crates/vol-alert/src/rate_change.rs:119-137`

- [ ] **Step 1: 更新 absolute_iv.rs 中的 Alert::new()**

修改 `crates/vol-alert/src/absolute_iv.rs` 第 65-83 行，移除最后 `trace_id` 参数：

```rust
let alert = Alert::new(
    AlertType::AbsoluteIv { threshold: iv_threshold },
    tenor,
    data.symbol.clone(),
    data.iv,
    format!(
        "{} {} IV {:.1}% (symbol: {}, moneyness: {:.2}%, ATM: {:.1}%) >= threshold {:.1}%",
        data.symbol, tenor,
        data.iv * 100.0, symbol_name, moneyness * 100.0, atm_threshold * 100.0, iv_threshold * 100.0
    ),
    data.timestamp,
    data.source.clone(),
    data.index_price,
    data.dte,
    data.option_type,
    moneyness,
    mark_price,
    // 移除了 trace_id 参数
);
```

同时删除第 84-91 行的 trace_id 提取代码：
```rust
// 删除以下代码：
// let current_span = tracing::Span::current();
// let trace_id = current_span
//     .context()
//     .span()
//     .span_context()
//     .trace_id();
// current_span.record("alert.trace_id", &trace_id.to_string());
```

- [ ] **Step 2: 更新 rate_change.rs 中的三个 Alert::new() 调用**

修改 `crates/vol-alert/src/rate_change.rs` 第 71-88 行、95-113 行、119-137 行，每个调用都移除最后的参数：

```rust
// 1h window (line 71-88)
return Some(Alert::new(
    AlertType::RateChange { window_hours: 1, change_pct: change },
    tenor,
    data.symbol.clone(),
    data.iv,
    format!(
        "{} {} IV changed {:.1}% in 1h (threshold: {:.1}%)",
        data.symbol, tenor,
        change * 100.0, self.config.window_1h_threshold * 100.0
    ),
    data.timestamp,
    data.source.clone(),
    data.index_price,
    data.dte,
    data.option_type,
    data.moneyness(),
    data.extra.get("mark_price_coin").and_then(|v| v.as_f64()).unwrap_or(0.0),
));

// 4h window (line 95-113) - same pattern
// 24h window (line 119-137) - same pattern
```

- [ ] **Step 3: 验证编译**

```bash
cargo check -p vol-alert
```

Expected: Compiles successfully

- [ ] **Step 4: 提交**

```bash
git add crates/vol-alert/src/absolute_iv.rs crates/vol-alert/src/rate_change.rs
git commit -m "refactor(alert): remove trace_id parameter from Alert::new() calls"
```

## 4. vol-datasource 使用 TracedEvent

**Files:**
- Modify: `crates/vol-datasource/src/volatility.rs:1-12`
- Modify: `crates/vol-datasource/src/volatility.rs:180-201`
- Modify: `crates/vol-datasource/src/volatility.rs:215-224`
- Test: `crates/vol-datasource/src/volatility.rs:275-306`

- [ ] **Step 1: 更新导入语句**

修改 `crates/vol-datasource/src/volatility.rs` 第 12 行：

```rust
use vol_tracing::{TracedEvent, new_trace_id, Instrument};
```

- [ ] **Step 2: 更新内部 channel 类型**

修改 `crates/vol-datasource/src/volatility.rs` 第 76 行：

```rust
let (internal_tx, mut internal_rx) = mpsc::channel::<TracedEvent<VolatilityData>>(1024);
```

- [ ] **Step 3: 更新数据接收处创建 TracedEvent**

修改 `crates/vol-datasource/src/volatility.rs` 第 180-201 行：

```rust
if let Some(vol_data) = option.to_volatility_data_with_index(index_price) {
    let trace_id = new_trace_id();
    let span = info_span!(
        "datasource_receive",
        source = "deribit",
        trace_id = %trace_id,
        iv = %vol_data.iv,
        symbol = %vol_data.symbol,
        dte = vol_data.dte,
        index_price = %vol_data.index_price,
        option_type = %vol_data.option_type,
    );

    let traced_event = TracedEvent::new(vol_data, span.clone(), trace_id);
    if let Err(e) = internal_tx.send(traced_event).instrument(span).await {
        error!(
            instrument = %option.instrument_name,
            error = %e,
            "Failed to send volatility data"
        );
    }
}
```

- [ ] **Step 4: 更新转发逻辑**

修改 `crates/vol-datasource/src/volatility.rs` 第 215-224 行：

```rust
while let Some(traced_vol_data) = internal_rx.recv().await {
    // 提取 trace_id 用于转发包装
    let trace_id = traced_vol_data.trace_id().to_string();
    let event = traced_vol_data.into_value();
    
    let monitoring_event = MonitoringEvent::Volatility(event);
    let forward_span = info_span!(
        "datasource_forward",
        trace_id = %trace_id,
        event_type = "volatility"
    );
    let traced_monitoring = TracedEvent::new(monitoring_event, forward_span, trace_id);
    
    if let Err(e) = tx.send(traced_monitoring).await {
        error!("Failed to send event: {}", e);
        break;
    }
}
```

- [ ] **Step 5: 验证编译**

```bash
cargo check -p vol-datasource
```

- [ ] **Step 6: 提交**

```bash
git add crates/vol-datasource/src/volatility.rs
git commit -m "feat(datasource): use TracedEvent for trace context propagation"
```

## 5. vol-engine 使用 TracedEvent

**Files:**
- Modify: `crates/vol-engine/src/engine.rs:1-10`
- Modify: `crates/vol-engine/src/engine.rs:58`
- Modify: `crates/vol-engine/src/engine.rs:95-134`
- Modify: `crates/vol-engine/src/engine.rs:137-204`
- Modify: `crates/vol-engine/src/engine.rs:206-243`

- [ ] **Step 1: 更新导入语句**

修改 `crates/vol-engine/src/engine.rs` 第 9 行：

```rust
use vol_tracing::{TracedEvent, Instrument};
```

- [ ] **Step 2: 更新事件 channel 类型**

修改 `crates/vol-engine/src/engine.rs` 第 58 行：

```rust
let (event_tx, _) = broadcast::channel::<TracedEvent<MonitoringEvent>>(self.config.event_buffer_size);
```

- [ ] **Step 3: 更新 alert channel 类型**

修改 `crates/vol-engine/src/engine.rs` 第 60 行：

```rust
let (alert_tx, alert_rx) = mpsc::channel::<TracedEvent<Alert>>(self.config.alert_buffer_size);
```

- [ ] **Step 4: 更新 spawn_datasources 转发逻辑**

修改 `crates/vol-engine/src/engine.rs` 第 113-129 行：

```rust
while let Some(event) = ds_rx.recv().await {
    let trace_id = vol_tracing::new_trace_id();
    let span = info_span!(
        "datasource_event",
        source = %event.source(),
        event_type = ?event.event_type(),
        trace_id = %trace_id,
    );
    span.record("timestamp", &event.timestamp());

    let traced_event = TracedEvent::new(event, span, trace_id);

    if tx.send(traced_event).is_err() {
        warn!("No event receivers, stopping datasource");
        break;
    }
}
```

- [ ] **Step 5: 更新 spawn_rules 从 TracedEvent 提取 trace_id**

修改 `crates/vol-engine/src/engine.rs` 第 151-199 行：

```rust
tokio::spawn(async move {
    info!("Starting rule: {}", rule_id);
    while let Ok(traced_event) = rx.recv().await {
        // Extract event and trace_id from the wrapper
        let (event, _parent_span, trace_id) = traced_event.split();

        // Fast path: skip events we're not interested in
        if !interests.contains(&event.event_type()) {
            continue;
        }

        // Create span for rule evaluation with business attributes
        let span = info_span!(
            "rule_evaluate",
            rule_id = %rule_id,
            rule_type = %rule_type,
            event_type = ?event.event_type(),
            event_timestamp = %event.timestamp(),
            event_source = %event.source(),
            trace_id = %trace_id,
        );

        // Evaluate rule within span context
        let alerts = rule_clone.evaluate(&event).instrument(span).await;

        // Process each alert with its own span
        for alert in alerts {
            // Create child span for alert
            let alert_span = info_span!(
                "alert_generated",
                alert_type = %alert.alert_type,
                tenor = ?alert.tenor,
                symbol = %alert.symbol,
                iv = %alert.iv,
                dte = alert.dte,
                index_price = %alert.index_price,
                trace_id = %trace_id,
            );

            // Wrap alert in TracedEvent with same trace_id
            let traced_alert = TracedEvent::new(alert, alert_span, trace_id.clone());

            // Send alert within span context
            if let Err(e) = tx.send(traced_alert).instrument(alert_span).await {
                error!(error = %e, "Failed to send alert");
                break;
            }
        }
    }
    Ok(())
})
```

- [ ] **Step 6: 更新 spawn_notifications 从 TracedEvent 提取 trace_id**

修改 `crates/vol-engine/src/engine.rs` 第 225-241 行：

```rust
vec![tokio::spawn(async move {
    info!("Starting {} notification channels", num_notifications);
    while let Some(traced_alert) = alert_rx.recv().await {
        // Extract alert and trace_id from wrapper
        let (alert, _span, trace_id) = traced_alert.split();
        
        // Create notification span with trace_id
        let notif_span = info_span!(
            "notification_send",
            trace_id = %trace_id,
            channel = "feishu"
        );
        
        // Check cooldown before sending
        if !alert_manager.can_send(&alert) {
            debug!("Alert in cooldown, skipping: {}:{}:{}",
                alert.alert_type, alert.tenor, alert.symbol);
            continue;
        }
        for notif in &notifications {
            let notif_span = info_span!(
                "notification_send",
                trace_id = %trace_id,
                channel = %notif.name()
            );
            if let Err(e) = notif.send(&alert).instrument(notif_span).await {
                error!("Notification {} failed: {}", notif.name(), e);
            }
        }
    }
    Ok(())
})]
```

- [ ] **Step 7: 验证编译**

```bash
cargo check -p vol-engine
```

- [ ] **Step 8: 提交**

```bash
git add crates/vol-engine/src/engine.rs
git commit -m "feat(engine): use TracedEvent for trace context in event pipeline"
```

## 6. vol-notification 从 TracedEvent 获取 trace_id

**Files:**
- Modify: `crates/vol-notification/src/feishu.rs:277-306`
- Modify: `crates/vol-notification/src/stdout.rs:29-72`

注意：由于 notification 层现在接收的是 `TracedEvent<Alert>`，需要在 engine.rs 的 spawn_notifications 中解包，所以实际 feishu.rs 和 stdout.rs 的 send() 方法签名不变，仍然接收 `&Alert`。trace_id 的提取在 engine.rs 中完成并创建 span。

- [ ] **Step 1: 验证 feishu.rs 不需要修改**

feishu.rs 的 send() 方法签名：
```rust
async fn send(&self, alert: &Alert) -> Result<()>
```

这个方法由 engine.rs 调用，engine.rs 负责从 TracedEvent 提取 alert 并创建包含 trace_id 的 span。

确认 feishu.rs 当前代码已经从 Alert 提取 trace_id 改为从 span 上下文获取（通过 engine 传入的 span）。

读取 `crates/vol-notification/src/feishu.rs` 第 277-306 行，确认是否需要修改。

如果 feishu.rs 中仍有 `let trace_id = vol_tracing::new_trace_id();` 则删除这行，改为从当前 span 获取：

```rust
async fn send(&self, alert: &Alert) -> Result<()> {
    // trace_id 由调用方 (engine) 在 span 中设置
    let span = info_span!(
        "notification_send",
        channel = "feishu",
        alert_type = %alert.alert_type,
        tenor = ?alert.tenor,
        symbol = %alert.symbol,
        iv = %alert.iv,
        dte = alert.dte,
        index_price = %alert.index_price,
        // trace_id 由 engine.rs 的 span 继承
    );
    // ...
}
```

- [ ] **Step 2: 更新 stdout.rs**

同样的，stdout.rs 也不直接处理 trace_id，由 engine.rs 创建的 span 提供上下文。

- [ ] **Step 3: 验证编译**

```bash
cargo check -p vol-notification
```

- [ ] **Step 4: 提交**

```bash
git add crates/vol-notification/src/feishu.rs crates/vol-notification/src/stdout.rs
git commit -m "refactor(notification): rely on parent span for trace context"
```

## 7. 清理 WithSpan（可选）

**Files:**
- Modify: `crates/vol-tracing/src/lib.rs`
- Modify: `crates/vol-tracing/src/with_span.rs`

- [ ] **Step 1: 将 WithSpan 标记为 deprecated**

修改 `crates/vol-tracing/src/lib.rs`:

```rust
pub use with_span::WithSpan;
pub use traced_event::TracedEvent;

/// @deprecated Use TracedEvent instead
#[deprecated(since = "0.5.0", note = "Use TracedEvent instead")]
pub use traced_event::TracedEvent as WithSpanCompat;
```

- [ ] **Step 2: 提交**

```bash
git add crates/vol-tracing/src/lib.rs
git commit -m "chore(tracing): mark WithSpan as deprecated in favor of TracedEvent"
```

## 8. 全workspace 验证

- [ ] **Step 1: 运行全量编译检查**

```bash
cargo check --workspace
```

Expected: No errors

- [ ] **Step 2: 运行测试（如果存在）**

```bash
cargo test --workspace
```

- [ ] **Step 3: 构建 release 版本**

```bash
cargo build --release
```

- [ ] **Step 4: 提交最终更改**

```bash
git commit -am "chore: complete TracedEvent migration"
```

## 9. 文档更新

**Files:**
- Modify: `docs/tracing.md`

- [ ] **Step 1: 更新 tracing 文档**

在 `docs/tracing.md` 中添加 TracedEvent 的说明：

```markdown
## TracedEvent: 跨 Channel Trace 上下文传递

`TracedEvent<T>` 用于在 channel 间传递事件时保持 trace 上下文一致：

```rust
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
```

- [ ] **Step 2: 提交**

```bash
git add docs/tracing.md
git commit -m "docs: add TracedEvent documentation"
```
