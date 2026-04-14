# AgentStreamEvent 与可观测性增强设计

**创建日期**: 2026-04-14
**状态**: 设计中

---

## 1. 动机

当前代码存在以下问题：

1. **事件缺少时间戳** — JSONL 日志的 timestamp 来自 `LogEntry` 写入时间，不是事件实际发生时间
2. **LLMCallComplete 数据缺失** — agent.rs 里 emit 的是 `model: String::new(), usage: None`，硬编码空值
3. **Tool 事件没有 duration** — 工具执行耗时无法从事件直接获取
4. **AgentComplete 缺少 response** — 最终答案不在事件里
5. **SessionMessage 的 parent_id 总是 None** — `add_message` 没有设置 parent 链
6. **agent.rs 里散布 debug/info 打印** — 日志输出职责应该统一归 observability plugin

---

## 2. AgentStreamEvent 变更

### 2.1 新增 timestamp 字段（所有事件）

```rust
pub enum AgentStreamEvent {
    AgentStart {
        timestamp: chrono::DateTime<chrono::Utc>,
        input: String,
    },
    AgentComplete {
        timestamp: chrono::DateTime<chrono::Utc>,
        response: Option<serde_json::Value>, // AgentResponse 的 JSON 表示
    },
    AgentAborted {
        timestamp: chrono::DateTime<chrono::Utc>,
        reason: String,
    },
    LLMCallStart {
        timestamp: chrono::DateTime<chrono::Utc>,
        iteration: u32,
        messages: Vec<Message>, // 发送给 LLM 的完整消息历史
    },
    LLMCallComplete {
        timestamp: chrono::DateTime<chrono::Utc>,
        model: String,
        usage: Option<TokenUsage>,
    },
    LLMCallError {
        timestamp: chrono::DateTime<chrono::Utc>,
        error: String,
    },
    ToolCallBegin {
        timestamp: chrono::DateTime<chrono::Utc>,
        tool_call_id: String,
        tool_name: String,
        arguments: String,
    },
    ToolCallComplete {
        timestamp: chrono::DateTime<chrono::Utc>,
        tool_call_id: String,
        tool_name: String,
        result: String,
        duration_ms: Option<u64>,
    },
    ToolCallError {
        timestamp: chrono::DateTime<chrono::Utc>,
        tool_call_id: String,
        tool_name: String,
        error: String,
        duration_ms: Option<u64>,
    },
    ToolCallSkipped {
        timestamp: chrono::DateTime<chrono::Utc>,
        tool_call_id: String,
        tool_name: String,
        reason: String,
        duration_ms: Option<u64>,
    },
    // ... 其余事件同样增加 timestamp 字段
}
```

### 2.2 新增辅助构造器

为方便 emit，每个 variant 提供快捷方法：

```rust
impl AgentStreamEvent {
    fn agent_start(input: String) -> Self {
        Self::AgentStart { timestamp: Utc::now(), input }
    }
    fn tool_call_begin(tool_call_id: String, tool_name: String, arguments: String) -> Self {
        Self::ToolCallBegin { timestamp: Utc::now(), tool_call_id, tool_name, arguments }
    }
    // ... 每个 variant 都有一个快捷构造器
}
```

---

## 3. SessionMessage parent_id 自动设置

### 3.1 改动 `RunContext::add_message`

当前：
```rust
let session_msg = SessionMessage::new(self.session_id.clone(), message);
// parent_id 永远是 None
```

改为：
```rust
let session_msg = SessionMessage::new(self.session_id.clone(), message)
    .with_parent_id(self.last_message_id.clone());
self.last_message_id.write().await.replace(session_msg.id.clone());
```

需要 `RunContext` 新增 `last_message_id: Arc<Mutex<Option<String>>>` 字段。

### 3.2 Session 增加 `get_last_message_id` 方法

`Session::add_message` 返回刚写入消息的 ID，或者新增 `last_message_id: Arc<Mutex<Option<String>>>` 字段跟踪。

**采用方案：在 RunContext 侧跟踪**，因为：
- Session 存储是异步的，返回 ID 需要改动接口
- RunContext 是 add_message 的调用方，直接 track 更简单

