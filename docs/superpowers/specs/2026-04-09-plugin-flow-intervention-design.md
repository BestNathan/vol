# ReAct Agent 插件流程干预机制设计

**日期：** 2026-04-09  
**作者：** Claude Code  
**状态：** 设计完成，待实现

---

## 概述

在现有插件系统基础上，实现可以在特定 Agent 事件上插件可以阻断流程的机制。支持两种独立的插件钩子：

- **Interceptor（拦截器）** - 同步、串行、可阻断流程
- **Listener（监听器）** - 异步、并行、仅监听不影响流程

所有事件通过 RunContext 中心化事件总线分发，确保插件可以一致性地访问和干预 Agent 流程。

---

## 问题陈述

### 当前问题

**1. 事件传播与流程控制未分离**

当前 `intercept` 钩子返回 `PluginAction<Option<StreamEvent>>`，混合了事件修改和流程控制：
- `Continue(Some(event))` - 继续并可能修改事件
- `Continue(None)` / `Skip` - 丢弃事件
- `ShortCircuit` - 终止整个流
- `Abort` - 错误终止

但无法精确控制 Agent Loop 的执行（如跳过当前 Tool 但继续 Loop）。

**2. 缺少中心化事件总线**

事件通过 `PluginStream` 的顺序迭代处理，插件无法：
- 主动触发自定义事件
- 在 Agent Loop 的关键位置（如 `on_start`、`on_complete`）接收通知
- 订阅特定类型的事件

**3. Listener 机制缺失**

 observability 插件等只需要监听事件的场景，当前也必须实现 `intercept` 钩子，语义不清晰。

**4. Abort 事件不可见**

插件 Abort 流程时，其他插件无法收到通知，无法做清理或审计。

---

## 设计目标

| 目标 | 描述 | 优先级 |
|------|------|--------|
| **中心化事件总线** | 所有事件通过 RunContext 分发 | 高 |
| **Interceptor 可阻断** | 插件可在关键位置阻断并返回决策 | 高 |
| **Listener 异步监听** | 插件可异步监听事件，不影响流程 | 高 |
| **Abort 事件化** | Abort 本身是事件，可被 Listener 监听 | 高 |
| **插件自定义事件** | 插件可触发自定义事件 | 中 |
| **事件先广播后拦截** | Listener 先收到事件，Interceptor 后决策 | 高 |

---

## 架构设计

### 核心组件

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           RunContext                                     │
│  ┌─────────────────────────────────────────────────────────────────────┐│
│  │                      Event Bus (中心化)                               ││
│  │                                                                      ││
│  │  event_tx: broadcast::Sender<AgentStreamEvent>                      ││
│  │  plugin_event_tx: mpsc::Sender<PluginRequest>                       ││
│  │                                                                      ││
│  │  + emit(event)           - 发送事件到总线                            ││
│  │  + intercept(event)      - 发送并等待 PluginDecision                 ││
│  │  + spawn_listener_task() - 启动 Listener 任务                          ││
│  └─────────────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────────────┘
         │                                    │
         │                                    │
         ▼                                    ▼
┌─────────────────────────────────┐  ┌───────────────────────────────────────┐
│  Listener Task (异步、并行)       │  │  Interceptor (同步、串行、可阻断)     │
│                                 │  │                                       │
│  while let Ok(event) =          │  │  for key_point in agent_loop {        │
│      event_rx.recv().await {    │  │      emit(event);  // 先广播          │
│      spawn_all_listeners(event) │  │      decision = intercept(event);     │
│  }                              │  │      match decision {                 │
│                                 │  │          Continue => execute()        │
│  直到 run 结束                     │  │          Abort => break             │
└─────────────────────────────────┘  │          Skip => continue             │
                                     │      }                                │
                                     │  }                                    │
                                     └───────────────────────────────────────┘
