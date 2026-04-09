# ReAct Agent RunContext 消息管理优化设计

**日期：** 2026-04-09
**作者：** Claude Code
**状态：** 设计完成，待实现

---

## 概述

将 ReAct Agent 运行时的消息管理完全迁移到 RunContext 中，实现单一数据源，消除局部 `messages` 变量，使插件可以通过 RunContext 访问和修改消息历史。

---

## 问题陈述

### 当前问题

**1. 双重状态维护**

当前 `ReActAgent::run()` 中同时存在：
- 局部变量 `messages: Vec<Message>` (line 139)
- `run_ctx.messages: Arc<RwLock<Vec<Message>>>` (未充分利用)

```rust
// 当前代码 - 局部变量
let mut messages = Vec::new();
messages.push(Message::system(config.system_prompt.clone()));
let history = session.get_messages(...).await;  // 每次都获取
messages.push(Message::user(user_input.clone()));

// 同时 ctx 也有 messages 字段，但未用于主逻辑
```

**2. 重复获取历史消息**

如果在 `build_messages()` 中每次都从 session 获取历史：
- 第一次 loop：获取历史 + 添加 tool result
- 第二次 loop：再次获取历史 + 再次添加 tool result
- **结果：历史消息被重复添加**

**3. 插件无法访问消息**

局部 `messages` 变量对插件不可见，插件无法：
- 查看当前消息历史
- 修改或注入消息
- 基于消息内容做决策

---

## 设计目标

| 目标 | 描述 | 优先级 |
|------|------|--------|
| **单一数据源** | 所有消息操作通过 `run_ctx.messages` | 高 |
| **初始化一次** | 历史消息只在 `init_messages()` 获取一次 | 高 |
| **插件可访问** | 插件可通过 `ctx.get_messages()` 访问 | 高 |
| **Session 同步** | 添加消息时自动持久化到 session | 中 |

---

## 架构设计

### 核心变更

```
┌────────────────────────────────────────────────────────────┐
│                     RunContext                             │
├────────────────────────────────────────────────────────────┤
│  messages: Arc<RwLock<Vec<Message>>>                       │
├────────────────────────────────────────────────────────────┤
│  + init_messages() -> Result<()>                           │
│    - 构建 System 消息（从 prompt_context）                   │
│    - 获取历史消息（从 session，只一次）                      │
│    - 添加用户输入                                           │
│    - 写入 messages                                         │
│                                                            │
│  + add_message(msg) -> Result<()>                          │
│    - 添加到 messages                                       │
│    - 同步持久化到 session                                  │
│                                                            │
│  + get_messages() -> Vec<Message>                          │
│    - 返回 messages 克隆                                     │
└────────────────────────────────────────────────────────────┘
```

### 数据流

```
ReActAgent::run()
    ↓
创建 RunContext
    ↓
run_ctx.init_messages()  ← 只调用一次，初始化消息数组
    ↓
┌───────────────────────────────────────────────────────────┐
│ Loop (迭代)                                                │
│    ↓                                                       │
│    let messages = run_ctx.get_messages().await            │
│    ↓                                                       │
│    call_llm(messages)                                      │
│    ↓                                                       │
│    if tool_call:                                           │
│        run_ctx.add_message(tool_result).await             │
│        (同时写入 messages 和 session)                         │
└───────────────────────────────────────────────────────────┘
```

---

## 详细设计

### 1. RunContext::init_messages()

```rust
/// 初始化消息数组 - 必须在 loop 前调用一次
/// 
/// 按顺序添加：
/// 1. System 消息（从 config.prompt_context.build_system()）
/// 2. 历史消息（从 session 获取，受 max_history_messages 限制）
/// 3. 用户输入
pub async fn init_messages(&self) -> Result<(), InitError> {
    let mut messages = Vec::new();
    
    // 1. System 消息
    let system_content = self.config.prompt_context.build_system();
    messages.push(Message::system(system_content));
    
    // 2. 历史消息（只获取一次）
    let history = self.session
        .get_messages(self.config.max_history_messages)
        .await
        .unwrap_or_default();
    
    for session_msg in history {
        messages.push(session_msg.message);
    }
    
    // 3. 用户输入
    messages.push(Message::user(self.user_input.clone()));
    
    // 写入共享状态
    *self.messages.write().await = messages;
    
    Ok(())
}
```

### 2. RunContext::add_message()

```rust
/// 添加消息到消息数组，并同步持久化到 session
/// 
/// 用于添加：
/// - Tool call 结果
/// - 运行时动态消息
pub async fn add_message(&self, message: Message) -> Result<(), AddError> {
    // 1. 添加到运行时消息数组
    self.messages.write().await.push(message.clone());
    
    // 2. 持久化到 session
    let session_msg = SessionMessage::new(self.session_id.clone(), message);
    self.session.add_message(session_msg).await?;
    
    Ok(())
}
```

### 3. RunContext::get_messages()

```rust
/// 获取当前消息数组的克隆
/// 
/// 返回：
/// - System 消息
/// - 历史消息
/// - 用户输入
/// - 所有运行时添加的消息（tool results 等）
pub async fn get_messages(&self) -> Vec<Message> {
    self.messages.read().await.clone()
}
```

