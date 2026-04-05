## ADDED Requirements

### Requirement: OTLP gRPC 导出器

系统应当通过 OTLP gRPC 协议将 spans 导出到 Jaeger Collector 或其他兼容的后端。

#### Scenario: 基本导出
- **WHEN** 配置 `endpoint = "http://localhost:4317"` 且 span 结束
- **THEN** span 数据通过 gRPC 发送到指定 endpoint

#### Scenario: 批量导出
- **WHEN** 配置 `batch.max_batch_size = 512` 且累积了 512 个 span
- **THEN** 批量发送这 512 个 span，而不是单个发送

#### Scenario: 定时导出
- **WHEN** 配置 `batch.scheduled_delay_millis = 5000`
- **THEN** 每隔 5 秒执行一次导出，即使未达到 max_batch_size

#### Scenario: 导出超时
- **WHEN** 配置 `batch.max_export_timeout_millis = 30000` 且导出超过 30 秒
- **THEN** 取消本次导出，记录错误日志，继续后续导出

### Requirement: Service 元数据

导出的 spans 应当包含服务元数据，便于在 Jaeger UI 中识别和过滤。

#### Scenario: Service Name
- **WHEN** 配置 `service_name = "vol-monitor"`
- **THEN** 所有 span 的 resource 包含 `service.name = "vol-monitor"` 属性

#### Scenario: Service Namespace
- **WHEN** 配置 `service_namespace = "deribit"`
- **THEN** 所有 span 的 resource 包含 `service.namespace = "deribit"` 属性

#### Scenario: Deployment Environment
- **WHEN** 配置 `deployment_environment = "production"`
- **THEN** 所有 span 的 resource 包含 `deployment.environment = "production"` 属性

### Requirement: 采样配置

系统应当支持配置采样率，控制导出到 Jaeger 的 trace 数量。

#### Scenario: 100% 采样
- **WHEN** 配置 `sample_rate = 1.0`
- **THEN** 所有 trace 都被导出到 Jaeger

#### Scenario: 10% 采样
- **WHEN** 配置 `sample_rate = 0.1`
- **THEN** 大约 10% 的 trace 被导出到 Jaeger

#### Scenario: 禁用采样
- **WHEN** 配置 `sample_rate = 0.0` 或 `enabled = false`
- **THEN** 没有 trace 被导出到 Jaeger

### Requirement: 导出失败处理

OTLP 导出失败不应当影响主业务流程，仅记录错误日志。

#### Scenario: Jaeger 服务不可用
- **WHEN** Jaeger Collector 宕机，网络无法连接
- **THEN** 导出失败记录 ERROR 日志，主业务流程继续正常运行

#### Scenario: 网络超时
- **WHEN** 网络延迟导致导出超时
- **THEN** 记录 WARN 日志，不重试失败的批次，继续处理新 span

#### Scenario: 认证失败
- **WHEN** Jaeger 需要认证但配置错误
- **THEN** 记录 ERROR 日志，提示检查配置，不阻塞主流程
