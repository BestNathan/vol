# Session 与 Message Store 设计文档

## 概述

本文档描述了 ReAct Agent 的 Session（会话）和 MessageStore（消息存储）的设计与实现。该设计支持：
- 会话级别的消息管理
- 历史消息持久化（内存/DB）
- 多会话并发处理
- 分叉对话（通过 parent_id）

## 架构设计

### 核心思想

1. **Session 作为会话上下文容器** - 封装会话元数据和存储操作
2. **Store Trait 抽象** - 分离接口与实现，支持多种后端
3. **Agent 绑定 Session** - 每个 Agent 实例关联一个 Session，支持多会话并发
4. **不侵入 core::Message** - 新增 SessionMessage 包装，保持 core 模块纯净

### 架构图

```
┌─────────────────────────────────────────────────────────────┐
│                    ReActAgent                               │
│                                                             │
│  - llm: Arc<dyn LLMClient>                                 │
│  - tools: Arc<ToolRegistry>                                │
│  - config: AgentConfig                                     │
│  - session: Arc<Session>                                   │
│                                                             │
│  + run(user_input, context) -> AgentStreamReceiver         │
│  + with_new_session(id) -> Self                            │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│                      Session                                │
│                                                             │
│  - id: String                                               │
│  - created_at: i64                                          │
│  - metadata: HashMap<String, String>                        │
│  - session_store: Arc<dyn SessionStore>                     │
│  - message_store: Arc<dyn MessageStore>                     │
│                                                             │
│  + get_messages(limit) -> Vec<SessionMessage>              │
│  + add_message(message) -> Result<()>                       │
│  + get_or_create_parent(parent_id) -> Option<Session>       │
└─────────────────────────────────────────────────────────────┘
                     │                    │
          ┌──────────┴──────┐   ┌─────────┴────────┐
          ▼                 ▼   ▼                  ▼
┌─────────────────┐ ┌──────────────────────────────────────┐
│ SessionStore    │ │         MessageStore                 │
│ + create()      │ │ + save()                             │
│ + get()         │ │ + get_by_session()                   │
│ + delete()      │ │ + get_before()                       │
│ + update()      │ │ + delete_session()                   │
│                 │ │ + update()                           │
│                 │ │ + get_count()                        │
│                 │ │ + cleanup_expired()                  │
└─────────────────┘ └──────────────────────────────────────┘
         │                        │
         ▼                        ▼
┌─────────────────┐   ┌──────────────────────────────────────┐
│ InMemorySession │   │       InMemoryMessageStore           │
│ Store           │   │ - RwLock<HashMap<id, SessionMessage>>│
│ - HashMap       │   │                                      │
└─────────────────┘   └──────────────────────────────────────┘
```

### 数据流

```
用户输入 → ReActAgent.run()
              │
              ▼
    Session.get_messages()  ← 从 Store 加载历史
              │
              ▼
    构建 ConversationRequest (包含历史消息)
              │
              ▼
    LLM 调用 → 生成响应/工具调用
              │
              ▼
    Session.add_message()  ← 保存新消息到 Store
```

## 核心组件

### 1. SessionMessage

位置：`crates/vol-llm-agent/src/session/message.rs`

```rust
/// 会话消息包装
///
/// 包装 `vol_llm_core::Message`，增加会话相关字段
pub struct SessionMessage {
    /// 消息唯一 ID (UUID)
    pub id: String,
    
    /// 所属会话 ID
    pub session_id: String,
    
    /// 核心消息体
    pub message: vol_llm_core::Message,
    
    /// 父消息 ID，支持树形对话结构
    /// None 表示根消息（对话起点）
    pub parent_id: Option<String>,
    
    /// 创建时间戳（Unix 秒）
    pub created_at: i64,
    
    /// 元数据，可扩展用途
    /// 例如：user_id, tags, 等
    pub metadata: HashMap<String, String>,
}
```

**设计说明**：
- 采用组合模式包装 `core::Message`，不修改原有结构
- `parent_id` 支持分叉对话，后续可扩展分支切换
- `metadata` 提供灵活扩展能力

### 2. Session

位置：`crates/vol-llm-agent/src/session/session.rs`

```rust
/// 会话管理
///
/// 封装会话元数据和存储操作
pub struct Session {
    /// 会话唯一 ID
    pub id: String,
    
    /// 创建时间戳（Unix 秒）
    pub created_at: i64,
    
    /// 会话元数据
    /// 例如：user_id, title, 等
    pub metadata: HashMap<String, String>,
    
    /// 会话存储
    session_store: Arc<dyn SessionStore>,
    
    /// 消息存储
    message_store: Arc<dyn MessageStore>,
}

impl Session {
    /// 创建新会话
    pub fn new(
        id: String,
        session_store: Arc<dyn SessionStore>,
        message_store: Arc<dyn MessageStore>,
    ) -> Self;
    
    /// 获取历史消息
    pub async fn get_messages(&self, limit: usize) -> Result<Vec<SessionMessage>>;
    
    /// 添加消息
    pub async fn add_message(&self, message: SessionMessage) -> Result<()>;
    
    /// 根据父会话 ID 获取或创建会话（支持分叉）
    pub async fn get_or_create_parent(&self, parent_id: &str) -> Option<Session>;
}
```

