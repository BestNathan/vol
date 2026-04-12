# Session 包重构设计文档

**日期:** 2026-04-11  
**状态:** 待评审

---

## 概述

将 Session 相关逻辑重构为独立的 `vol-session` 包，专注于会话消息的记录和还原。

### 核心原则

1. **Session 专注于 Message 内容** - 通过 session 文件可以还原整个会话
2. **监听关键事件** - 只记录与对话相关的事件，不是全部事件
3. **职责分离** - Session 作为独立 Listener，不依赖 ReAct Agent 主动写入

---

## 记录的事件类型

| 事件 | 记录内容 | JSONL 字段 |
|------|---------|-----------|
| `UserMessage` | 用户输入 prompt | `{event: "UserMessage", data: {content: "..."}}` |
| `ThinkingComplete` | LLM 思考内容 | `{event: "ThinkingComplete", data: {thinking: "..."}}` |
| `ToolCallBegin` | 工具调用（名称 + 参数） | `{event: "ToolCallBegin", data: {tool_name, arguments}}` |
| `ToolCallComplete` | 工具返回结果 | `{event: "ToolCallComplete", data: {tool_name, result}}` |
| `IterationComplete` | 迭代完成（含 final_answer） | `{event: "IterationComplete", data: {iteration, tool_calls, final_answer}}` |

**不记录的事件：**
- `AgentStart` / `AgentComplete` - 由 observability 日志负责
- `AgentResponse` - 已通过 `IterationComplete` 中的 `final_answer` 记录

---

## 架构设计

### 包结构

```
vol-session/
├── src/
│   ├── lib.rs           # 包入口和导出
│   ├── message.rs       # SessionMessage 类型定义
│   ├── session.rs       # Session 容器
│   ├── store.rs         # SessionStore / MessageStore traits
│   ├── memory_store.rs  # InMemory 实现
│   ├── file_store.rs    # 文件存储实现（JSONL）
│   └── listener.rs      # SessionListener - 事件监听和过滤
├── Cargo.toml
└── README.md
```

### 组件职责

| 组件 | 职责 |
|------|------|
| `SessionMessage` | 包装 `vol_llm_core::Message`，添加 session_id、parent_id、metadata |
| `Session` | 会话容器，提供 `get_messages()`、`add_message()` API |
| `SessionStore` | Session CRUD 接口 |
| `MessageStore` | Message 持久化接口 |
| `FileMessageStore` | JSONL 文件存储实现 |
| `SessionListener` | 订阅 event bus，过滤关键事件，写入 FileMessageStore |

### SessionListener 工作流程

```
┌─────────────────────────────────────────────────────────────┐
│                    Event Broadcast Channel                   │
└─────────────────────────────┬───────────────────────────────┘
                              │
                              ▼
                    ┌─────────────────┐
                    │ SessionListener │
                    │                 │
                    │ - event_rx      │ 订阅 event bus
                    │ - store         │ MessageStore 引用
                    │ - session_id    │ 当前 session ID
                    └────────┬────────┘
                             │
                             ▼ 过滤关键事件
                    ┌─────────────────┐
                    │ 关键事件过滤器   │
                    │                 │
                    │ ✓ UserMessage   │
                    │ ✓ Thinking      │
                    │ ✓ ToolCallBegin │
                    │ ✓ ToolCallEnd   │
                    │ ✓ Iteration     │
                    └────────┬────────┘
                             │
                             ▼ 转换为 SessionMessage
                    ┌─────────────────┐
                    │ FileMessageStore│
                    │                 │
                    │ JSONL 追加写入   │
                    └─────────────────┘
```

---

## 文件存储格式

### JSONL 格式

每个事件一行，便于流式读取和追加写入：

```jsonl
{"event":"UserMessage","data":{"content":"请查询 BTC 的波动率"},"session_id":"abc123","timestamp":1712851200}
{"event":"ThinkingComplete","data":{"thinking":"用户要求查询 BTC 波动率..."},"session_id":"abc123","timestamp":1712851203}
{"event":"ToolCallBegin","data":{"tool_name":"volatility_index","arguments":"{\"symbol\": \"BTC\"}"},"session_id":"abc123","timestamp":1712851204}
{"event":"ToolCallComplete","data":{"tool_name":"volatility_index","result":"Index: btc_usd | Volatility: 42.98%"},"session_id":"abc123","timestamp":1712851205}
{"event":"IterationComplete","data":{"iteration":1,"final_answer":"BTC 当前波动率为 42.98%..."},"session_id":"abc123","timestamp":1712851210}
```

