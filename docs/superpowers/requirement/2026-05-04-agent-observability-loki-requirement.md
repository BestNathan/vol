# Requirements: Agent Observability → Loki Integration

## Background

当前 agent 的可观测性通过 `vol-llm-observability` crate 的 `LoggerPlugin` 将结构化事件写入本地 JSONL 文件。这种方案只能在本地查看，缺乏查询分析能力。项目已有 LGTM（Loki + Grafana + Tempo + Mimir/Prometheus）基础设施部署在 `observability` 命名空间下，希望将 agent 运行日志统一接入。

## Goals

1. **双写模式**：保持本地 JSONL 日志不变，新增 Loki 上报插件，同一份事件同时写入两个后端。
2. **Loki 可查询**：核心查询场景为按 `session_id` 和 `run_id` 查询。通过 Loki Labels（`namespace`、`agent`、`agent_id`）缩小范围，结合 LogQL 行过滤器按 session/run 追溯。
3. **预留扩展点**：日志中携带 `trace_id` 字段，为后续 Tempo（链路追踪）和 Prometheus（聚合指标）接入预留关联能力。
4. **配置灵活**：支持环境变量优先、TOML 配置 fallback 的方式读取 Loki endpoint，未配置时不启用 Loki 上报。

## Non-Goals

1. **不实现容错/降级**：Loki 未配置或禁用时仅写本地日志；Loki 写失败不重试、不降级，直接失败。
2. **不实现 Tempo/Traces 和 Prometheus/Metrics**：本次只做 Logs → Loki，但数据结构预留 `trace_id` 字段。
3. **不修改本地日志格式**：本地 JSONL 格式保持不变。

## Scope

### Included
- 在 `vol-llm-observability` crate 中新增 `LokiPlugin`（实现 `AgentPlugin` trait），复用与 `LoggerPlugin` 相同的事件流。
- Loki 上报通过 HTTP POST 到 `http://loki.observability.svc.cluster.local:3100/loki/api/v1/push`（Loki Push API），批量写入。
- 环境变量 `LOKI_URL` 优先，TOML 配置 `[observability.loki]` 作为 fallback。
- `CodingAgent` builder 新增 `.with_loki()` 方法注册插件。
- Loki Labels 设计（低基数）：

  | Label | 示例值 | 说明 |
  |-------|--------|------|
  | `namespace` | `"agent"` | 固定值，标识数据来源 |
  | `agent` | `"coding"` / `"advice"` / `"qa"` / `"ppt"` | agent 类型 |
  | `agent_id` | `"user-xyz"` 等 | agent 实例标识 |

- Log Line 内部字段（高基数，行过滤器查询）：

  | 字段 | 查询方式 |
  |------|----------|
  | `session_id` | `{namespace="agent"} \|= "session-abc"` |
  | `run_id` | `{namespace="agent"} \|= "run-def-456"` |
  | `event` | `{namespace="agent"} \|= "ToolCall"` |
  | `tool_name` | `{namespace="agent"} \|= "bash"` |
  | `model` | LLM 模型名 |
  | `trace_id` | 预留，后续关联 Tempo |

### Excluded
- Tempo 链路追踪接入
- Prometheus 指标采集
- 日志采集失败重试机制
- 已有 `observability-service` worktree 中的 Loki writer 代码复用（当前工作基于 master 分支从零实现）

## Constraints

- **Loki endpoint**：`http://loki.observability.svc.cluster.local:3100`（可通过配置覆盖）
- **认证**：不需要认证
- **事件过滤**：复用 `LoggerPlugin::should_log()` 逻辑，跳过高频 delta 事件（`ThinkingDelta`、`ContentDelta`、`ToolCallArgumentDelta`）
- **现有插件架构**：必须复用 `AgentPlugin` trait 的 `listen()` 钩子，保持与 `LoggerPlugin` 一致的集成模式

## Success Criteria

1. **双写正确**：agent 每次运行的事件同时出现在本地 JSONL 和 Loki 中。
2. **Loki 可查询**：通过 LogQL `{namespace="agent", agent="coding"}` 能查询到 agent 事件；按 `session_id` 和 `run_id` 的行过滤器能追溯单次运行的完整事件流。
3. **未配置不启用**：`LOKI_URL` 和 TOML 均未配置时，`with_loki()` 不注册 Loki 插件，agent 正常运行仅写本地日志。
4. **预留 trace_id**：每条 Loki 日志 entry 携带 `trace_id` 字段（初始为空字符串），为后续 Tempo 关联做准备。

## Edge Cases

| Edge Case | Handling |
|-----------|----------|
| `LOKI_URL` 未设置且 TOML 无配置 | 不注册 Loki 插件，仅写本地日志 |
| Loki 服务不可达（HTTP 失败） | 直接失败，不重试，不降级，tracing error 记录 |
| 空 event 数据 | 照常写入，不做特殊处理 |

## Open Questions

- Loki batch size 和 flush interval 的默认值？（建议 batch_size=50, flush_interval=1000ms，可后续调优）
