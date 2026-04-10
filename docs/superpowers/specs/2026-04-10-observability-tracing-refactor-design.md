# Observability 日志系统重构设计

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 使用 `tracing` + `tracing_appender` 替换当前手写的日志系统，简化代码并统一日志生态。

**Architecture:** 使用 `tracing_subscriber::Registry` 叠加两个 layer：stdout layer 输出人类可读格式，file layer 输出 JSONL 到 RollingFileAppender。run_id 去掉 `run_` 前缀，直接使用 UUID。

**Tech Stack:** tracing, tracing-appender, tracing-subscriber, tokio

---

## 1. 问题与动机

### 1.1 当前实现的问题

当前 `ObservabilityLogger` 使用手写日志逻辑：

```rust
// 当前实现
pub async fn log(&self, entry: LogEntry, log_type: LogType) {
    let json_line = entry.to_json_line();
    let stdout_line = entry.to_stdout_line();
    
    println!("{}", stdout_line);  // 手写 stdout 输出
    self.append_to_file(&file_path, &json_line).await;  // 手写文件追加
}
```

**问题：**
1. 手写 `println!()` 和文件追加逻辑，代码冗长
2. 缺少日志级别控制、采样等 tracing 生态功能
3. 文件句柄缓存使用 `HashMap` + `Mutex`，需要手写并发控制
4. 当前为修复"双倍 stdout 输出"问题，增加了 `log_to_file_only()` 方法，API 不够清晰

### 1.2 重构目标

| 目标 | 当前 | 重构后 |
|------|------|--------|
| stdout 输出 | `println!()` | `tracing::info!()` + fmt::layer |
| 文件输出 | 手写 `tokio::fs::OpenOptions` | `RollingFileAppender` |
| 并发控制 | 手写 `Mutex<HashMap>` | tracing-appender 内部处理 |
| 日志级别 | 无 | 支持 trace/debug/info/warn/error |
| 采样 | 无 | 可通过 layer 配置 |
| 代码行数 | ~150 行 | ~50 行 |

---

## 2. 设计详解

### 2.1 run_id 格式修改

**当前格式：**
```
run_abc123def456...
```

**修改后格式：**
```
abc123def456...
```

**修改位置：** `crates/vol-llm-agent/src/react/agent.rs:100`

```rust
// 当前
let run_id = format!("run_{}", uuid::Uuid::new_v4().simple());

// 修改后
let run_id = uuid::Uuid::new_v4().simple().to_string();
```

**影响范围：**
- `observability/logger.rs`: run log 文件名从 `run_{run_id}.jsonl` → `{run_id}.jsonl`
- `observability/cleanup.rs`: cleanup 逻辑需要适配新文件名（去掉 `run_` 前缀匹配）
- 测试用例：期望值需要更新

### 2.2 架构设计

```
┌─────────────────────────────────────────────────────────────┐
│          ObservabilityPlugin::listen(event, ctx)            │
│  → 将 event 转换为 tracing event                            │
│  → 使用 tracing::info!(event_data) 发射                     │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│           tracing_subscriber::Registry                      │
│  ┌─────────────────────┐  ┌───────────────────────────────┐ │
│  │  Layer 1 (stdout)   │  │     Layer 2 (file)            │ │
│  │  fmt::layer()       │  │  fmt::layer().json()          │ │
│  │  .with_writer(...)  │  │  .with_writer(file_appender)  │ │
│  │  → 人类可读格式     │  │  → JSONL 格式                 │ │
│  └─────────────────────┘  └───────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
                     │
                     ├─→ stdout (console)
                     │
                     └─→ RollingFileAppender
                         ├─ filename_prefix: "{run_id}" → runs/{run_id}.log
                         └─ filename_prefix: "session_{sid}_{date}" → sessions/...
```

### 2.3 核心实现

#### 2.3.1 ObservabilityLogger 重构

```rust
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::prelude::*;

pub struct ObservabilityLogger {
    agent_id: String,
    log_base_path: PathBuf,
    // 不再需要 file_cache - tracing-appender 内部管理
}

impl ObservabilityLogger {
    pub fn new(agent_id: String, log_base_path: PathBuf) -> Self {
        Self { agent_id, log_base_path }
    }

    /// 初始化 tracing subscriber，返回 guard（必须持有以确保非阻塞写入）
    pub fn init(&self) -> tracing_appender::non_blocking::WorkerGuard {
        // Run log appender - 不滚动
        let run_appender = RollingFileAppender::new(
            Rotation::NEVER,
            self.log_base_path.join("runs"),
            format!("{}", self.agent_id),  // 使用 agent_id 作为前缀
        );
        
        // Session log appender - 按天滚动
        let session_appender = RollingFileAppender::new(
            Rotation::DAILY,
            self.log_base_path.join("sessions"),
            format!("session_{}", self.agent_id),
        );
        
        // 创建非阻塞写入器
        let (run_non_blocking, run_guard) = tracing_appender::non_blocking(run_appender);
        let (session_non_blocking, session_guard) = tracing_appender::non_blocking(session_appender);
        
        // 创建 layers
        let stdout_layer = tracing_subscriber::fmt::layer()
            .with_writer(std::io::stdout);
            
        let file_layer = tracing_subscriber::fmt::layer()
            .json()
            .with_writer(run_non_blocking);
        
        // 注册 subscriber
        tracing_subscriber::registry()
            .with(stdout_layer)
            .with(file_layer)
            .init();
        
        // 返回 guard - 调用者必须持有直到日志完成
        run_guard  // 实际需要考虑多个 guard 的生命周期管理
    }

    /// 记录事件 - 使用 tracing macro
    pub fn log(&self, event: &AgentStreamEvent, ctx: &PluginContext) {
        // 将 event 转换为 tracing event
        match event {
            AgentStreamEvent::AgentStart { input } => {
                tracing::info!(
                    run_id = %ctx.run_id,
                    session_id = %ctx.session_id,
                    event = "AgentStart",
                    input = %input,
                    "Agent started - input: {:?}", input
                );
            }
            AgentStreamEvent::ToolCallBegin { tool_name, arguments } => {
                tracing::info!(
                    run_id = %ctx.run_id,
                    session_id = %ctx.session_id,
                    event = "ToolCallBegin",
                    tool_name = %tool_name,
                    "Tool call: {} - {}", tool_name, arguments
                );
            }
            // ... 其他事件类型
        }
    }
}
```

