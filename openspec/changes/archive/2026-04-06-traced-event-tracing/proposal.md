## Why

当前 traceId 在 datasource、rule、notification 之间传递时存在断点，每个组件独立生成 traceId 导致无法端到端追踪。需要在 channel 传递层面统一 trace 上下文，实现完整的分布式追踪链路。

## What Changes

- **新增 TracedEvent<T> 包装类型**：替代现有的 WithSpan<T>，统一封装 Value + ParentSpan + TraceId
- **Datasource 入口生成 traceId**：在数据源入口处生成 traceId 并注入 Span 和 TracedEvent
- **Channel 传递统一包装**：所有跨 channel 传递的数据使用 TracedEvent 包装
- **Span 继承机制**：下游组件从 TracedEvent 获取 parent_span 创建子 Span 实现继承
- **Alert 移除 traceId 字段**：业务数据结构不再携带 traceId，通过 TracedEvent 传递
- **Notification 从 TracedEvent 获取 traceId**：通知层从包装类型提取 traceId 用于日志和消息

## Capabilities

### New Capabilities
- `traced-event`: TracedEvent<T> 包装类型，封装 Value + ParentSpan + TraceId，支持跨 channel 传递 trace 上下文

### Modified Capabilities
- `vol-core/event`: MonitoringEvent 不再直接传递，改为 TracedEvent<MonitoringEvent>
- `vol-core/alert`: Alert 结构移除 traceId 字段（如已添加则移除）
- `vol-datasource`: 入口生成 traceId，发送时使用 TracedEvent
- `vol-engine`: 从 TracedEvent 提取 traceId 和 span，创建子 span 继承
- `vol-notification`: 从 TracedEvent 提取 traceId，不再从 Alert 获取

## Impact

- **Breaking**: WithSpan<T> 替换为 TracedEvent<T>，所有使用位置需要更新
- **Breaking**: Alert 结构如已添加 traceId 字段需移除
- **影响模块**: vol-tracing, vol-core, vol-datasource, vol-engine, vol-notification
- **依赖**: tracing, tracing-opentelemetry
