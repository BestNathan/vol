# Coding Agent Design

**Date**: 2026-04-12
**Author**: Claude Code
**Status**: Draft

---

## Context

当前 vol-llm-agents 包中已有 PPT Agent、QaAgent、AdviceAgent 等业务 Agent，但缺少通用的代码助手 Agent。需要新增 `coding` 子包，实现一个能够理解、修改、审查代码的 coding agent。

**约束条件**:
- 复用现有 ReActAgent 基础设施
- 使用已实现的 vol-llm-tools-builtin 工具（Read, Edit, Bash 等）
- 事件驱动架构，不侵入 ReAct Agent 核心逻辑
- MVP 先跑通核心流程，支持未来扩展到生产级 Dashboard

---

## Goals / Non-Goals

**Goals:**
- 在 `vol-llm-agents` 中新增 `coding` 子包
- 实现 CodingAgent 封装，提供便捷的配置和启动接口
- 集成核心工具（ReadTool, EditTool, BashTool）
- 实现 HTML 时间线报告生成器（基于 AgentStreamEvent）
- 支持 HITL 确认机制（危险操作需要用户确认）
- 事件驱动，可替换的 Observer 架构

**Non-Goals:**
- 实时流式 HTML 更新（MVP 仅支持结束后生成）
- 生产级 Dashboard 集成（预留扩展性，但 MVP 不实现）
- 复杂的代码分析功能（如 AST 解析、类型推断）

---

## Architecture

### 整体架构

```
┌─────────────────────────────────────────────────────────────┐
│                     CodingAgent                              │
│  (vol-llm-agents/coding)                                     │
└─────────────────────────────────────────────────────────────┘
         │
         │ uses
         ▼
┌─────────────────────────────────────────────────────────────┐
│                    ReActAgent                                │
│  (vol-llm-agent)                                             │
└─────────────────────────────────────────────────────────────┘
         │
         │ emits
         ▼
┌─────────────────────────────────────────────────────────────┐
│              AgentStreamEvent (broadcast channel)            │
│  - AgentStart / AgentComplete                                │
│  - ThinkingComplete                                          │
│  - ToolCallBegin / ToolCallComplete                          │
│  - IterationComplete                                         │
└─────────────────────────────────────────────────────────────┘
         │
         │ observes
         ▼
┌─────────────────────────────────────────────────────────────┐
│              EventObserver (trait + implementations)         │
│  ┌─────────────────┐  ┌─────────────────┐                   │
│  │ HTMLReporter    │  │ (future: DBWriter)│                  │
│  │ (MVP impl)      │  │ (dashboard prep) │                   │
│  └─────────────────┘  └─────────────────┘                   │
└─────────────────────────────────────────────────────────────┘
```

### 核心组件

#### 1. CodingAgent

```rust
pub struct CodingAgent {
    config: CodingAgentConfig,
    react_agent: ReActAgent,
    observer: Box<dyn EventObserver>,
}

pub struct CodingAgentConfig {
    pub max_iterations: u32,
    pub working_dir: PathBuf,
    pub hitl_enabled: bool,
    pub verbose: bool,
    pub html_report_path: Option<PathBuf>,
}
```

**职责**:
- 封装 ReActAgent 创建和配置
- 注册核心工具（ReadTool, EditTool, BashTool）
- 绑定 EventObserver
- 提供简洁的 `run(task: String)` 接口

#### 2. EventObserver (trait)

```rust
#[async_trait]
pub trait EventObserver: Send + Sync {
    async fn on_event(&self, event: &AgentStreamEvent) -> Result<(), ObserverError>;
    async fn on_complete(&self) -> Result<(), ObserverError>;
}
```

**MVP 实现**: `HTMLReporter`
- 监听所有 `AgentStreamEvent`
- 结束后生成 HTML 时间线报告

**未来扩展**:
- `DatabaseWriter` - 写入数据库供 Dashboard 查询
- `WebSocketPusher` - 实时推送事件到前端
- `LoggerObserver` - 结构化日志

#### 3. HTMLReporter

```rust
pub struct HTMLReporter {
    output_path: PathBuf,
    events: Mutex<Vec<AgentStreamEvent>>,
    start_time: Instant,
}

impl EventObserver for HTMLReporter {
    async fn on_event(&self, event: &AgentStreamEvent) {
        self.events.lock().push(event.clone());
    }

    async fn on_complete(&self) {
        let events = self.events.lock().drain(..).collect();
        self.generate_html_report(events).await;
    }
}
```

