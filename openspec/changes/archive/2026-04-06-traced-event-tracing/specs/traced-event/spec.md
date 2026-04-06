## ADDED Requirements

### Requirement: TracedEvent 包装类型

系统应当提供 `TracedEvent<T>` 泛型包装类型，封装业务数据、parent_span 和 trace_id，支持跨 channel 传递 trace 上下文。

#### Scenario: 创建 TracedEvent
- **WHEN** DataSource 生成 trace_id 和 span 后
- **THEN** 调用 `TracedEvent::new(value, span, trace_id)` 创建包装事件

#### Scenario: 提取包装内容
- **WHEN** 下游组件收到 TracedEvent
- **THEN** 调用 `traced_event.split()` 获取 `(value, parent_span, trace_id)`

#### Scenario: 获取 trace_id 引用
- **WHEN** 只需要 trace_id 而不需要消费整个包装
- **THEN** 调用 `traced_event.trace_id()` 获取 `&str` 引用

### Requirement: traceId 从 TracedEvent 获取

业务数据结构（如 Alert）不再携带 trace_id 字段，trace_id 通过 TracedEvent 传递到需要的位置。

#### Scenario: Rule 生成 Alert 时不携带 traceId
- **WHEN** Rule 处理 MonitoringEvent 生成 Alert
- **THEN** Alert 结构体不包含 trace_id 字段

#### Scenario: Notification 从 TracedEvent 提取 traceId
- **WHEN** Notification 准备发送告警
- **THEN** 从 TracedEvent<Alert> 提取 trace_id 用于日志和消息前缀

## MODIFIED Requirements

### Requirement: Span 跨 Channel 传播

原有 `WithSpan<T>` 替换为 `TracedEvent<T>`，提供统一的 trace_id 访问 API。

#### Scenario: TracedEvent wrapper
- **WHEN** DataSource 发送事件到 channel
- **THEN** 使用 `TracedEvent { value, parent_span, trace_id }` wrapper 包裹事件

#### Scenario: Receiver 端继承 traceId
- **WHEN** Rule Engine 从 channel 接收 TracedEvent
- **THEN** 调用 `split()` 获取 trace_id，创建新 span 时设置相同的 trace_id 字段

### Requirement: Trace ID 生成

trace_id 在 DataSource 入口处生成，通过 TracedEvent 贯穿整个处理链路。

#### Scenario: datasource 生成 trace_id
- **WHEN** DataSource 收到 WebSocket 消息
- **THEN** 生成 trace_id 如 `tr_0000018c9a62f3d0` 并存入 TracedEvent

#### Scenario: traceId 贯穿链路
- **WHEN** 同一事件经过 datasource → rule → notification
- **THEN** 三个阶段的 trace_id 完全相同

## REMOVED Requirements

### Requirement: Alert.traceId 字段

**Reason**: trace_id 由 TracedEvent 统一传递，业务数据结构不应耦合追踪逻辑
**Migration**: 从 TracedEvent<Alert> 提取 trace_id 而非从 Alert 结构体获取
