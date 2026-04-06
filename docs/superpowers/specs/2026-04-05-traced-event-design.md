# TracedEvent 实现设计

## 1. 架构概述

```
┌─────────────────────────────────────────────────────────────────┐
│                    traceId 贯穿链路                              │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  datasource           engine              notification           │
│  ┌─────────┐         ┌─────────┐         ┌─────────┐           │
│  │ 生成     │         │ 从       │         │ 从       │           │
│  │ traceId │─────▶   │ Traced   │─────▶   │ Traced   │          │
│  │ 创建     │         │ Event    │         │ Event    │          │
│  │ Traced   │         │ 提取     │         │ 提取     │          │
│  │ Event    │         │ traceId  │         │ traceId  │          │
│  └─────────┘         └─────────┘         └─────────┘           │
│       │                    │                    │                │
│       ▼                    ▼                    ▼                │
│  TracedEvent          TracedEvent          TracedEvent          │
│  <VolatilityData>     <MonitoringEvent>    <Alert>              │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## 2. TracedEvent 结构设计

### 2.1 结构体定义

```rust
// crates/vol-tracing/src/traced_event.rs
pub struct TracedEvent<T> {
    value: T,
    parent_span: Option<Span>,
    trace_id: String,
}
```

### 2.2 API 设计

| 方法 | 签名 | 用途 |
|------|------|------|
| `new` | `fn new(value: T, span: Span, trace_id: String) -> Self` | 创建包装事件 |
| `split` | `fn split(self) -> (T, Option<Span>, String)` | 解包获取三要素 |
| `trace_id` | `fn trace_id(&self) -> &str` | 获取 trace_id 引用 |
| `value` | `fn value(&self) -> &T` | 获取 value 引用 |
| `into_value` | `fn into_value(self) -> T` | 消耗包装获取 value |

## 3. 数据流与类型转换

### 3.1 完整数据流

```
VolatilityDataSource
    │
    │ TracedEvent<VolatilityData>
    ▼
MonitoringEngine::spawn_datasources
    │
    │ TracedEvent<MonitoringEvent::Volatility(VolatilityData)>
    ▼
MonitoringEngine::spawn_rules
    │
    │ TracedEvent<MonitoringEvent>
    ▼
RuleProcessor::evaluate() → Alert
    │
    │ TracedEvent<Alert>
    ▼
NotificationHandler::send()
```

### 3.2 关键点说明

1. **Datasource 层**：生成 trace_id，创建 `TracedEvent<VolatilityData>`
2. **Engine 转发**：将 `TracedEvent<VolatilityData>` 转换为 `TracedEvent<MonitoringEvent>`
3. **Rule 层**：从 `TracedEvent<MonitoringEvent>` 提取 trace_id，创建 `TracedEvent<Alert>`
4. **Notification 层**：从 `TracedEvent<Alert>` 提取 trace_id 用于日志

## 4. 各层实现细节

### 4.1 Datasource 层 (volatility.rs)

```rust
// 收到市场数据时
let trace_id = new_trace_id();
let span = info_span!(
    "datasource_receive",
    trace_id = %trace_id,
    source = "deribit",
    symbol = %vol_data.symbol,
    // ... 其他字段
);

// 创建包装事件
let traced = TracedEvent::new(vol_data.clone(), span.clone(), trace_id.clone());

// 通过内部 channel 发送
internal_tx.send(traced).instrument(span).await?;

