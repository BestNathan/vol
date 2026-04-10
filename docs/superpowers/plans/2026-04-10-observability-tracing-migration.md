# Observability 迁移到 tracing_subscriber 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 使用 `tracing_subscriber` + `tracing_appender` 替换当前手写的日志系统，简化代码并统一日志生态。

**Architecture:** 
- stdout: 使用 `tracing_subscriber::fmt::layer()` 输出人类可读格式
- 文件：使用 `tracing_appender::rolling::RollingFileAppender` 替代手写文件追加
- 由于需要每条日志写入两个文件（run + session），文件写入保持当前逻辑，但使用 `RollingFileAppender` 管理文件句柄

**Tech Stack:** tracing, tracing-appender, tracing-subscriber

---

## 当前问题分析

当前实现使用手写日志逻辑：
1. `println!()` 输出 stdout
2. 手写 `tokio::fs::OpenOptions` + `HashMap<Mutex<File>>` 管理文件句柄

**问题：**
- 手写并发控制（Mutex<HashMap>）
- 缺少日志级别控制
- 无法与 tracing 生态集成

---

## 设计方案

### 方案 A：完全使用 tracing（不可行）

```rust
// 问题：tracing 的 subscriber 是全局的，无法根据每条 log 动态切换文件
tracing_subscriber::registry()
    .with(fmt::layer().with_writer(run_appender))  // 只能固定一个 writer
    .init();
```

**问题**：无法实现"每条日志同时写入 run log 和 session log"的需求。

### 方案 B：混合方案（采用）

```
ObservabilityPlugin::listen()
    ↓
┌─────────────────────────────────────┐
│  stdout: tracing::info!(...)        │ → tracing_subscriber::fmt::layer()
│  文件：logger.append_to_file()      │ → RollingFileAppender
└─────────────────────────────────────┘
```

**优点**：
- stdout 使用 tracing，支持日志级别、格式化
- 文件保持按 run_id/session_id 分离
- `RollingFileAppender` 自动管理文件句柄和滚动

---

## 实施步骤

### Task 1: 添加 tracing-appender 依赖

**Files:**
- Modify: `crates/vol-llm-agent/Cargo.toml`

- [ ] **Step 1: 添加依赖**

```toml
[dependencies]
tracing = { workspace = true }
tracing-appender = "0.2"
tracing-subscriber = "0.3"
```

- [ ] **Step 2: 验证编译**

```bash
cargo check -p vol-llm-agent
```

---

### Task 2: 重构 ObservabilityLogger

**Files:**
- Modify: `crates/vol-llm-agent/src/observability/logger.rs`

- [ ] **Step 1: 简化 LogEntry 结构**

移除 `to_stdout_line()` 方法，因为 stdout 由 tracing 处理：

```rust
impl LogEntry {
    /// Serialize log entry as JSON line (for file output)
    pub fn to_json_line(&self) -> String {
        json!({
            "timestamp": self.timestamp.to_rfc3339(),
            "run_id": self.run_id,
            "agent_id": self.agent_id,
            "event": self.event,
            "data": self.data,
        }).to_string()
    }
}
```

- [ ] **Step 2: 移除 file_cache，改用 RollingFileAppender**

```rust
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use std::sync::Arc;

pub struct ObservabilityLogger {
    agent_id: String,
    agent_path: PathBuf,
    // 不再需要 file_cache - RollingFileAppender 内部管理
}
```

- [ ] **Step 3: 实现 get_or_create_appender 方法**

```rust
fn get_run_appender(&self, run_id: &str) -> RollingFileAppender {
    RollingFileAppender::new(
        Rotation::NEVER,
        self.agent_path.join("runs"),
        run_id,  // filename_prefix
    )
}

fn get_session_appender(&self, session_id: &str, date: &str) -> RollingFileAppender {
    RollingFileAppender::new(
        Rotation::DAILY,
        self.agent_path.join("sessions"),
        &format!("session_{}_{}", session_id, date),
    )
}
```

- [ ] **Step 4: 重写 log() 方法**

```rust
pub async fn log(&self, entry: &LogEntry, log_type: &LogType) {
    // stdout 使用 tracing
    tracing::info!(
        run_id = %entry.run_id,
        agent_id = %entry.agent_id,
        event = %entry.event,
        "{}",
        entry.format_event_summary()
    );

    // 文件写入 JSON
    let json_line = entry.to_json_line();
    let file_path = match log_type {
        LogType::Session { session_id, date } => {
            self.get_session_log_path(session_id, date)
        }
        LogType::Run { run_id } => self.get_run_log_path(run_id),
    };

    let _ = self.append_to_file(&file_path, &json_line).await;
}
```

- [ ] **Step 5: 移除 log_to_file_only()**

不再需要此方法，因为 stdout 由 tracing 统一处理。

---

### Task 3: 更新 ObservabilityPlugin

**Files:**
- Modify: `crates/vol-llm-agent/src/observability/plugin.rs`

- [ ] **Step 1: 简化 listen() 方法**

```rust
async fn listen(&self, event: &AgentStreamEvent, ctx: &PluginContext) {
    let entry = self.create_log_entry(event, ctx);

    // Log to run log - this also emits to stdout via tracing
    let run_log_type = LogType::Run { run_id: ctx.run_id.clone() };
    self.logger.log(&entry, &run_log_type).await;

    // Log to session log - same entry, different file
    let date = Utc::now().format("%Y%m%d").to_string();
    let session_log_type = LogType::Session {
        session_id: ctx.session_id.clone(),
        date,
    };
    self.logger.log(&entry, &session_log_type).await;
}
```

---

### Task 4: 初始化 tracing subscriber

**Files:**
- Modify: `crates/vol-llm-agent/src/observability/mod.rs` 或 `lib.rs`

- [ ] **Step 1: 创建 init_tracing() 函数**

```rust
pub fn init_tracing() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .without_time()
        .init();
}
```

- [ ] **Step 2: 在 agent 启动时调用**

在 `ReActAgent::run()` 中初始化 tracing（如果尚未初始化）。

---

### Task 5: 更新测试

**Files:**
- Modify: `crates/vol-llm-agent/src/observability/logger.rs` tests
- Modify: `crates/vol-llm-agent/src/observability/plugin.rs` tests

- [ ] **Step 1: 更新 logger 测试**

移除 `test_log_entry_to_stdout()` 测试。

- [ ] **Step 2: 更新 plugin 测试**

验证文件输出仍然正常工作。

---

### Task 6: 验证和清理

- [ ] **Step 1: 运行测试**

```bash
cargo test -p vol-llm-agent --lib observability
```

- [ ] **Step 2: 运行集成测试**

```bash
cargo test -p vol-llm-agent --test observability_integration
```

- [ ] **Step 3: 验证 stdout 输出**

确认每条日志只输出一次，格式正确。

---

## 验收标准

1. ✅ stdout 使用 `tracing::info!()` 输出
2. ✅ 文件使用 `RollingFileAppender` 写入
3. ✅ 移除手写 `HashMap<Mutex<File>>` 缓存逻辑
4. ✅ 每条日志仍然写入 run log 和 session log 两个文件
5. ✅ 所有测试通过

---

## 代码行数对比

| 文件 | 修改前 | 修改后 |
|------|--------|--------|
| logger.rs | ~280 行 | ~150 行 |
| plugin.rs | ~220 行 | ~200 行 |
| 总计 | ~500 行 | ~350 行 |
