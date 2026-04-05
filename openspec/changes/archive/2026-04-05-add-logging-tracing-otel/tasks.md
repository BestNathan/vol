## 1. 依赖与配置

- [ ] 1.1 添加 `tracing-appender` 依赖到 workspace
- [ ] 1.2 添加 `tracing-opentelemetry` 依赖到 workspace
- [ ] 1.3 添加 `opentelemetry-otlp` 依赖到 workspace
- [ ] 1.4 添加 `opentelemetry_sdk` 依赖到 workspace
- [ ] 1.5 创建 `vol-config/src/tracing.rs` 配置模块
- [ ] 1.6 更新 `vol-config/src/lib.rs` 导出 tracing 模块
- [ ] 1.7 更新 `Config` 结构体添加 `tracing` 字段

## 2. 日志基础设施

- [ ] 2.1 创建 `vol-monitor/src/tracing_setup.rs` 模块
- [ ] 2.2 实现控制台日志层（紧凑格式，带颜色）
- [ ] 2.3 实现文件日志层（JSON 格式，按天滚动）
- [ ] 2.4 实现错误日志层（仅 ERROR 级别）
- [ ] 2.5 实现日志配置解析（config.toml + 环境变量）
- [ ] 2.6 更新 `main.rs` 调用 `tracing_setup::init()`
- [ ] 2.7 更新 `config.toml` 添加 `[tracing.logging]` 配置节

## 3. OpenTelemetry Tracing

- [ ] 3.1 实现 OTLP gRPC 导出器配置
- [ ] 3.2 实现 Service 元数据（service_name, namespace, environment）
- [ ] 3.3 实现采样率配置
- [ ] 3.4 实现批量导出配置（max_queue_size, max_batch_size 等）
- [ ] 3.5 创建 `OpenTelemetryLayer` 并注册到 subscriber
- [ ] 3.6 实现导出失败降级逻辑（不阻塞主流程）
- [ ] 3.7 更新 `config.toml` 添加 `[tracing.opentelemetry]` 配置节

## 4. Span 埋点 - DataSource

- [ ] 4.1 创建 `WithSpan<T>` wrapper 类型（使用官方 `tracing::Span`）
- [ ] 4.2 实现 `generate_trace_id()` 函数
- [ ] 4.3 在 `VolatilityDataSource::run()` 中为每条消息创建根 span
- [ ] 4.4 为 VolatilityData 添加业务标签（iv, symbol, mark_price 等）
- [ ] 4.5 在 `PortfolioDataSource::run()` 中为每次轮询创建根 span
- [ ] 4.6 使用 `WithSpan` wrapper 发送到 channel

## 5. Span 埋点 - Rule Engine

- [ ] 5.1 在 `Rule::evaluate()` 入口创建子 span
- [ ] 5.2 从 `WithSpan` 提取 parent span，使用 `follows_from()` 建立关联
- [ ] 5.3 为 Rule 添加业务标签（rule_id, rule_type, threshold 等）
- [ ] 5.4 在 `AbsoluteIvRule` 中实现埋点
- [ ] 5.5 在 `RateChangeRule` 中实现埋点
- [ ] 5.6 在 `TermStructureRule` 中实现埋点
- [ ] 5.7 在 `SkewRule` 中实现埋点
- [ ] 5.8 在 `PortfolioRule` 中实现埋点

## 6. Span 埋点 - Notification

- [ ] 6.1 在 `NotificationHandler::send()` 入口创建子 span
- [ ] 6.2 为 Alert 添加完整业务标签（alert_type, tenor, iv, dte 等）
- [ ] 6.3 在 `StdoutNotification` 中实现埋点
- [ ] 6.4 在 `FeishuNotification` 中实现埋点
- [ ] 6.5 Feishu 消息格式添加 trace_id 前缀

## 7. 日志迁移

- [ ] 7.1 迁移所有 `println!` 到 `tracing::info!`
- [ ] 7.2 迁移所有 `eprintln!` 到 `tracing::error!`
- [ ] 7.3 清理未使用的 `use tracing` 导入
- [ ] 7.4 统一日志消息格式（key=value 风格）

## 8. 部署配置

- [ ] 8.1 创建 `docker-compose.jaeger.yml` 参考配置
- [ ] 8.2 创建 `k8s/jaeger-configmap.yaml` 参考配置
- [ ] 8.3 更新 `k8s/deployment.yaml` 添加 `OTEL_ENDPOINT` 环境变量
- [ ] 8.4 更新 `CLAUDE.md` 添加日志和 tracing 相关命令

## 9. 测试与验证

- [ ] 9.1 编写单元测试验证日志配置解析
- [ ] 9.2 编写集成测试验证 `follows_from()` 跨 channel 传播
- [ ] 9.3 本地启动 Jaeger 验证 trace 导出
- [ ] 9.4 验证日志文件滚动和清理
- [ ] 9.5 验证 Jaeger UI 查询功能
- [ ] 9.6 验证环境变量覆盖配置

## 10. 文档

- [ ] 10.1 更新 `README.md` 添加 Tracing 架构说明
- [ ] 10.2 创建 `docs/tracing.md` 详细文档
- [ ] 10.3 创建 `docs/jaeger-setup.md` 部署指南
