# ObserverPlugin Integration Design

**Date**: 2026-04-12
**Author**: Claude Code
**Status**: Approved

---

## Context

当前 Coding Agent MVP 实现中，`EventObserver` 只能在 `run()` 方法的开始和结束时接收 `AgentStart` 和 `AgentComplete` 事件，无法捕获中间的详细事件（`ThinkingComplete`、`ToolCallBegin`、`ToolCallComplete`、`IterationComplete`）。这导致 HTML 报告只显示开始和结束，缺少完整的执行时间线。

**约束条件**:
- 不修改 ReActAgent 核心逻辑（符合原始设计原则）
- 使用现有 PluginRegistry 机制
- 事件驱动、非侵入式架构
- Observer 失败不应影响其他插件执行

---

## Goals / Non-Goals

**Goals:**
- 实现 ObserverPlugin 包装器，将 EventObserver 集成到 PluginRegistry
- HTML 报告捕获所有 AgentStreamEvent 事件
- 保持事件驱动、非侵入式架构
- 错误隔离：Observer 失败不影响其他插件

**Non-Goals:**
- 修改 ReActAgent 核心事件广播机制
- 支持多个 Observer（MVP 仅支持单个）
- 实时流式报告（仍为结束后生成）

---

## Architecture

### 整体架构

```
┌─────────────────────────────────────────────────────────────┐
│                     CodingAgent                              │
│  - Holds ObserverPlugin                                      │
│  - Registers plugin on run()                                 │
└─────────────────────────────────────────────────────────────┘
         │
         │ creates & registers
         ▼
┌─────────────────────────────────────────────────────────────┐
│                  ObserverPlugin                              │
│  - Wraps Arc<EventObserver>                                  │
│  - Implements AgentPlugin trait                              │
│  - listen() forwards events to observer                      │
└─────────────────────────────────────────────────────────────┘
         │
         │ integrates with
         ▼
┌─────────────────────────────────────────────────────────────┐
│                  PluginRegistry                              │
│  - Existing mechanism from vol-llm-agent                     │
│  - Calls plugin.listen() on all events                       │
└─────────────────────────────────────────────────────────────┘
         │
         │ receives events from
         ▼
┌─────────────────────────────────────────────────────────────┐
│                    ReActAgent                                │
│  - Emits AgentStreamEvent via broadcast channel              │
│  - spawn_listener_task calls all plugin.listen()             │
└─────────────────────────────────────────────────────────────┘
```

### 事件流

```
1. CodingAgent::new(config)
   → PluginRegistry created empty

2. CodingAgent::with_observer(observer)
   → ObserverPlugin created, stored in CodingAgent

3. CodingAgent::run(task)
   → ObserverPlugin registered to PluginRegistry
   → ReActAgent::run() called
   
4. ReActAgent emits events:
   AgentStart ─────────┐
   ThinkingComplete    │
   ToolCallBegin       │
   ToolCallComplete    ├─→ spawn_listener_task
   IterationComplete   │    │
   ... (repeat)        │    ▼
   AgentComplete ──────┘    plugin.listen() for each plugin
                                 │
                                 ▼
                         ObserverPlugin.listen()
                                 │
                                 ▼
                         EventObserver.on_event()
                                 │
                                 ▼
                         HTMLReporter records event

5. AgentComplete triggers on_complete()
   → HTMLReporter generates report
```

---

## Component Design

### 1. ObserverPlugin

```rust
/// ObserverPlugin - wraps EventObserver and implements AgentPlugin
pub struct ObserverPlugin {
    observer: Arc<dyn EventObserver>,
}

impl ObserverPlugin {
    pub fn new(observer: Arc<dyn EventObserver>) -> Self {
        Self { observer }
    }
}

impl AgentPlugin for ObserverPlugin {
    fn id(&self) -> PluginId {
        "observer".to_string()
    }

    fn priority(&self) -> u32 {
        0 // Low priority, runs after other plugins
    }

    async fn listen(&self, event: &AgentStreamEvent, _ctx: &PluginContext) {
        // Forward to observer, ignore errors to not block other plugins
        let _ = self.observer.on_event(event).await;
    }
}
```

**职责**:
- 实现 `AgentPlugin` trait
- `listen()` 方法转发所有事件到 `EventObserver`
- 错误隔离：`on_event()` 失败不影响其他插件

### 2. CodingAgent 修改