### 3. SessionStore Trait

位置：`crates/vol-llm-agent/src/session/store.rs`

```rust
/// 会话存储接口
#[async_trait]
pub trait SessionStore: Send + Sync {
    /// 创建会话
    async fn create(&self, session: Session) -> Result<()>;
    
    /// 获取会话
    async fn get(&self, session_id: &str) -> Result<Option<Session>>;
    
    /// 删除会话
    async fn delete(&self, session_id: &str) -> Result<()>;
    
    /// 更新会话
    async fn update(&self, session: Session) -> Result<()>;
}
```

### 4. MessageStore Trait

位置：`crates/vol-llm-agent/src/session/store.rs`

```rust
/// 消息存储接口
#[async_trait]
pub trait MessageStore: Send + Sync {
    /// 保存消息
    async fn save(&self, message: SessionMessage) -> Result<()>;
    
    /// 按会话获取历史消息
    async fn get_by_session(&self, session_id: &str, limit: usize) -> Result<Vec<SessionMessage>>;
    
    /// 分页获取消息（在指定时间之前的消息）
    async fn get_before(&self, session_id: &str, before: i64, limit: usize) -> Result<Vec<SessionMessage>>;
    
    /// 删除会话的所有消息
    async fn delete_session(&self, session_id: &str) -> Result<()>;
    
    /// 更新消息
    async fn update(&self, id: &str, message: SessionMessage) -> Result<()>;
    
    /// 获取会话消息数量
    async fn get_count(&self, session_id: &str) -> Result<usize>;
    
    /// 清理过期消息
    async fn cleanup_expired(&self, before: i64) -> Result<()>;
}
```

### 5. InMemorySessionStore / InMemoryMessageStore

位置：`crates/vol-llm-agent/src/session/memory_store.rs`

```rust
/// 内存会话存储
pub struct InMemorySessionStore {
    sessions: RwLock<HashMap<String, Session>>,
}

/// 内存消息存储
pub struct InMemoryMessageStore {
    messages: RwLock<HashMap<String, Vec<SessionMessage>>>,
}
```

**特点**：
- 使用 `RwLock` 实现线程安全
- 适用于测试、演示、短期会话
- 无持久化，重启后数据丢失

### 6. ReActAgent 集成

位置：`crates/vol-llm-agent/src/react/agent.rs`

```rust
pub struct ReActAgent {
    llm: Arc<dyn LLMClient>,
    tools: Arc<ToolRegistry>,
    config: AgentConfig,
    session: Arc<Session>,
}

impl ReActAgent {
    /// 创建绑定到会话的 Agent
    pub fn with_session(
        llm: Arc<dyn LLMClient>,
        tools: Arc<ToolRegistry>,
        config: AgentConfig,
        session: Arc<Session>,
    ) -> Self;
    
    /// 克隆到新会话
    pub fn with_new_session(&self, session_id: String) -> Self {
        let new_session = Arc::new(Session::new(
            session_id,
            self.session.session_store.clone(),
            self.session.message_store.clone(),
        ));
        Self {
            session: new_session,
            ..self.clone()
        }
    }
    
    /// 运行 Agent，自动从 session 加载历史消息
    pub async fn run(&self, user_input: &str, context: ToolContext) -> Result<AgentStreamReceiver> {
        // 1. 从 Session 获取历史消息
        let history = self.session.get_messages(self.config.max_iterations as usize).await?;
        
        // 2. 构建对话请求（包含历史）
        let messages = build_messages_with_history(history, user_input);
        
        // 3. 执行 ReAct 循环
        // ...
        
        // 4. 保存新消息到 Session
        self.session.add_message(new_message).await?;
    }
}
```

## 使用示例

### 基础用法

```rust
use vol_llm_agent::session::{Session, SessionMessage, InMemorySessionStore, InMemoryMessageStore};
use vol_llm_agent::react::ReActAgent;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    // 1. 创建存储
    let session_store = Arc::new(InMemorySessionStore::new());
    let message_store = Arc::new(InMemoryMessageStore::new());
    
    // 2. 创建会话
    let session = Arc::new(Session::new(
        "session-123".to_string(),
        session_store.clone(),
        message_store.clone(),
    ));
    
    // 3. 创建 Agent 绑定会话
    let agent = ReActAgent::builder()
        .with_llm(llm)
        .with_tools(tools)
        .with_session(session.clone())
        .build()
        .unwrap();
    
    // 4. 运行 Agent
    let context = ToolContext::default();
    let mut receiver = agent.run("你好，请分析市场情况", context).await.unwrap();
    
    // 5. 处理流事件
    while let Some(event) = receiver.recv().await {
        match event {
            Ok(AgentStreamEvent::AgentComplete { response }) => {
                println!("回答：{}", response.content);
            }
            _ => {}
        }
    }
}
```

### 多会话并发