### 4. ReActAgent::run() 变更

```rust
pub async fn run(...) -> Result<AgentStreamReceiver, AgentError> {
    // Phase 1: 创建 RunContext
    let run_ctx = RunContext::new(...);
    
    // Phase 2: 初始化消息（只调用一次）
    run_ctx.init_messages().await?;
    
    // Phase 3: on_start hooks
    // ...
    
    // Phase 4: Spawn task
    tokio::spawn(async move {
        loop {
            // 从 ctx 获取消息（不重复获取历史）
            let messages = run_ctx.get_messages().await;
            
            // Call LLM
            let request = ConversationRequest::with_history(None, messages);
            let response = llm.converse(request).await?;
            
            // Handle tool calls
            for tool_call in tool_calls {
                let result = tools.execute(tool_call).await?;
                
                // 添加 tool result 到 ctx（同时更新 session）
                run_ctx.add_message(Message::tool(result, tool_call.id)).await?;
            }
        }
    });
    
    // Phase 5: Wrap with plugin stream
    // ...
}
```

---

## 使用示例

### 基础用法

```rust
let agent = ReActAgent::builder()
    .with_llm(llm)
    .with_tools(tools)
    .build()?;

let stream = agent.run("分析市场波动率", context).await?;
// 内部自动调用 run_ctx.init_messages()
```

### 插件访问消息

```rust
#[async_trait]
impl AgentPlugin for ObservabilityPlugin {
    async fn intercept(&self, event: StreamEvent, ctx: &RunContext) -> PluginAction<Option<StreamEvent>> {
        // 访问当前消息历史
        let messages = ctx.get_messages().await;
        
        tracing::info!("Current conversation has {} messages", messages.len());
        
        // 可以基于消息内容做决策
        if messages.len() > 10 {
            tracing::warn!("Conversation getting long");
        }
        
        PluginAction::Continue(Some(event))
    }
}
```

### 插件注入消息

```rust
#[async_trait]
impl AgentPlugin for SecurityPlugin {
    async fn intercept(&self, event: StreamEvent, ctx: &RunContext) -> PluginAction<Option<StreamEvent>> {
        if let Ok(AgentStreamEvent::ToolCallBegin { tool_name, .. }) = &event {
            if tool_name == "dangerous_tool" {
                // 注入警告消息
                ctx.add_message(Message::system("Security warning: dangerous tool requested"))
                    .await?;
            }
        }
        
        PluginAction::Continue(Some(event))
    }
}
```

---

## 迁移计划

### 阶段 1：添加新方法

- `RunContext::init_messages()`
- `RunContext::add_message()` 
- 保持现有 `get_messages()` 不变

### 阶段 2：更新 ReActAgent::run()

- 移除局部 `messages` 变量
- 调用 `run_ctx.init_messages()` 一次
- Loop 中使用 `run_ctx.get_messages()`
- Tool result 使用 `run_ctx.add_message()`

### 阶段 3：清理

- 移除不再需要的代码
- 更新测试

---

## 测试计划

### 单元测试

1. `test_init_messages_system_message` - System 消息正确构建
2. `test_init_messages_history` - 历史消息正确获取
3. `test_init_messages_user_input` - 用户输入正确添加
4. `test_add_message_updates_messages` - add_message 更新消息数组
5. `test_add_message_syncs_to_session` - add_message 同步到 session
6. `test_get_messages_returns_all` - get_messages 返回完整列表
7. `test_init_messages_only_once` - 多次调用 init_messages 不会重复添加

### 集成测试

1. `test_full_react_loop_messages` - 完整 ReAct loop 消息正确累积
2. `test_plugin_can_access_messages` - 插件可以访问消息
3. `test_plugin_can_inject_messages` - 插件可以注入消息
4. `test_history_limit_applied` - 历史消息数量限制生效

---

## 验收标准

1. ✅ 局部 `messages` 变量被移除
2. ✅ `init_messages()` 只在 loop 前调用一次
3. ✅ 历史消息不会重复添加
4. ✅ 插件可以通过 `ctx.get_messages()` 访问消息
5. ✅ 插件可以通过 `ctx.add_message()` 注入消息
6. ✅ 所有现有测试通过
7. ✅ 新增 7+ 单元测试
8. ✅ 新增 4+ 集成测试

---

## 设计决策记录

### 决策 1：初始化 vs 构建

**决策：** 使用 `init_messages()` 一次性初始化，而非每次 `build_messages()`

**理由：**
- 历史消息只获取一次，避免重复
- 性能更优（避免重复 session 查询）
- 语义清晰（初始化 vs 构建）

### 决策 2：同步到 Session

**决策：** `add_message()` 自动同步到 session

**理由：**
- 减少调用方负担（不需要手动调用两次）
- 保证数据一致性（不会漏同步）
- session 持久化是预期行为

### 决策 3：消息数组可变性

**决策：** 使用 `Arc<RwLock<Vec<Message>>>`

**理由：**
- 与现有 RunContext 模式一致
- 支持 async 读写
- 插件可以并发访问