```rust
pub struct CodingAgent {
    config: CodingAgentConfig,
    react_agent: ReActAgent,
    observer: Option<Arc<dyn EventObserver>>,
    observer_plugin: Option<Arc<ObserverPlugin>>, // New field
}

pub fn with_observer(mut self, observer: Arc<dyn EventObserver>) -> Self {
    let plugin = Arc::new(ObserverPlugin::new(observer.clone()));
    self.observer = Some(observer);
    self.observer_plugin = Some(plugin);
    self
}

pub async fn run(&self, task: &str) -> Result<CodingAgentResponse, CodingAgentError> {
    // Register observer plugin if present
    if let Some(ref plugin) = self.observer_plugin {
        // Note: PluginRegistry is inside AgentConfig, need mutable access
        // This requires modifying how plugin_registry is accessed
    }

    // Run agent - events will be forwarded to observer via plugin
    let response = self.react_agent.run(task).await?;

    // Extract response...
    Ok(CodingAgentResponse { ... })
}
```

### 3. HTMLReporter 行为

```rust
impl EventObserver for HTMLReporter {
    async fn on_event(&self, event: &AgentStreamEvent) -> Result<(), ObserverError> {
        // Record start time on first event
        if self.start_time.lock().unwrap().is_none() {
            *self.start_time.lock().unwrap() = Some(Instant::now());
        }

        // Record all events
        self.events.lock().unwrap().push(event.clone());
        Ok(())
    }

    async fn on_complete(&self) -> Result<(), ObserverError> {
        // Generate report on AgentComplete event
        let events: Vec<AgentStreamEvent> = self.events.lock().unwrap().drain(..).collect();
        self.generate_html_report(events).await
    }
}
```

---

## Error Handling

**Observer 错误隔离**:
```rust
async fn listen(&self, event: &AgentStreamEvent, _ctx: &PluginContext) {
    // Ignore errors - observer failure should not block other plugins
    let _ = self.observer.on_event(event).await;
}
```

**报告生成失败**:
- `on_complete()` 返回 `Result<(), ObserverError>`
- `CodingAgent::run()` 中记录错误但不中断主流程

---

## Testing Strategy

### 单元测试

- `ObserverPlugin::new()` - 验证包装器创建
- `ObserverPlugin::listen()` - 验证事件转发
- `ObserverPlugin::id()` / `priority()` - 验证插件标识

### 集成测试

- 验证 Observer 接收所有事件类型
- 验证 Observer 错误不影响其他插件
- 验证 HTML 报告包含完整时间线

### 端到端测试

- 运行实际 coding 任务
- 验证 HTML 报告包含 Thinking/ToolCall/Iteration 事件
- 验证报告时间与事件数量准确

---

## Implementation Notes

### 关键挑战

**PluginRegistry 访问**:
- `PluginRegistry` 存储在 `AgentConfig` 中
- `ReActAgent::run()` 时 `AgentConfig` 是只读的
- 需要在 `run()` 之前注册插件

**解决方案**:
在 `CodingAgent::with_observer()` 时直接注册到 `PluginRegistry`，而不是在 `run()` 时：

```rust
pub fn with_observer(mut self, observer: Arc<dyn EventObserver>) -> Self {
    let plugin = Arc::new(ObserverPlugin::new(observer.clone()));
    
    // Access plugin_registry and register
    // Note: This requires PluginRegistry to be mutable
    // May need to restructure how CodingAgent holds ReActAgent
    
    self.observer = Some(observer);
    self
}
```

或者，在 `CodingAgent::new()` 完成后通过 builder 模式注册：

```rust
let config = CodingAgentConfig { ... };
let mut agent = CodingAgent::new(config).await?;
agent.register_observer(observer); // Mutable method
```

---

## Risks / Trade-offs

| Risk | Impact | Mitigation |
|------|--------|------------|
| PluginRegistry 访问复杂 | 中 | 使用 builder 模式或重构存储方式 |
| Observer 错误被忽略 | 低 | 记录 tracing 日志用于调试 |
| 多个 Observer 冲突 | 低 | MVP 仅支持单个，未来可扩展 |

---

## Open Questions

| Question | Decision |
|----------|----------|
| ObserverPlugin 存储方式 | CodingAgent 自己保存 |
| 错误处理策略 | 忽略错误，不影响其他插件 |
| 完成信号 | 仅依赖 AgentComplete 事件 |

---

## Implementation Phases

| Phase | Tasks | Duration |
|-------|-------|----------|
| **1** | Create ObserverPlugin struct + AgentPlugin impl | 1 hour |
| **2** | Update CodingAgent to hold and register ObserverPlugin | 1 hour |
| **3** | Update HTMLReporter to handle all event types | 1 hour |
| **4** | Test and verify complete timeline in HTML report | 1 hour |
