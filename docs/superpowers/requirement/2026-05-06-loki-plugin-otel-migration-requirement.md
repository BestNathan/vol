# Requirements: LokiPlugin → OTel SDK Migration

## Background

当前 `LokiPlugin` 通过自定义 `LokiWriter` 直接 HTTP POST 到 Loki Push API。项目已有 `opentelemetry` Rust SDK 和 `tracing-opentelemetry` layer 用于 traces，但 logs 未通过 OTel 层导出。目标是让 LokiPlugin 使用 Rust OTel SDK 统一对接 OTel Collector，不再直接调用 Loki HTTP 端点。

## Goals

1. **LokiPlugin 使用 OTel SDK 导出日志**：将现有 `LokiWriter`（HTTP POST）替换为 OTel log exporter，日志通过 `opentelemetry` Rust SDK 发送到 OTel Collector
2. **插件不感知端点**：LokiPlugin 本身不持有任何 URL/endpoint 配置，通过 `tracing::info!` 宏发送结构化日志，由 `tracing-opentelemetry` layer 自动转换为 OTel logs
3. **独立初始化步骤**：新增 `init_otel()` 函数，从环境变量读取 OTel Collector 端点并完成 OTel SDK 初始化，在 `main()` 或 agent 启动入口调用
4. **结构化日志字段**：每条日志携带结构化字段，不再使用 Loki labels 方式

## Non-Goals

1. **不修改 AgentPlugin trait 架构**：LokiPlugin 仍然实现 `AgentPlugin` 的 `listen()` 钩子，但 `listen()` 内部改为调用 `tracing::info!` 而非 HTTP 请求
2. **不改变 JSONL 本地日志**：`LoggerPlugin` 保持不变
3. **不实现 traces 导出**：本次仅做 logs，traces 已通过现有 OTel 层正常工作
4. **不引入 Loki 客户端代码**：移除 `loki/client.rs`、`loki/config.rs`、`loki/labels.rs` 等 HTTP 直连代码

## Scope

### Included

- **LokiPlugin 重构**：移除 `LokiWriter`、`LokiConfig`、`LokiLabels` 依赖。`listen()` 内部通过 `tracing::info!` 宏发送结构化日志，例如：
  ```rust
  tracing::info!(
      namespace = "agent",
      session_id = ctx.session_id,
      agent_id = agent_id,
      agent_type = agent_type,
      run_id = ctx.run_id,
      model = ctx.model,
      event = %serialized_event,
      "agent_event"
  );
  ```
- **OTel 初始化**：新增 `init_otel()` 函数，从环境变量读取 `OTEL_EXPORTER_OTLP_ENDPOINT` 等配置，设置 OTel log exporter。调用方：`main()` 或 agent 启动入口处，在 `tracing-subscriber` 初始化之前调用
- **结构化字段**：每条日志包含以下结构化字段：
  - `timestamp` — 事件时间戳（由 `tracing` 自动记录）
  - `session_id` — 会话 ID
  - `agent_id` — Agent 实例标识
  - `agent_type` — Agent 类型
  - `run_id` — 运行 ID
  - `model` — 当前轮次使用的模型（从 `RunContext` 获取）
  - `event` — 全量序列化的 `AgentStreamEvent` 变体内容
  - `namespace` — 固定值 `"agent"`，作为 OTel log attribute 设置（不再是 Loki label）
- **RunContext 扩展**：增加 `model: String` 字段，agent run 时从 LLM config 组装。**行为变化**：model 之前仅从 `LLMCallComplete` 事件中提取（只有该事件有 model），现在从 `RunContext` 获取，所有事件都携带 model 字段
- **Delta 事件过滤**：保留 `should_send()` 逻辑，跳过 `ThinkingDelta`、`ContentDelta`、`ToolCallArgumentDelta`

### Excluded

- OTel Collector 端部署或配置
- Tempo 链路追踪
- Prometheus 指标采集
- Loki HTTP 重试/降级机制

## Constraints

- **端点配置**：`OTEL_EXPORTER_OTLP_ENDPOINT` 环境变量优先，fallback 默认 `http://localhost:4317`
- **OTel SDK**：优先使用 `opentelemetry-otlp` crate 的 logs 模块，fallback 为 `tracing-opentelemetry` layer
- **Agent 身份**：agent_id、agent_type 仍从 `RunContext.config.def` 获取
- **model 字段**：从 `RunContext` 上的 `model` 字段获取，不再从 `LLMCallComplete` 事件中提取

## Success Criteria

1. LokiPlugin 不再持有 URL/endpoint 字段，不执行任何 HTTP 请求（grep `loki`、`http`、`url` 在 `loki/plugin.rs` 中无匹配）
2. 新增 `init_otel()` 函数可从环境变量读取端点并完成 OTel log exporter 初始化，有独立单元测试
3. 每条日志结构化字段完整：timestamp、session_id、agent_id、agent_type、run_id、model、event、namespace（检查 OTel collector 接收的日志属性）
4. `RunContext` 携带 `model` 字段，agent run 时正确赋值（单元测试验证）
5. 现有 LokiPlugin 测试场景全部通过：`test_plugin_id`、`test_plugin_priority`、`test_should_send_skips_delta_events`、`test_loki_entry_tool_call_has_all_fields`、`test_loki_entry_llm_complete_includes_model_label`、`test_loki_entry_plugin_event`、`test_loki_entry_fallback_no_agent_def`、`test_loki_entry_timestamp_uses_event_timestamp`、`test_loki_entry_llm_call_start_includes_messages`
6. `cargo test` 全量通过，无编译警告

## Edge Cases

| Edge Case | Handling |
|-----------|----------|
| OTel Collector 不可达 | OTel batch exporter 内置缓冲，不阻塞 agent 执行，tracing error 记录 |
| OTel 未初始化 | LokiPlugin 通过 `tracing::info!` 发送日志，走默认 console/file 层，不报错 |
| `RunContext.model` 为空 | 使用 `"unknown"` 作为 fallback |
| Agent 无 AgentDef | agent_id/agent_type 使用 `"unknown"`（现有行为） |
| 多个 LokiPlugin 克隆 | 不持有共享 writer，直接通过 `tracing` macros 发送，天然安全 |

## Open Questions

- batch size 和 flush interval 的默认值由 OTel SDK 的 `BatchLogProcessor` 决定，实现时确认默认值并记录
