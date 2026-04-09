# ReAct Agent RunContext 实现

**日期：** 2026-04-09
**作者：** Claude Code

---

## 概述

本次实现完成了 ReAct Agent 插件系统的 RunContext 统一上下文管理，替代了原有的 PluginContext。

### 核心改进

1. **统一状态管理** - 所有 run 状态集中在 RunContext 中
2. **内部可变性** - 使用 `Arc<RwLock<>>` 和 `AtomicU32` 实现线程安全的可变状态
3. **资源引用** - 直接持有 session、tools、config 的引用，插件可访问

---

## RunContext 结构

```rust
pub struct RunContext {
    // 不可变字段 (run 开始时固定)
    pub run_id: String,
    pub user_input: String,
    pub session_id: String,
    
    // 可变字段 (内部可变性)
    pub iteration: AtomicU32,
    pub messages: Arc<RwLock<Vec<Message>>>,
    pub all_tool_calls: Arc<RwLock<Vec<ToolCall>>>,
    pub current_tool_calls: Arc<RwLock<Vec<ToolCall>>>,
    pub data: Arc<RwLock<HashMap<String, serde_json::Value>>>,
    
    // 资源引用
    pub session: Arc<Session>,
    pub tools: Arc<ToolRegistry>,
    pub config: AgentConfig,
}
```

---

## ReActAgent 核心代码

### Agent 结构

```rust
pub struct ReActAgent {
    llm: Arc<dyn LLMClient>,
    tools: Arc<vol_llm_tool::ToolRegistry>,
    config: AgentConfig,
    session: Arc<Session>,
}
```

### run() 方法 - 4 个阶段

#### Phase 1: 生成 run_id 和创建 RunContext

```rust
let run_id = format!("run_{}", uuid::Uuid::new_v4().simple());

let run_ctx = RunContext::new(
    run_id.clone(),
    user_input.to_string(),
    self.session.id.clone(),
    session,
    tools,
    config,
);
```

#### Phase 2: 执行 on_start hooks

```rust
for plugin in self.config.plugin_registry.plugins() {
    match plugin.on_start(&run_ctx).await {
        PluginAction::Continue(()) => { /* 继续 */ }
        PluginAction::ShortCircuit(response) => { /* 短路返回 */ }
        PluginAction::Skip => { /* 跳过 */ }
        PluginAction::Abort(error) => { /* 中止 */ }
    }
}
```

#### Phase 3: 克隆并 spawn 任务

```rust
let ctx_for_task = run_ctx.clone();
tokio::spawn(async move {
    // 使用 ctx_for_task 访问状态
    ctx_for_task.add_message(msg).await;
    ctx_for_task.next_iteration();
    ctx_for_task.add_tool_call(call).await;
});
```

#### Phase 4: 包装 PluginStream

```rust
let plugin_stream = PluginStream::new(raw_receiver, plugins, run_ctx_for_stream);
Ok(plugin_stream.into_receiver())
```

---

## AgentPlugin Trait

```rust
#[async_trait]
pub trait AgentPlugin: Send + Sync {
    fn id(&self) -> PluginId;
    fn priority(&self) -> u32 { 100 }
    
    async fn on_start(&self, ctx: &RunContext) -> PluginAction<()>;
    
    async fn intercept(
        &self,
        event: StreamEvent,
        ctx: &RunContext,
    ) -> PluginAction<Option<StreamEvent>>;
    
    async fn on_complete(
        &self,
        ctx: &RunContext,
        response: &AgentResponse,
    ) -> PluginAction<()>;
    
    async fn on_error(
        &self,
        ctx: &RunContext,
        error: &AgentError,
    ) -> PluginAction<()>;
}
```

---

## 内置插件

| 插件 | 优先级 | 功能 |
|------|--------|------|
| RateLimiterPlugin | 5 | 并发控制，信号量 |
| ObservabilityPlugin | 10 | 追踪、指标、审计日志 |
| CachingPlugin | 20 | 语义缓存，TTL |
| RetryPlugin | 30 | 指数退避重试 |
| HitlPlugin | 25 | 人工审批 (HITL) |

---

## 测试结果

```
cargo test -p vol-llm-agent

test result: ok. 62 passed; 0 failed
- 49 lib tests
- 4 plugin tests  
- 2 integration tests
- 2 session tests
- 5 doc tests
```

---

## 提交记录

```
7f14862 chore: remove unused helper function from plugin_test.rs
dbb0b66 test: update plugin_test.rs to use RunContext
a912afc feat: create RunContext struct for unified run state management
420dccb feat: replace PluginContext with RunContext in AgentPlugin trait
```

---

**完整代码位置：** `crates/vol-llm-agent/src/react/agent.rs`
**设计文档：** `docs/superpowers/specs/2026-04-09-react-agent-run-context-design.md`
