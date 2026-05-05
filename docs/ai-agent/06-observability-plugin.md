# AI Agent - 可观测插件设计

**创建日期**: 2026-04-10  
**状态**: 已完成  
**作者**: vol-monitor team

---

## 1. 概述

### 1.1 设计目标

为 ReAct Agent 提供全面的可观测性能力，包括：

| 目标 | 说明 |
|------|------|
| **结构化日志** | JSONL 格式，易于解析和分析 |
| **双重输出** | 文件日志（完整 JSON）+ 标准输出（人类可读） |
| **自动轮转** | 会话日志按日期轮转，运行日志按 run_id 组织 |
| **保留策略** | 会话日志保留 7 天，运行日志保留最近 10 个 |
| **Agent 中心** | 日志按 agent_id 组织，支持多 Agent 日志隔离 |

### 1.2 架构范围

```
┌─────────────────────────────────────────────────────────────┐
│                    ReActAgent                                │
│                          │                                   │
│                          ▼                                   │
│              ObservabilityPlugin                             │
│                          │                                   │
│          ┌───────────────┴───────────────┐                  │
│          ▼                               ▼                   │
│   RunLogLogger            Cleanup Functions           │
│   (日志写入器)                    (日志清理)                  │
│          │                                                       │
│   ┌──────┴──────┐                                           │
│   ▼             ▼                                               │
│ Session Logs  Run Logs                                          │
│ (按日期)      (按 run_id)                                       │
└─────────────────────────────────────────────────────────────┘
```

### 1.3 关键设计决策

1. **Agent ID 由用户配置** - 确保日志组织的一致性
2. **JSONL 格式** - 追加写入，无文件锁定问题，兼容日志分析工具
3. **启动时清理** - 性能影响最小，实现简单
4. **非阻塞日志** - 日志失败不影响 Agent 执行

---

## 2. 目录结构

### 2.1 日志目录组织

```
logs/agents/
├── vol_advice/                    # agent_id
│   ├── sessions/
│   │   ├── session_<id>_20260410.jsonl
│   │   ├── session_<id>_20260409.jsonl
│   │   └── ...
│   └── runs/
│       ├── run_<run_id>.jsonl
│       └── ...
└── vol_code_assistant/
    └── ...
```

### 2.2 文件命名规范

| 日志类型 | 命名模式 | 示例 |
|----------|---------|------|
| 会话日志 | `session_{session_id}_{YYYYMMDD}.jsonl` | `session_sess_abc123_20260410.jsonl` |
| 运行日志 | `run_{run_id}.jsonl` | `run_run_xyz789.jsonl` |

---

## 3. 日志格式

### 3.1 JSONL 格式（文件输出）

每条日志为一个 JSON 对象，一行一条：

```json
{"timestamp":"2026-04-10T12:34:56.789Z","run_id":"run_abc123","agent_id":"vol_advice","event":"AgentStart","data":{"input":"analyze market"}}
{"timestamp":"2026-04-10T12:34:57.123Z","run_id":"run_abc123","agent_id":"vol_advice","event":"ToolCallBegin","data":{"tool_name":"get_price","arguments":"{\"symbol\":\"BTC\"}"}}
{"timestamp":"2026-04-10T12:34:58.456Z","run_id":"run_abc123","agent_id":"vol_advice","event":"ToolCallComplete","data":{"tool_name":"get_price","result":"69000"}}
{"timestamp":"2026-04-10T12:34:59.789Z","run_id":"run_abc123","agent_id":"vol_advice","event":"AgentComplete","data":{"iterations":"1","tool_calls_count":"1"}}
```

**字段说明**:

| 字段 | 类型 | 说明 |
|------|------|------|
| `timestamp` | string | RFC3339 格式时间戳 |
| `run_id` | string | 运行唯一标识 |
| `agent_id` | string | Agent 标识 |
| `event` | string | 事件类型 |
| `data` | object | 事件数据 |

### 3.2 人类可读格式（标准输出）