**HTML 报告结构**:
```html
<!DOCTYPE html>
<html>
<head><title>Coding Agent Report - {task}</title></head>
<body>
  <h1>Coding Agent Report</h1>
  <div class="summary">
    <p>Task: {task_description}</p>
    <p>Duration: {duration}s | Iterations: {N} | Tool Calls: {M}</p>
  </div>
  <div class="timeline">
    <!-- 时间线事件 -->
  </div>
</body>
</html>
```

---

## Component Design

### 1. CodingAgent 使用示例

```rust
// 创建 agent
let config = CodingAgentConfig {
    max_iterations: 10,
    working_dir: PathBuf::from("/path/to/project"),
    hitl_enabled: true,
    verbose: false,
    html_report_path: Some("reports/coding-report.html".into()),
};

let agent = CodingAgent::new(config);

// 执行任务
let result = agent.run("Add a new API endpoint for user login").await?;

println!("Task completed: {}", result.summary);
```

### 2. 核心工具注册

```rust
fn register_coding_tools(registry: &mut ToolRegistry) {
    // 代码读取
    registry.register(ReadTool::new());
    
    // 代码修改
    registry.register(EditTool::new());
    
    // 编译/测试
    registry.register(BashTool::new());
    
    // 可选：文件搜索
    registry.register(GlobTool::new());
    
    // 可选：内容搜索
    registry.register(GrepTool::new());
}
```

### 3. HITL 确认机制

危险操作列表（需要用户确认）：
- 删除文件（`rm`, `DeleteTool`）
- 覆盖重要配置文件
- 执行系统级命令（格式化磁盘、修改系统配置等）
- 批量修改多个文件（>10 个）

```rust
pub enum HITLDecision {
    Approve,
    Reject { reason: String },
    Modify { new_command: String },
}

pub async fn check_dangerous_operation(
    operation: &str,
    details: &str,
) -> Result<HITLDecision, HITLError> {
    // 通过 HTTP 或 CLI 请求用户确认
    // 详见插件系统设计
}
```

---

## Error Handling

### 统一错误类型

```rust
#[derive(Debug, thiserror::Error)]
pub enum CodingAgentError {
    #[error("Agent error: {0}")]
    Agent(#[from] vol_llm_agent::AgentError),

    #[error("Tool error: {0}")]
    Tool(#[from] vol_llm_tool::ToolError),

    #[error("Observer error: {0}")]
    Observer(#[from] ObserverError),

    #[error("HITL error: {0}")]
    HITL(#[from] HITLError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
```

---

## Testing Strategy

### 单元测试

- `CodingAgent::new()` - 验证配置解析
- `HTMLReporter::generate_report()` - 验证 HTML 生成
- HITL 决策逻辑测试

### 集成测试

- 完整任务流程：读取 → 修改 → 测试
- 危险操作拦截测试
- HTML 报告内容验证

### 端到端测试

- 真实项目场景测试
- 多文件修改场景
- 长任务（多轮迭代）场景

---

## Risks / Trade-offs

| Risk | Impact | Mitigation |
|------|--------|------------|
| HTML 报告过大 | 中 | 限制最大事件数，支持折叠/分页 |
| HITL 打断流畅性 | 低 | 可配置 HITL 触发条件 |
| 工具执行副作用 | 高 | 危险操作黑名单 + HITL 确认 |
| Observer 性能开销 | 低 | 异步处理，不阻塞 agent |

---

## Implementation Phases

| Phase | Tasks | Duration |
|-------|-------|----------|
| **1** | CodingAgent 基础结构，工具注册 | 1 day |
| **2** | EventObserver trait + HTMLReporter | 1 day |
| **3** | HITL 确认机制集成 | 1 day |
| **4** | 测试 + 文档 | 1 day |

---

## Open Questions

| Question | Decision |
|----------|----------|
| HTML 报告更新方式 | 结束后一次性生成（MVP） |
| Observer 接口设计 | 基于 AgentStreamEvent 事件流 |
| HITL 触发条件 | 危险操作需要确认 |
| 核心工具最小集 | Read, Edit, Bash |

---

## Future Extensions (Post-MVP)

1. **Dashboard 集成** - DatabaseWriter 实现，WebSocket 实时推送
2. **智能代码分析** - AST 解析，代码依赖图谱
3. **多文件批量编辑** - 事务性修改 + 回滚支持
4. **知识库集成** - RAG 支持项目特定知识
5. **协作功能** - 多用户评论/审批工作流
