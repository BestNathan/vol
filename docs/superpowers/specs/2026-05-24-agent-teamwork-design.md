# Design: Agent Teamwork Capability

## Architecture Overview

```
vol-llm-runtime (新 crate)
└── AgentRuntime          // agent 系统核心运行时
    ├── llm_registry: ProviderLoader
    ├── tool_registry: Arc<ToolRegistry>
    ├── task_store: Arc<dyn TaskStore>
    ├── mcp_manager: Arc<McpManager>
    ├── skill_loader: Arc<SkillLoader>
    ├── router: AgentRouter
    ├── agent_defs: Arc<RwLock<HashMap<String, AgentDef>>>
    ├── agent_status: Arc<RwLock<HashMap<String, AgentStatus>>>
    ├── register_agent(id, def) // 内部构造 ReActAgent
    ├── discover_agents()       // 从 .agents/ 加载
    ├── run() -> AgentRuntimeHandle
    └── stop()                  // 优雅退出

vol-llm-agent-channel
└── AgentServerCore { runtime: AgentRuntime, holders, handler_registry }
    ├── handle()  // 协议消息分发
    └── serve()   // 通信循环

vol-agent-manager
└── AppRouterState { runtime: AgentRuntime, ... }
    └── ws/ → 通过 runtime.router.submit() 驱动 agent
```

### Crate dependency graph

```
vol-llm-runtime
  ├── vol-llm-core
  ├── vol-llm-provider
  ├── vol-llm-tool
  ├── vol-llm-agent        // AgentDef, AgentLoader, ReActAgent, AgentConfig
  ├── vol-llm-task         // Task, TaskStore, task tools
  ├── vol-llm-mcp
  ├── vol-llm-skill
  └── vol-session

vol-llm-agent-channel
  ├── vol-llm-runtime      // NEW dependency
  └── vol-llm-core

vol-agent-manager
  ├── vol-llm-runtime      // NEW dependency
  ├── vol-llm-agent-channel
  └── ...
```

---

## AgentRuntime Lifecycle

```
AgentRuntime::builder(working_dir, store_dir)
  .with_handler(extra)      // 可选的额外 handler
  .build()                  // 初始化所有资源，不启动后台任务
    ↓
runtime.run()               // 启动后台
    ├── MCP manager.connect()
    ├── skill_loader.discover_all()
    ├── discover_agents()   // 从 .agents/agents/ 加载并 register_agent()
    └── 返回 AgentRuntimeHandle { shutdown_tx, join_handle }
    ↓
外部通过 router.submit() 驱动 agent
外部通过 handle 监控/控制
    ↓
handle.stop()               // 优雅退出
    ├── 等待正在运行的 agent 完成（graceful timeout）
    ├── 断开 MCP 连接
    ├── 持久化状态
    └── join_handle.await
```

### Key types

```rust
pub struct AgentRuntime {
    llm_registry: ProviderLoader,
    tool_registry: Arc<ToolRegistry>,
    task_store: Arc<dyn TaskStore>,
    mcp_manager: Arc<McpManager>,
    skill_loader: Arc<SkillLoader>,
    router: AgentRouter,
    agent_defs: Arc<RwLock<HashMap<String, AgentDef>>>,
    agent_status: Arc<RwLock<HashMap<String, AgentStatus>>>,
}

pub struct AgentRuntimeHandle {
    pub shutdown_tx: tokio::sync::oneshot::Sender<()>,
    pub join_handle: tokio::task::JoinHandle<()>,
}
```

### Design decisions

- `new()` / `build()` and `run()` are separate — resources can be injected between construction and startup
- `register_agent()` works both before and after `run()` (runtime dynamic registration)
- `stop()` is idempotent

---

## Task Model Extension

### Fields added to `Task` (vol-llm-task)

```rust
pub struct Task {
    // ... existing fields ...
    pub publisher: Option<String>,   // agent type, auto-set on creation
    pub assignee: Option<String>,    // agent type, set at publish or claim time
}
```

### ToolContext extension (vol-llm-tool)

```rust
pub struct ToolContext {
    pub messages: Vec<Message>,
    pub sandbox: Option<SandboxRef>,
    pub agent_def: Option<AgentDef>,  // NEW — filled by ReActAgent at tool execution
}
```

`ReActAgent.run()` populates `ToolContext.agent_def` from `self.config.def` before each `tool_registry.execute()` call.

---

## Tool Changes

### task_create — extended

| Param | Required | Description |
|-------|----------|-------------|
| `subject` | yes | Task subject |
| `description` | yes | Task description |
| `assignee` | no | Target agent type; omit for open claim |

Execution:
- `publisher` auto-filled from `ToolContext.agent_def.r#type`
- `assignee` from params (if provided)
- Creates task with `status=Pending, kind=Agent`

### task_claim — new tool

