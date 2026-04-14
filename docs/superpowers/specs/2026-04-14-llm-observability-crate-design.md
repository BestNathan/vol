# vol-llm-observability — 可观测性 crate 设计

**创建日期**: 2026-04-14
**状态**: 设计中

---

## 1. 动机

当前可观测性代码散落在 `vol-llm-agent/src/observability/` 和 `vol-llm-agent/src/plugins/observability.rs`（旧版本，AuditEvent 通道）中。需要：

1. **提取为独立 crate** — 解耦 agent 核心循环，可独立复用
2. **移除旧代码** — `plugins/observability.rs` 是重复实现，已废弃
3. **新增 TTFT / Tool 指标** — 基于事件时序自动计算
4. **新增 Tracing Spans** — 在关键事件点创建 tracing span，携带 run_id、timing 等属性

---

## 2. 架构

### 2.1 依赖关系

```
vol-llm-agent ──depends──> vol-llm-observability
vol-llm-observability ──depends──> vol-llm-core (AgentStreamEvent, AgentPlugin trait)
                                 vol-llm-agent (PluginContext, PluginDecision)
                                 tokio, serde_json, chrono, tracing
```

### 2.2 数据流

```
AgentStreamEvent
    │
    ├── intercept() ──▶ TTFT 计时起点 (LLMCallStart)
    │                   Tool 执行计时起点 (ToolCallBegin)
    │                   Tracing Span 创建
    │
    └── listen()  ──▶ MetricsCollector 聚合
                      RunLogLogger 写入 JSONL
                      Tracing Span 关闭/事件记录
                      AgentComplete 时输出汇总
```

### 2.3 模块边界

```
crates/vol-llm-observability/
├── Cargo.toml
├── src/
│   ├── lib.rs              → 公开 API: ObservabilityPlugin, ObservabilityConfig
│   ├── plugin.rs           → ObservabilityPlugin: AgentPlugin 实现
│   ├── config.rs           → 配置: log 开关、metrics 开关、保留策略
│   ├── metrics/
│   │   ├── mod.rs          → MetricsCollector 入口
│   │   ├── state.rs        → 运行时状态: LLM call start time, tool start times
│   │   └── summary.rs      → AgentComplete 时输出汇总 (tracing::info!)
│   ├── tracing/
│   │   ├── mod.rs          → span 工厂函数
│   │   └── spans.rs        → agent_run, llm_call, tool_call span 创建
│   └── run_log/
│       ├── mod.rs          → RunLogLogger + LogEntry + cleanup 复用现有代码
│       ├── logger.rs
│       └── cleanup.rs
```

---

## 3. 核心类型

### 3.1 ObservabilityConfig

```rust
#[derive(Debug, Clone)]
pub struct ObservabilityConfig {
    /// Whether to record events to JSONL run logs
    pub enable_run_log: bool,
    /// Whether to collect and output metrics
    pub enable_metrics: bool,
    /// Base path for log files
    pub log_base_path: String,
    /// Agent identifier
    pub agent_id: String,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            enable_run_log: true,
            enable_metrics: true,
            log_base_path: "logs".into(),
            agent_id: "default".into(),
        }
    }
}
```

### 3.2 MetricsCollector

纯状态结构，不依赖文件系统。通过事件驱动更新：

```rust
pub struct MetricsCollector {
    run_id: String,
    // TTFT
    llm_call_start: Option<Instant>,
    thinking_start: Option<Instant>,
    ttft_values: Vec<Duration>,
    // Tool latency
    tool_start_times: HashMap<String, Instant>,
    tool_latencies: Vec<(String, Duration)>,
    // Token usage
    total_prompt_tokens: u32,
    total_completion_tokens: u32,
    // Iteration
    iteration_count: u32,
    total_tool_calls: u32,
}
```

**事件映射：**

| Event | Action |
|-------|--------|
| `LLMCallStart` | 记录 `llm_call_start = Instant::now()` |
| `ThinkingStart` | 如果有 `llm_call_start`，计算 TTFT 并记录 |
| `ContentStart` | 如果没有 thinking，用 `llm_call_start` 计算 TTFT |
| `ToolCallBegin` | 记录 `tool_start_times[tool_call_id] = Instant::now()` |
| `ToolCallComplete` | 如果有 start time，计算 latency 并记录 |
| `LLMCallComplete` | 累加 token usage |
| `IterationComplete` | 递增 iteration count |
| `AgentComplete` / `AgentAborted` | 输出汇总 |

### 3.3 Tracing Spans

通过 `tracing` crate 创建结构化 span：

```rust
// Agent 级 span — 整个 run() 周期
let agent_span = tracing::info_span!(
    "agent_run",
    run_id = %run_id,
    agent_id = %agent_id,
);

// LLM 调用 span — 嵌套在 agent span 内
let llm_span = tracing::info_span!(
    "llm_call",
    run_id = %run_id,
    iteration = iteration,
);

// 工具调用 span — 嵌套在 agent span 内
let tool_span = tracing::info_span!(
    "tool_call",
    run_id = %run_id,
    tool_name = %tool_name,
    tool_call_id = %tool_call_id,
);
```

**Span 生命周期：**

| Event | Span Action |
|-------|-------------|
| `AgentStart` | 创建 `agent_run` span，进入 |
| `LLMCallStart` | 创建 `llm_call` span，进入 |
| `LLMCallComplete/Error` | 关闭 `llm_call` span |
| `ToolCallBegin` | 创建 `tool_call` span，进入 |
| `ToolCallComplete/Error/Skipped` | 关闭 `tool_call` span |
| `AgentComplete/Aborted` | 关闭 `agent_run` span，输出 metrics 汇总 |