```rust
// 创建共享存储
let session_store = Arc::new(InMemorySessionStore::new());
let message_store = Arc::new(InMemoryMessageStore::new());

// 会话 1
let session1 = Arc::new(Session::new(
    "session-1".to_string(),
    session_store.clone(),
    message_store.clone(),
));
let agent1 = ReActAgent::builder()
    .with_session(session1)
    .build()
    .unwrap();

// 会话 2（克隆自同一 Agent 配置）
let session2 = Arc::new(Session::new(
    "session-2".to_string(),
    session_store.clone(),
    message_store.clone(),
));
let agent2 = agent1.with_new_session("session-2");

// 并发运行
tokio::spawn(async move {
    agent1.run("用户 1 的问题", context1).await
});
tokio::spawn(async move {
    agent2.run("用户 2 的问题", context2).await
});
```

### 分叉对话

```rust
// 从父会话创建新分支
let parent_session = Arc::new(Session::new(
    "session-parent".to_string(),
    session_store.clone(),
    message_store.clone(),
));

// ... 运行一些对话后 ...

// 创建分叉会话
let child_session = Arc::new(Session::new(
    "session-child".to_string(),
    session_store.clone(),
    message_store.clone(),
));
// 设置 parent_id 关联
child_session.metadata.insert("parent_id".to_string(), parent_session.id.clone());
```

## 扩展指南

### 实现 DB 版本

实现 `SessionStore` 和 `MessageStore` trait：

```rust
use vol_llm_agent::session::{SessionStore, MessageStore, SessionMessage};
use vol_llm_core::Result;

pub struct SqliteMessageStore {
    pool: SqlitePool,
}

#[async_trait]
impl MessageStore for SqliteMessageStore {
    async fn save(&self, message: SessionMessage) -> Result<()> {
        // 实现 SQLite 保存逻辑
        Ok(())
    }
    
    async fn get_by_session(&self, session_id: &str, limit: usize) -> Result<Vec<SessionMessage>> {
        // 实现查询逻辑
        Ok(vec![])
    }
    
    // ... 实现其他方法
}
```

### 添加索引优化

在 DB 实现中添加索引：
- `session_id + created_at` 复合索引（加速历史查询）
- `parent_id` 索引（加速分叉查询）

## 测试

### 单元测试

```bash
# 运行 session 模块测试
cargo test -p vol-llm-agent session
```

### 集成测试

```bash
# 运行 Agent 会话集成测试
cargo test -p vol-llm-agent --test session_integration
```

## 依赖关系

```toml
[dependencies]
vol-llm-core = { path = "crates/vol-llm-core" }
async-trait = "0.1"
uuid = "1.0"
tokio = { version = "1.0", features = ["sync"] }
```

## 文件结构

```
crates/vol-llm-agent/src/session/
├── mod.rs              # 模块入口，导出所有类型
├── message.rs          # SessionMessage
├── session.rs          # Session
├── store.rs            # SessionStore, MessageStore traits
└── memory_store.rs     # InMemory 实现

crates/vol-llm-agent/src/react/
└── agent.rs            # ReActAgent 集成 Session
```

## 设计决策

### 为什么 SessionMessage 包装 core::Message 而不是继承？

1. **职责分离** - `core::Message` 是通用 LLM 消息抽象，`SessionMessage` 是会话上下文包装
2. **不侵入 core** - 保持 core 模块纯净，不依赖会话概念
3. **灵活性** - 可以在不改变 core::Message 的情况下扩展 SessionMessage

### 为什么 Agent 持有 Session 而不是 Store？

1. **简化调用** - Agent 只需 `session.get_messages()`，无需直接操作 Store
2. **封装性好** - Session 封装了存储细节，Agent 不关心实现
3. **便于扩展** - Session 可以添加缓存、日志等逻辑

### 为什么需要 parent_id？

1. **支持分叉对话** - 用户可能想回到之前的某个节点继续对话
2. **对话树结构** - 完整的对话历史是树形而非线性
3. **后续扩展** - 支持对话分支切换、对比等高级功能

## 性能考虑

### 当前实现

- 内存版本使用 `RwLock`，读多写少场景性能良好
- 无缓存机制
- 无批量操作

### 未来优化方向

1. **消息缓存** - LRU 缓存最近消息，减少 DB 查询
2. **批量保存** - 批量写入消息，减少 DB 压力
3. **分页加载** - 大会话时分页加载历史消息
4. **消息压缩** - 长期存储时压缩旧消息

## 总结

Session 与 MessageStore 设计实现了一个灵活的会话管理框架：

- **SessionMessage** - 包装 core::Message，增加会话字段
- **Session** - 会话上下文容器，封装存储操作
- **SessionStore / MessageStore** - Trait 抽象，支持多种后端
- **InMemory 实现** - 开箱即用的内存存储
- **ReActAgent 集成** - Agent 绑定 Session，自动管理历史消息

用户可以通过实现 trait 对接自己的存储后端（SQLite、PostgreSQL、TDengine 等），快速构建支持历史消息的 Agent 应用。