// 转发到外部广播 channel 时
while let Some(traced_vol) = internal_rx.recv().await {
    let (vol_data, _span, trace_id) = traced_vol.split();
    
    // 创建新的 span 用于转发
    let forward_span = info_span!(
        "datasource_forward",
        trace_id = %trace_id,
        event_type = "volatility"
    );
    
    // 包装为 MonitoringEvent
    let monitoring_event = MonitoringEvent::Volatility(vol_data);
    let traced_monitoring = TracedEvent::new(monitoring_event, forward_span, trace_id);
    
    event_tx.send(traced_monitoring)?;
}
```

### 4.2 Engine 层 (engine.rs)

```rust
// spawn_rules 中
while let Ok(traced_event) = rx.recv().await {
    // 提取 trace_id 和原始数据
    let (event, _parent_span, trace_id) = traced_event.split();
    
    // 创建 rule 评估 span，注入相同 trace_id
    let span = info_span!(
        "rule_evaluate",
        trace_id = %trace_id,
        rule_id = %rule_id,
        // ...
    );
    
    // 执行评估
    let alerts = rule.evaluate(&event).instrument(span).await;
    
    // 为每个 alert 创建包装
    for alert in alerts {
        let alert_span = info_span!(
            "alert_generated",
            trace_id = %trace_id,
            alert_type = %alert.alert_type
        );
        
        let traced_alert = TracedEvent::new(alert, alert_span, trace_id.clone());
        alert_tx.send(traced_alert).await?;
    }
}
```

### 4.3 Notification 层 (notification manager)

```rust
// engine.rs::spawn_notifications
while let Some(traced_alert) = alert_rx.recv().await {
    // 提取 trace_id
    let (alert, _span, trace_id) = traced_alert.split();
    
    // 创建 notification span
    let notif_span = info_span!(
        "notification_send",
        trace_id = %trace_id,
        channel = "feishu"
    );
    
    // 发送通知
    notif.send(&alert).instrument(notif_span).await?;
}
```

## 5. Alert 结构修改

### 5.1 移除 trace_id 字段

**修改前：**
```rust
pub struct Alert {
    // ... 其他字段
    pub trace_id: String,  // ← 移除
}
```

**修改后：**
```rust
pub struct Alert {
    // ... 其他字段保持不变
    // trace_id 字段已移除，通过 TracedEvent<Alert> 传递
}
```

### 5.2 更新 Alert::new()

**修改前：**
```rust
pub fn new(..., mark_price_coin: f64, trace_id: String) -> Self
```

**修改后：**
```rust
pub fn new(..., mark_price_coin: f64) -> Self
```

## 6. WithSpan 迁移

### 6.1 重命名策略

1. 在 `vol-tracing/src/lib.rs` 中：
   ```rust
   pub use traced_event::TracedEvent;
   // 可选：保留 WithSpan 作为类型别名（deprecated）
   #[deprecated(since = "0.5.0", note = "Use TracedEvent instead")]
   pub use traced_event::TracedEvent as WithSpan;
   ```

2. 更新所有导入：
   ```rust
   // 原：use vol_tracing::WithSpan;
   use vol_tracing::TracedEvent;
   ```

## 7. 测试策略

### 7.1 单元测试

- `TracedEvent::new()` 正确存储三个字段
- `TracedEvent::split()` 正确解包
- `TracedEvent::trace_id()` 返回正确引用

### 7.2 集成测试

- 启动完整 pipeline，验证同一事件的 trace_id 一致
- 日志输出包含正确的 trace_id 字段
- Jaeger 中可查询完整调用链

## 8. 文件修改清单

| 文件 | 操作 | 说明 |
|------|------|------|
| `vol-tracing/src/traced_event.rs` | 新建 | TracedEvent 定义（可从 with_span.rs 重命名） |
| `vol-tracing/src/lib.rs` | 修改 | 导出 TracedEvent |
| `vol-core/src/alert.rs` | 修改 | 移除 trace_id 字段 |
| `vol-datasource/src/volatility.rs` | 修改 | 使用 TracedEvent |
| `vol-engine/src/engine.rs` | 修改 | 使用 TracedEvent |
| `vol-notification/src/feishu.rs` | 修改 | 从 TracedEvent 提取 trace_id |
| `vol-notification/src/stdout.rs` | 修改 | 从 TracedEvent 提取 trace_id |

## 9. 编译验证步骤

1. `cargo check --workspace` - 验证编译
2. `cargo build --release` - 构建 release 版本
3. 运行程序观察日志中的 trace_id
4. 检查 Jaeger 中的调用链
