# Requirements: K8s-Native LGTM Observability via OpenTelemetry

## Background

当前 LLM agent 服务使用自研的 HTTP batch sender + vol-observability 服务写入 Loki 和 TDengine。需要迁移到标准的 OpenTelemetry 可观测栈（Loki + Grafana + Tempo + Prometheus），运行在 K8s 集群中，实现日志、指标、Tracing 三支柱的统一采集和可视化。

现有 AgentPlugin 插件系统的 `listen()` hook 天然适合 OTel 接入——它接收完整的 `AgentStreamEvent`，可以异步映射为 OTel log/metric/span，无需修改 ReActAgent 核心循环。

## Goals

1. **统一 OTel 采集** — Agent 插件通过 OTLP 协议向 K8s 集群内的 OTel Collector 发送日志、指标、Tracing 数据
2. **混合 Tracing 模型** — Span 记录粗粒度链路（AgentRun → LLM Call → ToolCall），详细日志（thinking/content delta）写入 Loki，通过 `trace_id` 关联查询
3. **全维度 Metrics** — 收集 Agent 运行、LLM 调用、Tool 执行四类指标（总计 + histogram + tokens）
4. **对象存储后端** — Loki 和 Tempo 使用 S3/MinIO 作为持久化存储
5. **Grafana 统一看板** — Prometheus + Loki + Tempo 数据源统一配置，可关联查询

## Non-Goals

1. **不改造 vol-monitor 监控系统** — vol-monitor 的现有告警链路（Deribit → TDengine）保持不变
2. **不实现自定义 Alert Rules** — Prometheus 只负责采集和展示，告警规则后续单独做
3. **不替换现有 Loki/TDengine 双写逻辑** — 新的 OTel 链路与旧的 Loki/TDengine 链路并存，后续再决定是否统一
4. **不采集非 LLM agent 服务** — 本次只处理 vol-llm-agent 系列服务（CodingAgent、WikiAgent、YamlAgent 等）

## Scope

### In Scope

| 组件 | 变更内容 |
|------|----------|
| `vol-llm-observability` | 新增 OTel Plugin 实现，使用 opentelemetry-rust SDK 发送数据 |
| `vol-llm-agent` | 新增 OTelAgentPlugin wrapper，通过 `listen()` hook 接入 |
| `vol-llm-agents` | CodingAgent 支持 OTel 配置 |
| `vol-llm-yaml-agent` | YAML 配置支持 OTel 插件 |
| K8s manifests | OTel Collector Deployment/Service、Grafana 数据源配置、Loki/Tempo/Prometheus 带 S3 后端的部署配置 |
| Grafana dashboards | 更新现有 dashboard 使用 Prometheus/Loki/Tempo 数据源 |

### Out of Scope

- vol-monitor 二进制及其 K8s 部署
- Prometheus AlertManager 配置
- 非 agent 服务的可观测性
- TDengine 的替换

## Constraints

1. **OTel 部署模式** — In-cluster 集中式 OTel Collector Deployment，agent 通过 K8s Service（如 `otel-collector.default.svc.cluster.local:4318`）连接
2. **存储后端** — S3 兼容对象存储（MinIO 或已有 S3），Loki 和 Tempo 原生支持
3. **协议** — OTLP/HTTP（端口 4318），非 gRPC（4317），简化网络配置
4. **现有架构保留** — 新的 OTel Plugin 和现有的 ObservabilityPlugin 并存，通过配置开关控制

## Success Criteria

1. `cargo build --workspace` 无新增编译错误（允许现有 warning）
2. 新增 `OtelPlugin` 实现，注册到 `PluginRegistry` 后可接收所有 `AgentStreamEvent`
3. AgentRun 产生至少 3 级 Span（AgentRun → LLM Call → ToolCall），可在 Grafana Tempo 数据源中查看完整 trace tree
4. 每个 Span 带有 `trace_id`，相同 agent run 的所有 logs（Loki）和 spans（Tempo）可通过该 ID 关联
5. Prometheus 可查询到以下指标：
   - `agent_run_total`（counter, by agent_id, status）
   - `llm_calls_total`（counter, by model, tool_name）
   - `llm_latency_seconds`（histogram, buckets: 0.1, 0.5, 1, 2, 5, 10, 30）
   - `llm_tokens_total`（counter, by type=input/output）
   - `tool_calls_total`（counter, by tool_name, status）
   - `tool_duration_seconds`（histogram, by tool_name）