```

### 数据流

```
Agent Loop                          RunContext                      PluginStream
    │                                   │                                  │
    │  1. 产生事件                        │                                  │
    │ ─────────────────────────────────> │                                  │
    │                                   │                                  │
    │  2. emit(event)                   │                                  │
    │     (broadcast)                   │                                  │
    │                                   │ ───────────────────────────────> │
    │                                   │                                   │
    │                                   │  3. Listener Task 并行触发所有 listen()
    │                                   │     (不等待)                      │
    │                                   │                                  │
    │  4. intercept(event)              │                                  │
    │     (oneshot)                     │                                  │
    │ ─────────────────────────────────> │                                  │
    │                                   │ ───────────────────────────────> │
    │                                   │  5. 按 priority 串行执行 intercept()
    │                                   │     遇到 Skip/Abort 停止            │
    │                                   │                                  │
    │                                   │ <─────────────────────────────── │
    │                                   │  PluginDecision                  │
    │ <───────────────────────────────── │                                  │
    │  Decision                         │                                  │
    │                                   │                                  │
    │  6. 根据 Decision 执行                │                                  │
    │     Continue => execute()         │                                  │
    │     Skip => continue              │                                  │
    │     Abort => break + emit(Aborted)│                                  │
    │                                   │                                  │
```

---

## 详细设计

### 1. PluginDecision

```rust
/// Interceptor 返回的决策
#[derive(Debug, Clone)]
pub enum PluginDecision {
    /// 继续，传递给下一个 interceptor
    Continue,
    /// 跳过当前事件（不执行 tool/loop）
    Skip,
    /// 终止整个 agent，携带原因（不一定是错误）
    Abort(String),
}
```

### 2. AgentStreamEvent 扩展

```rust
pub enum AgentStreamEvent {
    /// Agent 启动
    AgentStart { input: String },
    /// LLM 思考完成
    ThinkingComplete { thinking: String },
    /// 即将调用工具（HITL 典型拦截点）
    ToolCallBegin { tool_name: String, arguments: String },
    /// 工具调用完成
    ToolCallComplete { tool_name: String, result: String },
    /// 一次迭代完成
    IterationComplete { 
        iteration: u32, 
        tool_calls: Vec<ToolCall>, 
        final_answer: Option<String>,
    },
    /// Agent 完成
    AgentComplete { response: AgentResponse },
    /// Agent 被中止（新增）
    AgentAborted { reason: String },
    /// 插件自定义事件（新增）
    PluginEvent { 
        name: String, 
        data: Map<String, Value>, 
    },
}
```

### 3. AgentPlugin Trait

```rust
#[async_trait]
pub trait AgentPlugin: Send + Sync {
    /// 插件唯一 ID
    fn id(&self) -> PluginId;
    
    /// 优先级（数字越小优先级越高）
    fn priority(&self) -> u32 { 100 }

    /// Interceptor 钩子（同步、串行、可阻断）
    /// 
    /// 在以下关键位置被调用：
    /// - AgentStart
    /// - ToolCallBegin
    /// - IterationComplete
    /// - AgentComplete
    /// 
    /// 返回 PluginDecision 控制流程：
    /// - Continue: 继续执行
    /// - Skip: 跳过当前事件
    /// - Abort: 终止整个 Agent
    async fn intercept(
        &self, 
        event: &AgentStreamEvent, 
        ctx: &RunContext
    ) -> PluginDecision {
        PluginDecision::Continue  // 默认无处理
    }

    /// Listener 钩子（异步、并行、仅监听）
    /// 
    /// 监听所有事件（包括 AgentAborted 和 PluginEvent）
    /// 不返回任何值，错误由插件自己处理
    async fn listen(
        &self, 
        event: &AgentStreamEvent, 
        ctx: &RunContext
    );
}
```

### 4. RunContext 事件总线

```rust
pub struct RunContext {
    // ... 现有字段
    
    // 事件总线
    event_tx: broadcast::Sender<AgentStreamEvent>,
    plugin_event_tx: mpsc::Sender<PluginRequest>,
}

enum PluginRequest {
    Intercept {
        event: AgentStreamEvent,
        tx: oneshot::Sender<PluginDecision>,
    },
    Emit {
        event: AgentStreamEvent,
    },
}

impl RunContext {
    /// 发送事件到总线（只通知 Listener）
    /// 
    /// 所有事件（包括 AgentAborted 和 PluginEvent）都通过此方法发送
    pub async fn emit(&self, event: AgentStreamEvent) {
        let _ = self.event_tx.send(event);
    }
    