**实现方式：**

由于 `ObservabilityPlugin` 的 `listen()` 是 fire-and-forget 的 `tokio::spawn`，不能直接持有 span guard。改用以下方案：

- 使用 `tracing::Span::current()` 获取当前 span，通过 `span.record()` 添加属性
- Span 的创建/关闭通过 `tracing::event!` 标记边界
- 实际的 span 生命周期由 agent loop 管理（通过 `RunContext` 上的方法），observability 只是 enrich 属性

或者更简单的方式：
- `intercept()` 中 emit tracing events（`tracing::info!` with span context）
- `listen()` 中记录事件详情
- 不尝试在 plugin 中创建完整的 span 生命周期（这需要状态传递，plugin 架构不支持）

采用简化方案：
```rust
// intercept: 标记 span 边界
tracing::info_span!("llm_call", run_id=%..., iteration=%...).in_scope(|| {
    // span is entered/exited here
});

// listen: 在 span 内记录事件
tracing::debug!(
    run_id = %...,
    event = "thinking_delta",
    "LLM thinking delta"
);
```

### 3.4 ObservabilityPlugin

```rust
pub struct ObservabilityPlugin {
    config: ObservabilityConfig,
    metrics: MetricsCollector,      // 如果 enable_metrics
    logger: Option<RunLogLogger>,   // 如果 enable_run_log
}

impl AgentPlugin for ObservabilityPlugin {
    fn id(&self) -> &str { "observability" }
    fn priority(&self) -> u8 { 10 } // 低优先级，不阻塞其他插件

    fn intercept(&self, ctx: &PluginContext) -> PluginDecision {
        // 在 intercept 中创建 tracing spans
        // 这是 fire-and-forget 之外的同步点
        match &ctx.last_event {
            AgentStreamEvent::LLMCallStart { iteration } => {
                let span = tracing::info_span!("llm_call",
                    run_id = %ctx.run_id,
                    iteration = iteration,
                );
                span.in_scope(|| tracing::info!("LLM call starting"));
            }
            AgentStreamEvent::ToolCallBegin { tool_name, .. } => {
                let span = tracing::info_span!("tool_call",
                    run_id = %ctx.run_id,
                    tool_name = tool_name,
                );
                span.in_scope(|| tracing::info!("Tool call starting"));
            }
            _ => {}
        }

        // 更新 metrics 状态（同步操作）
        if self.config.enable_metrics {
            self.metrics.on_event(&ctx.last_event);
        }

        PluginDecision::Continue
    }

    fn listen(&self, event: &AgentStreamEvent, ctx: &PluginContext) {
        // 异步：写入 run log
        if let Some(ref logger) = self.logger {
            logger.log_event(event);
        }

        // 更新 metrics（异步，可能在 agent 结束后才到达）
        if self.config.enable_metrics {
            self.metrics.on_event(event);

            // 在 terminal event 时输出汇总
            if matches!(event, AgentStreamEvent::AgentComplete | AgentStreamEvent::AgentAborted { .. }) {
                self.metrics.output_summary(&ctx.run_id);
            }
        }
    }
}
```

---

## 4. Tracing 输出示例

```
2026-04-14T10:00:00Z  INFO agent_run{run_id=run_abc agent_id=coding}: LLM call starting
2026-04-14T10:00:01Z  INFO agent_run{run_id=run_abc agent_id=coding}: Tool call starting tool_name=bash
2026-04-14T10:00:01Z DEBUG agent_run{run_id=run_abc agent_id=coding}: Tool call completed tool_name=bash duration_ms=500
2026-04-14T10:00:02Z  INFO agent_run{run_id=run_abc agent_id=coding}: === Agent Run Summary ===
2026-04-14T10:00:02Z  INFO agent_run{run_id=run_abc agent_id=coding}: TTFT: avg=1.2s min=0.8s max=1.5s
2026-04-14T10:00:02Z  INFO agent_run{run_id=run_abc agent_id=coding}: Tool Latency: avg=0.5s calls=3
2026-04-14T10:00:02Z  INFO agent_run{run_id=run_abc agent_id=coding}: Token Usage: prompt=1200 completion=800
```

---

## 5. 要删除的文件

| 文件 | 原因 |
|------|------|
| `crates/vol-llm-agent/src/plugins/observability.rs` | 旧版 AuditEvent 通道实现，被 `observability/` 版本取代 |
| `crates/vol-llm-agent/src/plugins/mod.rs` 中的 `pub mod observability;` | 同上 |

---

## 6. 向后兼容

- `AgentBuilder::with_observability_plugin()` 保持不变，只是内部使用新 crate
- `ObservabilityPlugin::new(agent_id, log_base_path)` 保持相同签名
- 新增 `ObservabilityConfig` builder 支持可选配置
- 现有测试和文档只需更新 import 路径

---

## 7. 实施步骤概要

1. 创建 `vol-llm-observability` crate 骨架
2. 移动 `run_log/` 目录到新 crate
3. 实现 `MetricsCollector`（TTFT + tool latency）
4. 实现 tracing spans 集成
5. 重写 `ObservabilityPlugin` 使用新组件
6. 删除 `plugins/observability.rs`
7. 更新 `vol-llm-agent` 依赖和 import
8. 验证所有测试通过