6. Loki 可查询到 thinking/content delta 的详细日志，包含 `trace_id` 标签
7. K8s 部署配置可一键拉起 OTel Collector + Loki + Tempo + Prometheus + Grafana（带 S3 后端）
8. Grafana dashboard 可直接使用，无需手动配置数据源

## Edge Cases

1. **OTel Collector 不可达** — Plugin 应降级为本地日志写入（fallback 到现有 LoggerPlugin），不阻塞 agent 运行
2. **Agent 并发运行** — 每个 agent run 生成独立的 `trace_id`，span 不互相污染
3. **长运行 Agent** — 单次 agent run 可能持续数小时（如 CodingAgent 多次 iteration），Span 不应超时截断
4. **空 ToolCall 结果** — 工具返回空字符串或超大结果（>1MB）时，截断后存入 span attributes
5. **LLM 流式输出中断** — ThinkingDelta 流式中断时，span 仍应标记为 complete，记录部分结果

## Architecture Summary

```
┌─────────────────────────────────────────────────────────────┐
│  LLM Agent Pod                                              │
│                                                             │
│  ┌─────────────┐  listen()  ┌───────────────────┐           │
│  │ ReActAgent  │ ─────────→ │ OTelAgentPlugin   │           │
│  │             │            │ (opentelemetry-rust)│          │
│  │ AgentStream │            │                   │           │
│  │ Events      │            │ → TraceProvider   │           │
│  │             │            │ → MeterProvider   │           │
│  │             │            │ → LoggerProvider  │           │
│  └─────────────┘            └────────┬──────────┘           │
└──────────────────────────────────────┼──────────────────────┘
                                       │ OTLP/HTTP (4318)
                                       ▼
┌─────────────────────────────────────────────────────────────┐
│  K8s Cluster                                                │
│                                                             │
│  ┌────────────────┐                                         │
│  │ OTel Collector │                                         │
│  │   receivers: otlp                                       │
│  │   processors: batch, memory_limiter                     │
│  │   exporters:                                           │
│  │     → Prometheus (metrics)                              │
│  │     → Loki (logs)                                      │
│  │     → Tempo (traces)                                   │
│  └────────┬──────────┬──────────┬──────────┘              │
│           │          │          │                          │
│     ┌─────▼───┐ ┌───▼──┐ ┌───▼────┐                      │
│     │Prometheus│ │ Loki │ │ Tempo  │                      │
│     │          │ │ +S3  │ │ +S3    │                      │
│     └─────────┘ └──────┘ └────────┘                      │
│           │          │          │                          │
│           └──────────┼──────────┘                          │
│                      ▼                                     │
│              ┌──────────────┐                              │
│              │   Grafana    │                              │
│              │ (auto-prov)  │                              │
│              └──────────────┘                              │
└─────────────────────────────────────────────────────────────┘
```

## Event-to-OTel Mapping

| AgentStreamEvent | OTel Type | Span Name | Attributes |
|------------------|-----------|-----------|------------|
| AgentStart | Span (root) | `agent.run` | `agent.id`, `agent.type`, `session.id` |
| AgentComplete | Span end | - | `response` |
| LLMCallStart | Span (child) | `llm.call` | `llm.model`, `llm.iteration` |
| LLMCallComplete | Span end | - | `llm.usage.input_tokens`, `llm.usage.output_tokens`, `llm.latency_ms` |
| ToolCallBegin | Span (child) | `tool.call` | `tool.name`, `tool.call_id` |
| ToolCallComplete | Span end | - | `tool.status=ok`, `tool.duration_ms` |
| ToolCallError | Span end | - | `tool.status=error`, `tool.error` |
| ThinkingDelta | Log | - | `content.delta` (truncated) |
| ContentDelta | Log | - | `content.delta` (truncated) |
| IterationComplete | - | Metrics: `agent.iterations_total` | - |

## Open Questions

无（所有问题已在澄清过程中确认）。