```
[INFO] [vol_advice] [run_abc123] Agent started - input: "analyze market"
[INFO] [vol_advice] [run_abc123] Tool call: get_price({"symbol":"BTC"})
[INFO] [vol_advice] [run_abc123] Tool result: 69000
[INFO] [vol_advice] [run_abc123] Agent completed - iterations: 1, tools: 1
```

**格式模式**:
```
[LEVEL] [agent_id] [run_id] Event summary - details
```

---

## 4. 支持的事件类型

可观测插件记录所有 8 种 `AgentStreamEvent` 事件，**每个事件同时记录到 Run 日志和 Session 日志**：

| 事件 | 记录内容 |
|------|---------|
| `AgentStart` | input |
| `ThinkingComplete` | thinking_length |
| `ToolCallBegin` | tool_name, arguments |
| `ToolCallComplete` | tool_name, result |
| `IterationComplete` | iteration, tool_calls_count, has_final_answer |
| `AgentComplete` | iterations, tool_calls_count |
| `AgentAborted` | reason |
| `PluginEvent` | name, data |

**日志类型说明**:
- **Run 日志** (`run_{run_id}.jsonl`): 包含所有事件，按 run_id 组织，便于追踪单次 Agent 运行的完整流程
- **Session 日志** (`session_{session_id}_{YYYYMMDD}.jsonl`): 包含所有事件，按 session 和日期组织，便于跨 run 聚合分析

---

## 5. 使用指南

### 5.1 基本使用

```rust
use vol_llm_agent::{ReActAgent, observability::ObservabilityPlugin};
use std::sync::Arc;

// 创建 Agent
let agent = ReActAgent::builder()
    .with_llm(llm_client)
    .with_agent_id("vol_advice".to_string())
    .with_log_base_path(std::path::PathBuf::from("logs/agents"))
    .with_observability_plugin()
    .build()
    .unwrap();

// 运行 Agent
let context = ToolContext::default();
let mut stream = agent.run("分析市场走势", context).await.unwrap();

// 消费事件流
while let Some(event) = stream.recv().await {
    // 事件已通过 ObservabilityPlugin 自动记录
}
```

### 5.2 配置选项

| 配置项 | 说明 | 默认值 |
|--------|------|--------|
| `agent_id` | Agent 唯一标识 | 自动生成 `agent_xxx` |
| `log_base_path` | 日志根目录 | `logs/agents` |

### 5.3 Builder 方法

```rust
impl AgentBuilder {
    /// 设置 Agent ID
    pub fn with_agent_id(mut self, agent_id: String) -> Self
    
    /// 设置日志根目录
    pub fn with_log_base_path(mut self, path: PathBuf) -> Self
    
    /// 启用可观测插件
    pub fn with_observability_plugin(mut self) -> Self
}
```

---

## 6. 组件详解

### 6.1 RunLogLogger

**职责**: 异步日志写入器，同时输出到文件和标准输出

```rust
pub struct RunLogLogger {
    agent_id: String,
    agent_path: PathBuf,
}

impl RunLogLogger {
    pub fn new(agent_id: String, log_base_path: PathBuf) -> Self;
    pub fn agent_id(&self) -> &str;
    pub async fn log(&self, entry: LogEntry, log_type: LogType);
}
```

**特性**:
- 自动创建目录结构
- 异步文件写入（非阻塞）
- 失败时仅记录警告，不影响 Agent 执行

### 6.2 LogEntry

**日志条目结构**:

```rust
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub run_id: String,
    pub agent_id: String,
    pub event: String,
    pub data: Value,
}

impl LogEntry {
    pub fn to_json_line(&self) -> String;      // JSONL 格式
    pub fn to_stdout_line(&self) -> String;    // 人类可读格式
}
```

### 6.3 LogType

**日志类型枚举**:

```rust
pub enum LogType {
    Session { session_id: String, date: String },
    Run { run_id: String },
}
```

### 6.4 ObservabilityPlugin

**插件实现**:

