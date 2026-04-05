## ADDED Requirements

### Requirement: Jaeger UI 查询

用户应当能够通过 Jaeger UI 查询和查看 trace 链路。

#### Scenario: 按服务名查询
- **WHEN** 用户在 Jaeger UI 选择服务 `vol-monitor`
- **THEN** 显示该服务的所有 trace 列表，按时间倒序排列

#### Scenario: 按 trace_id 查询
- **WHEN** 用户输入 trace_id `tr_abc123def456`
- **THEN** 显示该 trace 的完整 span 链路图

#### Scenario: 按标签过滤
- **WHEN** 用户设置过滤条件 `alert.type = absolute_iv`
- **THEN** 仅显示包含该标签的 trace

### Requirement: 告警反向追踪

用户应当能够从 Feishu 告警消息中的 trace_id 反向追踪完整链路。

#### Scenario: Feishu 消息包含 trace_id
- **WHEN** 告警触发发送 Feishu 消息
- **THEN** 消息格式为 `[tr_abc123] 🚨 BTC IV=0.72`，包含 trace_id 前缀

#### Scenario: 日志查询 trace
- **WHEN** 用户在日志文件搜索 `tr_abc123`
- **THEN** 显示该 trace 的所有相关日志，包括 datasource、rule、notification

#### Scenario: Jaeger 查询 trace
- **WHEN** 用户在 Jaeger UI 搜索 `tr_abc123`
- **THEN** 显示该 trace 的完整 waterfall 图，包含所有 span 的时间线

### Requirement: Jaeger 部署配置

项目应当提供 Jaeger 部署的参考配置，支持 K8s 环境。

#### Scenario: Docker Compose 部署
- **WHEN** 用户运行 `docker-compose -f docker-compose.jaeger.yml up -d`
- **THEN** Jaeger all-in-one 容器启动，UI 可通过 `http://localhost:16686` 访问

#### Scenario: K8s ConfigMap 配置
- **WHEN** 用户应用 `k8s/jaeger-configmap.yaml`
- **THEN** 创建 Jaeger Collector 配置，OTLP gRPC 端口 4317 暴露

#### Scenario: 环境变量注入
- **WHEN** K8s Deployment 配置 `OTEL_ENDPOINT` 环境变量指向 Jaeger 服务
- **THEN** vol-monitor pod 启动后自动连接到指定 Jaeger 服务

### Requirement: 标签查询优化

Jaeger 中的 span 标签应当支持高效查询，常用的业务字段都应当作为 tag 而非 log。

#### Scenario: 按 symbol 查询
- **WHEN** 用户搜索 `market.symbol = BTC`
- **THEN** 返回所有 BTC 相关的 trace

#### Scenario: 按 tenor 查询
- **WHEN** 用户搜索 `alert.tenor = short`
- **THEN** 返回所有 short tenor 告警的 trace

#### Scenario: 按 IV 阈值查询
- **WHEN** 用户搜索 `alert.iv > 0.7`
- **THEN** 返回所有 IV 大于 0.7 的告警 trace