| Param | Required | Description |
|-------|----------|-------------|
| `taskId` | yes | Task ID to claim |

Execution flow:
1. Read `claimant_type` from `ToolContext.agent_def.r#type`
2. `store.get(task_id)` — verify exists
3. Check `task.status == Pending` (else error)
4. Check `task.dependencies` all completed via `store.get_ready_tasks()` (else error)
5. Set `task.status = Running`, `task.assignee = Some(claimant_type)`
6. `store.update(task)`
7. Return task content (`subject` + `description`) as `ToolResult.content` — LLM processes it in the current ReAct loop

Atomicity: single writer per TaskStore (DashMap or file lock) ensures only one claim succeeds.

### task_list — enhanced

| Param | Required | Description |
|-------|----------|-------------|
| `status` | no | Filter by status (pending/running/completed/failed/killed) |
| `assignee` | no | `"me"` (current agent), specific agent_type, or `"unassigned"` |

`"me"` resolves from `ToolContext.agent_def.r#type`.

---

## AgentServerCore Refactoring

### Before (current)

```
AgentServerCore owns: llm, tool_registry, mcp_manager, skill_loader,
                       router, holders, agent_defs, agent_status,
                       handler_registry
```

### After

```
AgentServerCore {
    runtime: AgentRuntime,
    holders: Arc<Mutex<HashMap<String, Arc<ConnectionHolder>>>>,
    handler_registry: HandlerRegistry,
}
```

All runtime resources (llm_registry, tool_registry, mcp_manager, skill_loader, router, agent_defs, agent_status) live in `AgentRuntime`. AgentServerCore accesses them via `self.runtime.*`.

`AgentServerCore::register_agent()` delegates to `self.runtime.register_agent()`.

`AgentServerCoreBuilder` becomes a thin wrapper:
1. Create `AgentRuntime::builder(working_dir, store_dir).build()`
2. Register extra handlers via `agent_runtime` resources
3. Wrap in `AgentServerCore { runtime, holders, handler_registry }`

---

## Web Backend Integration (vol-agent-manager)

`vol-agent-manager` adopts `AgentRuntime` as its agent management layer:

```rust
// main.rs
let runtime = AgentRuntime::builder(working_dir, store_dir)
    .build()
    .await?;
let runtime_handle = runtime.run().await;

let app_state = AppRouterState {
    runtime: Arc::new(runtime),
    // ... other state
};
```

`ws/router.rs` changes: instead of constructing `AgentConfig` + `ReActAgent` manually, submit input through `runtime.router().submit(agent_type, input)`. The runtime internally manages agent lifecycle.

---

## Error Handling

| Scenario | Error |
|----------|-------|
| `task_create` with invalid assignee | Succeeds (no existence check); task stays unclaimed |
| `task_claim` called on non-Pending task | `"task is not in Pending status (current: {status})"` |
| `task_claim` called with uncompleted dependencies | `"task has uncompleted dependencies: [t1, t2]"` |
| `task_claim` without agent context | `"agent identity required for task_claim"` |
| Concurrent claim on same task | Second caller gets `"task is not in Pending status"` |

---

## Testing Strategy

- **Unit tests**: Task model serialization, task_create/claim extension logic, task_list filtering
- **Integration tests**: Two agents created via runtime, one publishes task, other claims and executes
- **Concurrency test**: Two agents attempt to claim same task simultaneously

---

## Files Changed

| File | Change |
|------|--------|
| `crates/vol-llm-runtime/src/lib.rs` | NEW — AgentRuntime, AgentRuntimeBuilder, AgentRuntimeHandle |
| `crates/vol-llm-runtime/Cargo.toml` | NEW — dependencies |
| `crates/vol-llm-task/src/model.rs` | Add `publisher`, `assignee` to Task |
| `crates/vol-llm-task/src/tools/task_create.rs` | Add `assignee` param, auto-fill `publisher` from context |
| `crates/vol-llm-task/src/tools/task_claim.rs` | NEW — task_claim tool |
| `crates/vol-llm-task/src/tools/task_list.rs` | Add `assignee` filter param |
| `crates/vol-llm-task/src/tools/mod.rs` | Register task_claim |
| `crates/vol-llm-tool/src/tool.rs` | Add `agent_def` to ToolContext |
| `crates/vol-llm-agent/src/react/agent.rs` | ReActAgent populates ToolContext.agent_def |
| `crates/vol-llm-agent-channel/src/server_core.rs` | Use AgentRuntime internally |
| `crates/vol-llm-agent-channel/Cargo.toml` | Add vol-llm-runtime dependency |
| `crates/vol-agent-manager/src/main.rs` | Create AgentRuntime, pass to state |
| `crates/vol-agent-manager/src/ws/router.rs` | Use runtime.router.submit() |
| `crates/vol-agent-manager/Cargo.toml` | Add vol-llm-runtime dependency |