    /// 发送事件并等待 Interceptor 决策
    /// 
    /// 在关键位置调用（ToolCallBegin, AgentStart 等）
    pub async fn intercept(
        &self, 
        event: &AgentStreamEvent
    ) -> Result<PluginDecision, AgentError> {
        let (tx, rx) = oneshot::channel();
        self.plugin_event_tx.send(PluginRequest::Intercept {
            event: event.clone(),
            tx,
        }).await.map_err(|e| AgentError::Context(e.to_string()))?;
        
        rx.await.map_err(|e| AgentError::Context(format!("Plugin channel error: {}", e)))
    }
    
    /// 启动 Listener Task
    /// 
    /// 独立运行，直到 RunContext 被 drop
    fn spawn_listener_task(
        &self, 
        plugins: Vec<Arc<dyn AgentPlugin>>
    ) -> JoinHandle<()> {
        let mut event_rx = self.event_tx.subscribe();
        let ctx = self.clone();
        
        tokio::spawn(async move {
            while let Ok(event) = event_rx.recv().await {
                // 并行触发所有 listener，不等待
                let tasks: Vec<_> = plugins.iter()
                    .map(|p| {
                        let plugin = p.clone();
                        let event = &event;
                        let ctx = &ctx;
                        tokio::spawn(async move {
                            plugin.listen(event, ctx).await;
                        })
                    })
                    .collect();
                
                // 不等待 tasks 完成（fire-and-forget）
            }
        })
    }
}
```

### 5. ReActAgent::run() 变更

```rust
pub async fn run(
    &self,
    user_input: &str,
    context: ToolContext,
) -> Result<AgentStreamReceiver, AgentError> {
    let run_ctx = RunContext::new(...);
    
    // 启动 Listener Task
    let listener_handle = run_ctx.spawn_listener_task(plugins.clone());
    
    run_ctx.init_messages().await?;
    
    // 发送 AgentStart 事件
    let start_event = AgentStreamEvent::AgentStart { 
        input: user_input.to_string() 
    };
    run_ctx.emit(start_event.clone()).await;
    
    // 拦截 AgentStart（插件可以 Abort 启动）
    match run_ctx.intercept(&start_event).await? {
        PluginDecision::Continue => {}
        PluginDecision::Skip => {
            // 跳过启动，返回空响应
            return create_skip_stream(run_ctx, run_id).await;
        }
        PluginDecision::Abort(reason) => {
            run_ctx.emit(AgentStreamEvent::AgentAborted { 
                reason: reason.clone() 
            }).await;
            return Err(AgentError::Context(reason));
        }
    }
    
    tokio::spawn(async move {
        loop {
            run_ctx.next_iteration();
            let iteration = run_ctx.current_iteration();
            
            if iteration > config.max_iterations {
                // 发送 Abort 事件
                run_ctx.emit(AgentStreamEvent::AgentAborted { 
                    reason: "Max iterations reached".to_string() 
                }).await;
                break;
            }
            
            // Call LLM...
            let tool_calls = ...;
            
            for call in &tool_calls {
                // === 1. ToolCallBegin 事件 ===
                let event = AgentStreamEvent::ToolCallBegin {
                    tool_name: call.name.clone(),
                    arguments: call.arguments.clone(),
                };
                
                // 先发送到事件总线
                run_ctx.emit(event.clone()).await;
                
                // 交给 Interceptor 判断
                let decision = run_ctx.intercept(&event).await?;
                
                match decision {
                    PluginDecision::Continue => {
                        // 执行 tool
                        let result = tools.execute(call, &context).await?;
                        
                        // ToolCallComplete 事件
                        let complete_event = AgentStreamEvent::ToolCallComplete {
                            tool_name: call.name.clone(),
                            result: result.content.clone(),
                        };
                        run_ctx.emit(complete_event).await;
                    }
                    PluginDecision::Skip => continue,  // 跳过此 tool
                    PluginDecision::Abort(reason) => {
                        run_ctx.emit(AgentStreamEvent::AgentAborted { 
                            reason: reason.clone() 
                        }).await;
                        break;
                    }
                }
            }
            
            // IterationComplete 事件
            let iter_event = AgentStreamEvent::IterationComplete { ... };
            run_ctx.emit(iter_event.clone()).await;
            
            match run_ctx.intercept(&iter_event).await? {
                PluginDecision::Continue => {}
                PluginDecision::Skip => continue,
                PluginDecision::Abort(reason) => {
                    run_ctx.emit(AgentStreamEvent::AgentAborted { 
                        reason: reason.clone() 
                    }).await;
                    break;
                }
            }
            
            // 无 tool calls，有 final answer
            // AgentComplete 事件
            let complete_event = AgentStreamEvent::AgentComplete { response };
            run_ctx.emit(complete_event.clone()).await;
            
            match run_ctx.intercept(&complete_event).await? {
                PluginDecision::Continue => {}
                PluginDecision::Abort(reason) => {
                    run_ctx.emit(AgentStreamEvent::AgentAborted { 
                        reason: reason.clone() 
                    }).await;
                }
                PluginDecision::Skip => {}  // 无操作
            }
            
            break;  // 正常完成
        }
        
        // Listener Task 清理
        listener_handle.abort();
    });
    
    // ... 返回 stream
}
```

### 6. PluginStream 处理

```rust
/// PluginStream 处理 Intercept 请求
impl PluginStream {
    pub async fn run_interceptor_loop(
        mut plugin_rx: mpsc::Receiver<PluginRequest>,
        plugins: Vec<Arc<dyn AgentPlugin>>,
        ctx: RunContext,
    ) {
        while let Some(msg) = plugin_rx.recv().await {
            match msg {
                PluginRequest::Intercept { event, tx } => {
                    let mut decision = PluginDecision::Continue;
                    for plugin in &plugins {
                        match plugin.intercept(&event, &ctx).await {
                            PluginDecision::Continue => continue,
                            PluginDecision::Skip => { 
                                decision = PluginDecision::Skip; 
                                break; 
                            }
                            PluginDecision::Abort(reason) => { 
                                decision = PluginDecision::Abort(reason); 
                                break; 
                            }
                        }
                    }
                    tx.send(decision).ok();
                }
                PluginRequest::Emit { event } => {
                    // 插件事件只触发 listener，不经过 interceptor
                    let _ = ctx.event_tx.send(event);
                }
            }
        }
    }
}
```

---

## 使用示例

### HITL 插件（Interceptor）

```rust
pub struct HitlPlugin {
    approval_channel: ApprovalChannel,
}