#### 2.3.2 按 run_id/session_id 分离文件

关键：在每次记录事件时，根据 event 类型和目标文件动态选择 writer。

**方案 A：使用 `Span` 绑定 run_id**
```rust
let span = tracing::info_span!("agent_run", run_id = %ctx.run_id);
span.in_scope(|| {
    tracing::info!(event = "AgentStart", ...);
});
```
但此方案不能动态切换文件（run vs session）。

**方案 B：自定义 `MakeWriter`（推荐）**
```rust
struct RunLogWriter {
    base_path: PathBuf,
    run_id: String,
}

impl<'a> MakeWriter<'a> for RunLogWriter {
    type Writer = NonBlocking;
    fn make_writer(&'a self) -> Self::Writer {
        let appender = RollingFileAppender::new(
            Rotation::NEVER,
            self.base_path.join("runs"),
            &self.run_id,  // filename_prefix
        );
        let (non_blocking, _guard) = tracing_appender::non_blocking(appender);
        non_blocking
    }
}
```

**问题**：`MakeWriter` 在 init 时创建，不能在每次 log 时动态改变。

**方案 C：使用 `dynamic_link` 或 `appender::Appender`（最佳方案）**

实际上，对于本需求，每个 agent run 应该有自己的 tracing subscriber。更好的设计是：

```rust
// 在 ObservabilityPlugin::listen() 中，每个事件写入两个文件
// 使用两个独立的 appender，在 log() 中分别写入
```

但 tracing 的设计是全局 subscriber，不适合这种"每条 log 路由到不同文件"的场景。

### 2.4 最终设计：简化方案

考虑到 tracing 的限制，采用**简化方案**：

1. **Run log 和 Session log 合并**：所有事件写入同一个文件，但 JSON 中包含 run_id 和 session_id 字段
2. **按 agent_id 组织目录**：`logs/agents/{agent_id}/events.jsonl`
3. **使用 `RollingFileAppender` 按天滚动**：自动管理文件大小

或者，保持当前分离文件的逻辑，但**不使用 tracing**：

- stdout: 使用 `tracing::info!()` 输出
- 文件：保持当前手写逻辑（已修复双倍输出问题）

这是最务实的方案。

---

## 3. 修正后的设计（混合方案）

考虑到实际需求（按 run_id/session_id 分离文件）与 tracing 的设计哲学不完全匹配，采用混合方案：

### 3.1 架构

```
┌─────────────────────────────────────────────────────────────┐
│          ObservabilityPlugin::listen(event, ctx)            │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ├─────────────────────────────────┐
                     ▼                                 ▼
         ┌─────────────────────┐         ┌─────────────────────┐
         │   stdout 输出       │         │   文件输出（手写）  │
         │  tracing::info!()   │         │  保持当前逻辑       │
         │  fmt::layer()       │         │  (已修复双倍问题)   │
         └─────────────────────┘         └─────────────────────┘
```

### 3.2 run_id 格式修改

```rust
// crates/vol-llm-agent/src/react/agent.rs:100
let run_id = uuid::Uuid::new_v4().simple().to_string();  // 去掉 run_ 前缀
```

### 3.3 清理逻辑更新

```rust
// crates/vol-llm-agent/src/observability/cleanup.rs
// run log 文件名：{run_id}.jsonl（无 run_ 前缀）
// 保留最近的 N 个 run log
```

### 3.4 stdout 输出格式（可选）

配置 tracing subscriber 输出结构化日志：

```rust
// 在 main.rs 或测试初始化中
tracing_subscriber::fmt()
    .with_max_level(tracing::Level::INFO)
    .with_target(false)  // 不输出 target
    .init();
```

---

## 4. 实施计划

1. **修改 run_id 生成** - agent.rs:100
2. **更新 cleanup 逻辑** - cleanup.rs，适配新文件名
3. **更新测试用例** - 所有期望 `run_` 前缀的断言
4. **（可选）配置 tracing subscriber** - 添加结构化 stdout 输出

---

## 5. 验收标准

1. ✅ run_id 格式为纯 UUID（如 `abc123...`），无 `run_` 前缀
2. ✅ run log 文件名：`{run_id}.jsonl`
3. ✅ session log 文件名：`session_{session_id}_{date}.jsonl`
4. ✅ stdout 每条日志只输出一次
5. ✅ 所有测试通过
6. ✅ cleanup 逻辑正常工作

---

## 6. 未来增强（可选）

1. **完全迁移到 tracing**：如果未来需要按天滚动、日志级别过滤等功能
2. **添加采样**：减少高流量时的日志量
3. **OTLP 导出**：发送到 Jaeger/Tempo 等 tracing 后端