### 文件路径

```
logs/sessions/{agent_id}/
  ├── sessions/
  │   └── {session_id}.jsonl    # Session 消息文件
  └── metadata/
      └── {session_id}.meta.json # Session 元数据（可选）
```

---

## API 设计

### SessionListener

```rust
pub struct SessionListener {
    event_rx: broadcast::Receiver<TracedEvent<AgentStreamEvent>>,
    store: Arc<dyn MessageStore>,
    session_id: String,
}

impl SessionListener {
    /// 创建新的 SessionListener
    pub fn new(
        event_rx: broadcast::Receiver<TracedEvent<AgentStreamEvent>>,
        store: Arc<dyn MessageStore>,
        session_id: String,
    ) -> Self;

    /// 启动监听循环
    pub async fn run(self) -> Result<()>;
}
```

### FileMessageStore

```rust
pub struct FileMessageStore {
    base_path: PathBuf,
    session_id: String,
}

impl FileMessageStore {
    /// 创建新的文件存储
    pub fn new(base_path: &str, session_id: &str) -> Result<Self>;
}

#[async_trait]
impl MessageStore for FileMessageStore {
    async fn save(&self, message: SessionMessage) -> Result<()>;
    async fn get_by_session(&self, session_id: &str, limit: usize) -> Result<Vec<SessionMessage>>;
    // ... 其他方法
}
```

---

## 与现有代码的集成

### 当前结构

目前 `vol-llm-agent/src/session/` 包含：
- `message.rs` - SessionMessage
- `session.rs` - Session
- `store.rs` - Store traits
- `memory_store.rs` - InMemory 实现
- `mod.rs` - 模块导出

### 重构步骤

1. **保留现有 API** - `Session`, `SessionMessage`, `SessionStore`, `MessageStore` 接口不变
2. **添加 FileMessageStore** - 新的 JSONL 文件存储实现
3. **添加 SessionListener** - 独立监听器，订阅 event bus
4. **更新 ReActAgent** - 启动时创建 SessionListener 任务

### 集成点

在 `ReActAgent::run()` 中：

```rust
// 创建 Session
let session = Session::new(session_id, session_store, message_store);

// 创建 SessionListener 并启动后台任务
let session_listener = SessionListener::new(
    event_tx.subscribe(),  // 订阅 event bus
    Arc::new(FileMessageStore::new(&config.log_base_path, &session_id)?),
    session_id,
);
tokio::spawn(session_listener.run());
```

---

## 测试策略

### 单元测试

1. **SessionMessage 测试** - 验证消息创建、parent_id、metadata
2. **FileMessageStore 测试** - 验证 JSONL 读写、分页查询
3. **SessionListener 测试** - 验证事件过滤逻辑

### 集成测试

```rust
#[tokio::test]
async fn test_session_listener_records_key_events() {
    // 创建 event bus
    let (event_tx, event_rx) = broadcast::channel(100);
    
    // 创建 SessionListener
    let store = Arc::new(FileMessageStore::new("/tmp/test", "session-1").unwrap());
    let listener = SessionListener::new(event_rx, store, "session-1".to_string());
    
    // 发送测试事件
    event_tx.send(UserMessage { content: "Hello" }.into()).unwrap();
    event_tx.send(ThinkingComplete { thinking: "..." }.into()).unwrap();
    
    // 等待处理
    tokio::time::sleep(Duration::from_millis(100)).await();
    
    // 验证文件中有 2 行记录
    let file_content = fs::read_to_string("/tmp/test/session-1.jsonl").unwrap();
    assert_eq!(file_content.lines().count(), 2);
}
```

---

## 验收标准

- [ ] `vol-session` 包独立编译
- [ ] JSONL 文件格式正确，可读可解析
- [ ] SessionListener 正确过滤并记录 5 种关键事件
- [ ] 通过 session 文件可以完整还原对话（用户输入 → 思考 → 工具调用 → 回答）
- [ ] 现有测试全部通过
- [ ] 添加集成测试验证端到端流程

---

## 后续增强（可选）

1. **Session 分支支持** - 通过 `parent_id` 支持会话树结构
2. **压缩存储** - 大 session 文件自动压缩
3. **增量读取** - 支持从指定位置继续读取（断点续传）
4. **会话搜索** - 基于关键词搜索会话内容

---

**下一步：** 使用 `writing-plans` skill 创建详细实施计划
