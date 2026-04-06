## Context

当前系统使用 `WithSpan<T>` 在 channel 间传递事件和 span 上下文，但存在以下问题：
1. traceId 在 datasource、rule、notification 三层之间传递不一致
2. Alert 结构体携带 traceId 字段导致业务数据与追踪逻辑耦合
3. 缺少统一的 trace 上下文继承机制

## Goals / Non-Goals

**Goals:**
- 设计 `TracedEvent<T>` 包装类型，统一封装 `Value + ParentSpan + TraceId`
- datasource 入口生成 traceId，通过 TracedEvent 贯穿到 notification
- 下游组件从 TracedEvent 继承 span 和 traceId
- Alert 等业务数据结构不携带 traceId 字段
- 支持从 TracedEvent 提取 traceId 用于日志和外部消息

**Non-Goals:**
- 不改变现有 DataSource/Rule/Notification trait 接口
- 不引入 OpenTelemetry Context 显式传递
- 不改变现有 span 层级结构（仅确保 traceId 一致）

## Decisions

### 1. TracedEvent 结构设计

```rust
pub struct TracedEvent<T> {
    value: T,
    parent_span: Option<Span>,
    trace_id: String,
}
```

**Rationale:**
- 显式存储 traceId 而非从 span context 读取，避免依赖 OpenTelemetry context API
- parent_span 用于 downstream 创建子 span 实现继承
- 泛型 T 支持包装任意事件类型

### 2. traceId 生成位置

**Datasource 入口处生成**：在 `volatility.rs` 接收到市场数据时生成 traceId

```rust
let trace_id = new_trace_id();
let span = info_span!("datasource_receive", trace_id = %trace_id, ...);
let traced = TracedEvent::new(vol_data, span, trace_id);
```

**Rationale:**
- 数据源是事件生命周期的起点
- 确保同一事件触发的所有后续操作共享 traceId

### 3. Channel 传递包装

所有跨 channel 传递使用 `TracedEvent<T>`：
- `broadcast::Sender<TracedEvent<MonitoringEvent>>`
- `mpsc::Sender<Alert>` → 保持原样（Alert 通过 TracedEvent 传递到 rule，rule 处理后生成 Alert）

**Rationale:**
- 统一 channel 层面的 trace 上下文传递
- 不影响业务数据结构

### 4. Span 继承机制

下游组件从 TracedEvent 提取 parent_span 创建子 span：

```rust
let (event, parent_span, trace_id) = traced_event.split();
let span = info_span!("rule_evaluate", trace_id = %trace_id, ...);
if let Some(parent) = parent_span {
    span.follows_from(parent.id());
}
```

**Rationale:**
- `follows_from` 建立因果关系而非父子关系，适合跨组件追踪
- traceId 保持一致，span ID 逐层创建新的

### 5. Alert 不携带 traceId

Alert 结构体不添加 traceId 字段，notification 从 TracedEvent 提取：

```rust
// Rule 层
let traced_alert = TracedEvent::new(alert, span, trace_id);
alert_tx.send(traced_alert).await;

// Notification 层
let (alert, _span, trace_id) = traced_alert.split();
```

**Rationale:**
- 业务数据纯净，不依赖追踪逻辑
- traceId 传递由 TracedEvent 负责

## Risks / Trade-offs

**[Risk] 现有 WithSpan 使用位置需要全部更新** → 系统性修改，需要完整测试
**[Risk] broadcast channel 需要更改类型** → 影响所有订阅者，需要协调修改
**[Trade-off] 显式存储 traceId vs 从 span context 读取** → 选择显式存储，API 更简单但增加数据冗余

## Migration Plan

1. 创建 `TracedEvent<T>` 类型（vol-tracing crate）
2. 更新 vol-datasource：入口处生成 traceId，使用 TracedEvent 包装
3. 更新 vol-engine：从 TracedEvent 提取 traceId 和 span
4. 更新 vol-notification：从 TracedEvent 提取 traceId
5. 移除 Alert.traceId 字段（如已添加）
6. 删除 WithSpan 或标记 deprecated
7. 编译测试 + 运行验证

## Open Questions

无
