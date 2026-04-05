## Context

当前 vol-monitor 项目使用 `tracing` crate 进行基础日志记录，但存在以下问题：
1. 日志仅输出到控制台，无文件持久化，重启后无法追溯
2. 错误日志与普通日志混合，无独立分类
3. 缺少分布式追踪能力，无法从告警反向追踪完整数据流
4. 无配置化支持，日志级别和输出格式硬编码

项目架构为多 crate workspace，数据流：Deribit WebSocket → DataSource → mpsc channel → Rule Engine → AlertManager → Notification。跨 channel 的 span 传播需要特殊处理。

## Goals / Non-Goals

**Goals:**
- 实现日志双输出（控制台 + 文件），文件按天滚动，保留 7 天
- 实现错误日志独立文件（仅 ERROR 级别）
- 实现 JSON 格式日志，便于结构化查询
- 实现 OpenTelemetry OTLP 导出到 Jaeger
- 实现 Span 跨 channel 传播（通过 `TracedEvent` wrapper）
- 实现配置化（config.toml + 环境变量覆盖）
- Span 包含完整业务标签（IV、threshold、DTE 等）

**Non-Goals:**
- 不修改现有业务数据结构（`VolatilityData`, `Alert` 保持原样）
- 不引入日志收集系统（如 Loki、ELK），仅本地文件 + Jaeger
- 不支持动态日志级别（需重启生效）
- 不实现 Metrics 指标收集（仅 Tracing）

## Decisions

### 1. 日志格式：控制台紧凑 + 文件 JSON

| 选项 | 方案 | 理由 |
|------|------|------|
| 控制台 | 紧凑格式，带颜色 | 适合人工阅读，减少干扰 |
| 文件 | JSON 格式 | 便于 `jq` 查询、日志收集、结构化分析 |

**Alternatives Considered:**
- 全部用纯文本：不利于自动化查询，排除
- 全部用 JSON：控制台可读性差，排除

### 2. Span 传播：`WithSpan<T>` wrapper + `follows_from()`

```rust
pub struct WithSpan<T>(T, Option<Span>);

// Sender
let span = tracing::info_span!("datasource_receive");
tx.send(WithSpan(event, Some(span))).await?;

// Receiver
let (event, parent_span) = traced.split();
let child_span = tracing::info_span!("rule_evaluate");
if let Some(parent) = parent_span {
    child_span.follows_from(parent.id());
}
```

使用 `follows_from()` 建立因果关系（receiver span 是 sender span 触发的结果），而不是父子关系。

**Alternatives Considered:**
- 修改 `VolatilityData` 添加 `trace_id` 字段：侵入业务数据，排除
- 使用 `task_local`：无法跨越 channel 边界，排除
- 使用全局 `DashMap<trace_id, Span>`：复杂度高，内存泄漏风险，排除
- 直接传递 `Span` 然后 `enter()`：语义不对，receiver 不是 sender 的子过程，排除

### 3. Jaeger 连接：OTLP gRPC

使用 `opentelemetry-otlp` crate，通过 gRPC 连接到 Jaeger Collector (port 4317)。

**Alternatives Considered:**
- Jaeger Thrift 协议：已过时，排除
- Jaeger Agent UDP 模式：不可靠，正在废弃，排除
- OTLP HTTP：gRPC 性能更好，Rust 支持成熟

### 4. 配置化：config.toml + 环境变量覆盖

```toml
[tracing.logging]
log_dir = "logs"
retention_days = 7

[tracing.opentelemetry]
endpoint = "http://jaeger:4317"
sample_rate = 1.0
```

环境变量优先级更高：`OTEL_ENDPOINT > config.toml`

**Alternatives Considered:**
- 仅配置文件：多环境部署不灵活，排除
- 仅环境变量：缺少默认值，本地开发不便，排除

### 5. 采样策略：可配置采样率

`sample_rate` 配置项 (0.0-1.0)，支持生产环境按需调整。

**Alternatives Considered:**
- 固定 100% 采样：高流量场景成本高，排除
- 基于概率的头采样：Rust OTLP 不支持，排除

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| `WithSpan<T>` wrapper 增加代码复杂度 | 封装为统一 API，提供 `enter_span()` 闭包方法简化使用 |
| Span 跨 channel 传播失败 | 使用官方 `follows_from()` 模式，编写集成测试验证 |
| Jaeger 服务不可用影响主流程 | 导出失败不阻塞主流程，仅记录错误日志 |
| 日志文件占用磁盘空间 | 配置 `retention_days=7` 自动清理，生产环境配置日志轮转 |
| OTLP gRPC 连接超时 | 配置 `max_export_timeout_millis=30000`，批量导出失败降级为本地日志 |
| Span attributes 数据量过大 | 仅包含关键字段，不包含完整 JSON payload |

## Migration Plan

### Phase 1: 基础设施 (Week 1)
1. 添加依赖：`tracing-appender`, `tracing-opentelemetry`, `opentelemetry-*`
2. 创建 `vol-config/src/tracing.rs` 配置模块
3. 创建 `vol-monitor/src/tracing_setup.rs` 初始化模块
4. 更新 `config.toml` 添加 `[tracing]` 配置节

### Phase 2: 日志迁移 (Week 2)
1. 更新 `main.rs` 初始化日志
2. 迁移所有 `println!` 为 `tracing::info!`
3. 迁移所有 `eprintln!` 为 `tracing::error!`

### Phase 3: Tracing 埋点 (Week 3)
1. DataSource 层：为每条消息创建根 span
2. Rule 层：为每次评估创建子 span（使用 `follows_from()` 关联）
3. Notification 层：为每次通知创建子 span
4. 实现 `WithSpan<T>` wrapper 跨越 channel（使用官方 `Span` 类型）

### Phase 4: 部署验证 (Week 4)
1. 部署 Jaeger Collector 到 K8s
2. 配置 ConfigMap 注入配置
3. 验证 trace 查询功能
4. 验证日志轮转功能

### Rollback Strategy
- 回滚 `config.toml` 删除 `[tracing]` 配置节
- 设置 `OTEL_ENABLED=false` 禁用 tracing
- 日志回退到仅控制台输出

## Open Questions

1. **Jaeger 存储后端**：生产环境使用什么存储？(InnoDB/Elasticsearch/Cassandra)
2. **日志收集**：是否需要将本地日志收集到中央系统？(如 Loki/ELK)
3. **告警通知中的 trace_id**：Feishu 消息是否需要包含 trace_id 短链接？