---

## 4. consume_llm_stream 返回更多信息

### 4.1 新增返回值

```rust
async fn consume_llm_stream(
    stream: StreamReceiver,
    run_ctx: &RunContext,
    iteration: u32,
) -> Result<(String, Vec<ToolCall>, String, String, Option<TokenUsage>), AgentError> {
    // 返回: (thinking, tool_calls, content, model, usage)
```

- **model**: 从 `StreamEventData::ResponseComplete` 或 LLM provider 获取
- **usage**: 从 `StreamEventData::UsageUpdate { usage }` 累计（当前被忽略了）

### 4.2 捕获 UsageUpdate

在 `consume_llm_stream` 的 match 分支中：

```rust
StreamEventData::UsageUpdate { usage } => {
    last_usage = Some(usage);
}
```

---

## 5. agent.rs 移除直接打印

### 5.1 移除列表

| 位置 | 当前代码 | 替代方式 |
|------|---------|---------|
| agent.rs:264 | `info!("Iteration {}")` | ObservabilityPlugin via IterationComplete event |
| agent.rs:274-303 | debug 打印对话历史 | 已通过 LLMCallStart.messages 事件携带 |
| agent.rs:353 | `debug!("Tool calls: ...")` | 已通过 ToolCallBegin 事件携带 |
| agent.rs:373 | `info!("Executing tool: ...")` | 已通过 ToolCallBegin 事件携带 |
| agent.rs:504 | `info!("Tool {} returned: ...")` | 已通过 ToolCallComplete 事件携带 |

### 5.2 保留

- `verbose` 模式下的 tracing 保留（给开发者调试用）
- 错误路径的 `tracing::warn!` 保留（这些不是常规日志）

---

## 6. 改动文件清单

| 文件 | 改动 |
|------|------|
| `crates/vol-llm-core/src/stream.rs` | AgentStreamEvent 全部 18 个 variant 增加 timestamp；新增快捷构造器 |
| `crates/vol-llm-core/src/message.rs` | 无改动（Message 已有 tool_call_id 字段） |
| `crates/vol-session/src/message.rs` | 无改动（SessionMessage 已有 id + parent_id） |
| `crates/vol-llm-agent/src/react/run_context.rs` | 新增 `last_message_id` 字段；修改 `add_message` 自动设置 parent_id |
| `crates/vol-llm-agent/src/react/agent.rs` | consume_llm_stream 返回 model/usage；移除 debug/info；更新所有 emit 调用携带 timestamp |
| `crates/vol-llm-observability/src/plugin.rs` | 更新 create_log_entry 处理新字段（timestamp 已在 LogEntry 中，事件本身也带） |
| `crates/vol-session/src/listener.rs` | 更新 event_to_message 处理新字段 |
| `crates/vol-llm-tui/src/render.rs` | 更新 render_event 处理新字段 |
| 所有测试文件 | 更新事件构造方式，或添加 timestamp 字段 |

---

## 7. 时序保证

timestamp 在事件 emit 时自动设置，保证：

```
AgentStart(ts1)
  LLMCallStart(ts2)
    ThinkingStart(ts3) → TTFT = ts3 - ts2
    ContentStart(ts4)
    ContentComplete(ts5)
  LLMCallComplete(ts6) → LLM 耗时 = ts6 - ts2
  ToolCallBegin(ts7)
  ToolCallComplete(ts8) → 工具耗时 = ts8 - ts7
AgentComplete(ts9)
```

Tool 的 `duration_ms` 由 agent.rs 在 emit ToolCallComplete 时计算：
```rust
let duration = begin_timestamp.elapsed().as_millis() as u64;
run_ctx.emit(AgentStreamEvent::ToolCallComplete {
    timestamp: Utc::now(),
    tool_call_id: ...,
    tool_name: ...,
    result: ...,
    duration_ms: Some(duration),
}).await;
```

需要在 emit ToolCallBegin 时记录开始时间，然后 emit ToolCallComplete 时计算差值。
