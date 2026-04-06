## 1. vol-tracing: TracedEvent 类型定义

- [ ] 1.1 创建 `crates/vol-tracing/src/traced_event.rs` 定义 `TracedEvent<T>` 结构体
- [ ] 1.2 实现 `TracedEvent::new(value, span, trace_id)` 构造函数
- [ ] 1.3 实现 `TracedEvent::split()` 方法返回 `(T, Option<Span>, String)`
- [ ] 1.4 实现 `TracedEvent::trace_id()` 方法返回 `&str`
- [ ] 1.5 实现 `TracedEvent::value()` 和 `TracedEvent::into_value()` 方法
- [ ] 1.6 在 `vol-tracing/src/lib.rs` 中导出 `TracedEvent`

## 2. vol-datasource: 入口生成 traceId

- [ ] 2.1 修改 `volatility.rs` 导入 `TracedEvent`
- [ ] 2.2 在收到市场数据时生成 `trace_id = new_trace_id()`
- [ ] 2.3 创建 span 时注入 `trace_id` 字段
- [ ] 2.4 使用 `TracedEvent::new(vol_data, span, trace_id)` 包装事件
- [ ] 2.5 通过 channel 发送 `TracedEvent<VolatilityData>`
- [ ] 2.6 移除 `monitoring_event` 包装中不必要的 span 创建（避免重复）

## 3. vol-engine: 从 TracedEvent 提取 traceId

- [ ] 3.1 修改 `engine.rs` 导入 `TracedEvent`
- [ ] 3.2 更新 `spawn_datasources` 使用 `TracedEvent<MonitoringEvent>` 包装
- [ ] 3.3 更新 `spawn_rules` 从 `TracedEvent<MonitoringEvent>` 提取 `trace_id`
- [ ] 3.4 在 `rule_evaluate` span 中注入 `trace_id` 字段
- [ ] 3.5 Rule 生成 Alert 后使用 `TracedEvent::new(alert, span, trace_id)` 包装
- [ ] 3.6 更新 notification 通道为 `TracedEvent<Alert>`

## 4. vol-notification: 从 TracedEvent 获取 traceId

- [ ] 4.1 修改 notification 入口从 `TracedEvent<Alert>` 提取 `trace_id`
- [ ] 4.2 在 `feishu.rs` 中使用 `alert.trace_id` 改为从 `TracedEvent` 提取
- [ ] 4.3 在 `stdout.rs` 中使用 `alert.trace_id` 改为从 `TracedEvent` 提取
- [ ] 4.4 创建 `notification_send` span 时注入 `trace_id` 字段

## 5. vol-core: 移除 Alert.traceId 字段

- [ ] 5.1 检查 `alert.rs` 中是否已添加 `trace_id` 字段
- [ ] 5.2 如已添加则移除 `trace_id` 字段
- [ ] 5.3 更新 `Alert::new()` 构造函数移除 `trace_id` 参数
- [ ] 5.4 更新所有 `Alert::new()` 调用位置

## 6. 清理与验证

- [ ] 6.1 移除或弃用 `WithSpan` 类型
- [ ] 6.2 更新所有 `WithSpan` 使用位置为 `TracedEvent`
- [ ] 6.3 运行 `cargo check --workspace` 验证编译
- [ ] 6.4 运行程序验证 traceId 贯穿链路
