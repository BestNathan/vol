## Why

当前项目缺少统一的日志规范和分布式追踪能力，生产环境问题排查困难：日志无文件持久化、错误日志未分离、无法通过告警反向追踪完整链路。引入 OpenTelemetry + Jaeger 可实现端到端的可观测性。

## What Changes

- **新增日志系统**：控制台 + 文件双输出，文件按天滚动保留 7 天，错误日志独立文件
- **新增 JSON 格式日志**：便于结构化查询和分析
- **新增 OpenTelemetry Tracing**：通过 `tracing-opentelemetry` 桥接，导出到 Jaeger
- **新增配置化支持**：`[tracing]` 配置节，支持日志、OTLP endpoint、采样率等配置
- **新增 Span 上下文传播**：通过 `TracedEvent` wrapper 跨越 channel 传递 span
- **新增完整业务标签**：Span 包含 IV、threshold、DTE、moneyness 等业务数据

## Capabilities

### New Capabilities

- `logging`: 统一日志基础设施，包括控制台/文件输出、日志轮转、JSON 格式化、错误日志分离
- `tracing`: 基于 tracing 的链路追踪，包括 span 创建、上下文传播、trace_id 生成
- `opentelemetry-export`: OpenTelemetry OTLP 导出器，将 spans 发送到 Jaeger Collector
- `jaeger-integration`: Jaeger 查询集成，包括 UI 访问、trace 查询、标签过滤

### Modified Capabilities

- (none)

## Impact

- **代码影响**：所有 datasource、rule、notification 模块需要添加 span 埋点
- **依赖新增**：`tracing-appender`, `tracing-opentelemetry`, `opentelemetry-otlp`, `opentelemetry_sdk`
- **配置影响**：`config.toml` 新增 `[tracing]` 配置节
- **部署影响**：需要部署 Jaeger Collector 服务，K8s ConfigMap 注入配置
- **日志路径**：新增 `logs/` 目录，生产环境需配置持久化存储或日志收集