```rust
pub struct ObservabilityPlugin {
    logger: Arc<RunLogLogger>,
}

#[async_trait]
impl AgentPlugin for ObservabilityPlugin {
    fn id(&self) -> PluginId { "observability".to_string() }
    fn priority(&self) -> u32 { 10 }
    async fn intercept(...) -> PluginDecision { PluginDecision::Continue }
    async fn listen(&self, event: &AgentStreamEvent, ctx: &RunContext);
}
```

### 6.5 清理函数

**日志清理工具**:

```rust
// 清理旧日志（综合入口）
pub async fn cleanup_old_logs(agent_path: &Path) -> Result<(), LogError>

// 清理会话日志（保留最近 N 天）
pub async fn cleanup_session_logs(sessions_path: &Path, retention_days: u32) -> Result<usize, LogError>

// 清理运行日志（保留最近 N 个）
pub async fn cleanup_run_logs(runs_path: &Path, max_runs: usize) -> Result<usize, LogError>
```

---

## 7. 保留策略

### 7.1 会话日志（7 天）

- **触发时机**: Agent 启动时
- **清理规则**: 删除文件名中日期早于 7 天的日志
- **文件名模式**: `session_{id}_{YYYYMMDD}.jsonl`

### 7.2 运行日志（最近 10 个）

- **触发时机**: Agent 启动时
- **清理规则**: 保留最近的 10 个文件，删除更早的
- **文件名模式**: `run_{run_id}.jsonl`

### 7.3 清理行为

```rust
// 在 ReActAgent::run() 中自动触发
tokio::spawn(async move {
    let agent_path = log_base_path.join(&agent_id);
    if let Err(e) = cleanup_old_logs(&agent_path).await {
        tracing::warn!(agent_id = %agent_id, error = %e, "Log cleanup failed");
    }
});
```

**特点**:
- 非阻塞（spawn 到独立任务）
- 失败仅记录警告
- 不影响 Agent 执行

---

## 8. 错误处理

### 8.1 错误类型

```rust
pub enum LogError {
    Io(std::io::Error),       // IO 错误
    Parse(String),            // 解析错误
}
```

### 8.2 错误处理原则

| 场景 | 行为 |
|------|------|
| 目录创建失败 | 记录 tracing 警告，继续执行 |
| 文件写入失败 | 记录 tracing 警告，继续执行 |
| 清理失败 | 记录 tracing 警告，继续执行 |

**核心原则**: 日志失败永远不会阻塞或崩溃 Agent

---

## 9. 测试策略

### 9.1 单元测试

```rust
// 1. 目录创建测试
#[tokio::test]
async fn test_logger_creates_directories()

// 2. 文件写入测试
#[tokio::test]
async fn test_logger_log_writes_to_file

// 3. JSONL 序列化测试
#[test]
fn test_log_entry_to_json()

// 4. 标准输出格式化测试
#[test]
fn test_log_entry_to_stdout()

// 5. 会话日志清理测试
#[tokio::test]
async fn test_cleanup_session_logs_removes_old_files

// 6. 运行日志清理测试
#[tokio::test]
async fn test_cleanup_run_logs_keeps_last_n

// 7. 全事件类型测试
#[tokio::test]
async fn test_observability_plugin_logs_all_event_types
```

### 9.2 集成测试

```rust
// 完整 Agent 运行测试
#[tokio::test]
async fn test_full_agent_run_with_observability()
```

---

## 10. 配置示例

### 10.1 开发环境

```rust
use vol_llm_agent::{ReActAgent, AgentConfig};
use std::path::PathBuf;

let agent = ReActAgent::builder()
    .with_llm(llm_client)
    .with_agent_id("vol_advice_dev".to_string())
    .with_log_base_path(PathBuf::from("./logs/agents"))
    .with_observability_plugin()
    .with_verbose(true)
    .build()
    .unwrap();
```

### 10.2 生产环境

```rust
let agent = ReActAgent::builder()
    .with_llm(llm_client)
    .with_agent_id("vol_advice_prod".to_string())
    .with_log_base_path(PathBuf::from("/var/log/vol-monitor/agents"))
    .with_observability_plugin()
    .with_verbose(false)
    .build()
    .unwrap();
```

### 10.3 自定义 Agent ID