#[async_trait]
impl AgentPlugin for HitlPlugin {
    fn id(&self) -> PluginId { "hitl".to_string() }
    
    async fn intercept(
        &self, 
        event: &AgentStreamEvent, 
        _ctx: &RunContext
    ) -> PluginDecision {
        match event {
            AgentStreamEvent::ToolCallBegin { tool_name, arguments } => {
                // 阻塞等待用户批准
                match self.approval_channel.wait_for_approval(tool_name, arguments).await {
                    ApprovalResult::Approved => PluginDecision::Continue,
                    ApprovalResult::Rejected => PluginDecision::Skip,
                    ApprovalResult::Stop => PluginDecision::Abort("用户终止".into()),
                }
            }
            _ => PluginDecision::Continue,
        }
    }
    
    async fn listen(
        &self, 
        event: &AgentStreamEvent, 
        _ctx: &RunContext
    ) {
        // 监听所有事件（包括 AgentAborted）
        if matches!(event, AgentStreamEvent::AgentAborted { .. }) {
            tracing::info!("HITL: Agent aborted");
        }
    }
}
```

### Observability 插件（Listener）

```rust
pub struct ObservabilityPlugin {
    metrics_tx: MetricsChannel,
}

#[async_trait]
impl AgentPlugin for ObservabilityPlugin {
    fn id(&self) -> PluginId { "observability".to_string() }
    
    // 不实现 intercept，默认 Continue
    
    async fn listen(
        &self, 
        event: &AgentStreamEvent, 
        _ctx: &RunContext
    ) {
        // 发送指标
        let _ = self.metrics_tx.send(Metric::AgentEvent { 
            event_type: event.name(),
            timestamp: Utc::now(),
        }).await;
        
        // 记录日志
        tracing::info!("Agent event: {:?}", event);
    }
}
```

### 插件触发自定义事件

```rust
pub struct AuditPlugin {
    event_tx: mpsc::Sender<AgentStreamEvent>,
}

