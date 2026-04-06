## ADDED Requirements

### Requirement: Span 命名规范

所有 span 使用统一的 `snake_case` 命名格式，遵循 `{stage}_{action}` 模式。

#### Scenario: DataSource span 命名
- **WHEN** DataSource 收到市场数据
- **THEN** span 命名为 `datasource_receive`

#### Scenario: Rule 评估 span 命名
- **WHEN** Rule 评估市场数据
- **THEN** span 命名为 `rule_evaluate`

#### Scenario: Alert 触发 span 命名
- **WHEN** Rule 触发告警
- **THEN** span 命名为 `alert_triggered`

#### Scenario: Notification 发送 span 命名
- **WHEN** 发送告警通知
- **THEN** span 命名为 `notification_send`

### Requirement: Span 字段注入规范

Span 字段使用预声明 + 运行时注入模式。

#### Scenario: 预声明空字段
- **WHEN** 创建 span 时
- **THEN** 使用 `tracing::field::Empty` 预声明需要后续注入的字段

#### Scenario: 运行时注入字段
- **WHEN** 获取到业务数据后
- **THEN** 使用 `span.record("field_name", &value)` 注入

### Requirement: 跨 Channel Span 传播

Span 上下文必须能够跨越 tokio mpsc channel 边界传播。

#### Scenario: WithSpan wrapper 发送
- **WHEN** DataSource 发送事件到 channel
- **THEN** 使用 `WithSpan::new(event, span)` 包裹事件和 span

#### Scenario: Receiver 端恢复 span
- **WHEN** Rule Engine 从 channel 接收 WithSpan
- **THEN** 使用 `.enter_span()` 方法恢复 span 上下文

#### Scenario: 建立因果关联
- **WHEN** 接收端创建新 span
- **THEN** 调用 `follows_from(parent_span.id())` 建立因果关联