```rust
// 推荐使用有意义的 Agent ID
let agent = ReActAgent::builder()
    .with_agent_id("market_analysis_agent".to_string())
    // ...
    .build()
    .unwrap();

// 或使用默认自动生成
let agent = ReActAgent::builder()
    // agent_id 默认为自动生成，如 agent_abc123
    // ...
    .build()
    .unwrap();
```

---

## 11. 日志分析示例

### 11.1 使用 jq 分析 JSONL 日志

```bash
# 查看特定 run_id 的所有事件
cat logs/agents/vol_advice/runs/run_abc123.jsonl | jq '.'

# 统计工具调用次数
cat logs/agents/vol_advice/runs/run_abc123.jsonl | \
  jq -r 'select(.event == "ToolCallBegin") | .data.tool_name' | \
  sort | uniq -c

# 提取所有错误
cat logs/agents/vol_advice/runs/*.jsonl | \
  jq -r 'select(.event == "AgentAborted") | "\(.timestamp): \(.data.reason)"'
```

### 11.2 使用 grep 快速搜索

```bash
# 查找特定工具调用
grep "ToolCallBegin" logs/agents/vol_advice/runs/*.jsonl

# 查找特定 run_id
grep "run_abc123" logs/agents/vol_advice/sessions/*.jsonl
```

---

## 12. 最佳实践

### 12.1 Agent ID 命名

- **推荐**: 使用有意义的名称，如 `vol_advice`, `market_analysis`
- **避免**: 使用随机 ID 或频繁变更的 ID

### 12.2 日志目录管理

- **开发**: 使用相对路径 `./logs/agents`
- **生产**: 使用绝对路径 `/var/log/vol-monitor/agents`
- **容器**: 挂载到持久化存储卷

### 12.3 日志聚合

对于多 Agent 部署，建议使用日志聚合工具：

```bash
# 示例：使用 loki + promtail 收集 JSONL 日志
# promtail 配置示例
scrape_configs:
  - job_name: agent-logs
    static_configs:
      - targets:
          - localhost
        labels:
          job: agent-logs
          __path__: /var/log/vol-monitor/agents/**/*.jsonl
```

---

## 13. 架构决策

### 13.1 为什么使用 JSONL？

| 特性 | 优势 |
|------|------|
| 追加写入 | 无文件锁定问题 |
| 逐行解析 | 易于处理大文件 |
| 工具兼容 | 支持 jq、promtail 等工具 |

### 13.2 为什么启动时清理？

| 方案 | 优点 | 缺点 |
|------|------|------|
| 启动时清理 | 实现简单，性能影响小 | 清理不够及时 |
| 每次写入前清理 | 清理及时 | 影响写入性能 |

**决策**: 启动时清理对性能影响最小，适合大多数场景

### 13.3 为什么日志失败不阻塞？

日志是辅助功能，不应影响核心 Agent 执行。失败时记录 tracing 警告，便于排查。

---

## 14. 参考

### 14.1 相关文件

- 设计文档：`docs/superpowers/specs/2026-04-10-agent-observability-plugin-design.md`
- 实现计划：`docs/superpowers/plans/2026-04-10-agent-observability-implementation.md`

### 14.2 代码位置

| 文件 | 路径 |
|------|------|
| 模块入口 | `crates/vol-llm-agent/src/observability/mod.rs` |
| 日志写入器 | `crates/vol-llm-agent/src/observability/logger.rs` |
| 清理工具 | `crates/vol-llm-agent/src/observability/cleanup.rs` |
| 插件实现 | `crates/vol-llm-agent/src/observability/plugin.rs` |

### 14.3 相关文档

- [AI Agent - ReAct Agent & Tools 设计](03-agent-tool-design.md)
- [AI Agent - LLM Client 架构设计](01-llm-client-architecture.md)

### 14.4 Wiki

- [[agent-observability]]: 可观测插件的 wiki 页面
- [[agent-plugin-system]]: 插件系统架构
- [[built-in-plugins]]: 内置插件列表
- [[agent-event-stream]]: 事件流类型
