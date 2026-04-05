## ADDED Requirements

### Requirement: Trace ID 生成

系统应当为每条从 DataSource 接收的消息生成唯一的 trace_id，格式为 `tr_` 加 16 字符十六进制数。

#### Scenario: 生成 trace_id
- **WHEN** DataSource 收到 WebSocket 消息
- **THEN** 生成 trace_id 如 `tr_0000018c9a62f3d0`

#### Scenario: trace_id 唯一性
- **WHEN** 两条不同消息在不同时间到达
- **THEN** 两条消息的 trace_id 不相同

### Requirement: Span 创建与命名

系统应当为每个关键处理阶段创建 span，span 名称使用 snake_case 格式。

#### Scenario: DataSource span
- **WHEN** 收到市场数据
- **THEN** 创建名为 `datasource_receive` 的 span，包含 trace_id、source、symbol 字段

#### Scenario: Rule 评估 span
- **WHEN** Rule 评估市场数据
- **THEN** 创建名为 `rule_evaluate` 的 span，包含 rule_id、rule_type 字段

#### Scenario: Alert 触发 span
- **WHEN** Rule 触发告警
- **THEN** 创建名为 `alert_triggered` 的 span，包含 alert_type、tenor、symbol 字段

#### Scenario: Notification 发送 span
- **WHEN** 发送告警通知
- **THEN** 创建名为 `notification_send` 的 span，包含 notification_type、receive_id 字段

### Requirement: Span 跨 Channel 传播

Span 上下文必须能够跨越 tokio mpsc channel 边界传播，确保链路完整性。使用 `tracing::Span::follows_from()` 官方模式。

#### Scenario: WithSpan wrapper
- **WHEN** DataSource 发送事件到 channel
- **THEN** 使用 `WithSpan(event, span)` wrapper 包裹事件和 `tracing::Span`

#### Scenario: Receiver 端使用 follows_from
- **WHEN** Rule Engine 从 channel 接收 WithSpan
- **THEN** 创建新 span 并调用 `follows_from(parent_span.id())` 建立因果关联

### Requirement: 业务标签注入

Span 应当包含完整的业务数据标签，便于 Jaeger 查询过滤。

#### Scenario: 波动率数据标签
- **WHEN** 处理 VolatilityData 事件
- **THEN** Span 包含 `market.iv`、`market.symbol`、`market.mark_price_coin` 标签

#### Scenario: 告警数据标签
- **WHEN** 触发告警
- **THEN** Span 包含 `alert.iv`、`alert.threshold`、`alert.dte`、`alert.moneyness`、`alert.option_type` 标签

#### Scenario: 可配置标签
- **WHEN** 配置 `include_iv_value = false`
- **THEN** Span 不包含 `market.iv` 标签