impl AuditPlugin {
    // 插件可以通过 RunContext 发送自定义事件
    async fn emit_audit_event(&self, ctx: &RunContext, action: &str) {
        let event = AgentStreamEvent::PluginEvent {
            name: "audit".to_string(),
            data: json!({
                "action": action,
                "timestamp": Utc::now(),
            }),
        };
        ctx.plugin_event_tx.send(PluginRequest::Emit { event }).await.ok();
    }
}

#[async_trait]
impl AgentPlugin for AuditPlugin {
    fn id(&self) -> PluginId { "audit".to_string() }
    
    async fn listen(
        &self, 
        event: &AgentStreamEvent, 
        ctx: &RunContext
    ) {
        // 监听特定事件并触发自定义审计事件
        if matches!(event, AgentStreamEvent::ToolCallBegin { .. }) {
            self.emit_audit_event(ctx, "tool_called").await;
        }
    }
}
```

---

## 测试计划

### 单元测试

1. `test_plugin_decision_continue` - Continue 继续执行
2. `test_plugin_decision_skip` - Skip 跳过当前事件
3. `test_plugin_decision_abort` - Abort 终止流程
4. `test_interceptor_chain_order` - Interceptor 按 priority 顺序执行
5. `test_interceptor_abort_stops_chain` - Abort 停止后续 interceptor
6. `test_listener_parallel_execution` - Listener 并行执行
7. `test_listener_does_not_affect_flow` - Listener 不影响流程
8. `test_emit_event_to_bus` - emit 发送事件到总线
9. `test_agent_aborted_event_emitted` - Abort 时发送 AgentAborted 事件
10. `test_plugin_custom_event` - 插件触发自定义事件

### 集成测试

1. `test_hitl_plugin_blocks_tool_call` - HITL 插件阻断 ToolCall
2. `test_observability_plugin_receives_all_events` - Observability 接收所有事件
3. `test_interceptor_abort_triggers_listener` - Abort 事件可被 Listener 监听
4. `test_multiple_interceptors_with_skip` - 多个 Interceptor 中的 Skip 行为
5. `test_full_agent_loop_with_plugins` - 完整 Agent Loop 与插件集成

---

## 验收标准

1. ✅ Interceptor 串行执行，按 priority 排序
2. ✅ Interceptor 可返回 Continue/Skip/Abort 控制流程
3. ✅ Listener 并行执行，不影响流程
4. ✅ 事件先 emit 到总线，再 intercept
5. ✅ AgentAborted 事件在 Abort 时发送
6. ✅ PluginEvent 自定义事件可被插件触发
7. ✅ 插件事件只触发 listener，不经过 interceptor
8. ✅ 所有现有测试通过
9. ✅ 新增 10+ 单元测试
10. ✅ 新增 5+ 集成测试

---

## 设计决策记录

### 决策 1：中心化事件总线

**决策：** 所有事件通过 RunContext 的 broadcast channel 分发

**理由：**
- 单一数据源，插件一致性访问
- Listener 可以订阅所有事件
- 支持插件触发自定义事件

### 决策 2：先 emit 后 intercept

**决策：** 事件先发送到总线（Listener 收到），再交给 Interceptor 决策

**理由：**
- Listener 总是能收到完整事件历史（包括 Abort 前的事件）
- 符合审计和日志的预期行为
- Interceptor 仍然可以阻断后续执行

### 决策 3：Listener 无返回值

**决策：** `listen()` 不返回任何值，错误由插件自己处理

**理由：**
- Listener 语义清晰（只监听不影响）
- 简化调用方（不等待、不处理错误）
- 插件可以在 listen() 内部处理错误（如记录日志）

### 决策 4：Abort 携带 String 原因

**决策：** `PluginDecision::Abort(String)` 而不是 `AgentError`

**理由：**
- Abort 原因不一定是错误（如用户主动终止）
- 简化类型，避免错误处理复杂化
- AgentAborted 事件也使用 String reason

### 决策 5：插件事件不经过 Interceptor

**决策：** 插件触发的自定义事件只发送到总线，不经过 Interceptor

**理由：**
- 避免循环（Interceptor 触发事件 → Interceptor 拦截 → ...）
- 简化流程
- Listener 仍然可以监听插件事件
