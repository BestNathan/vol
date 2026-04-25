# vol-llm-task: Task 管理与 Agent 调度设计

## 概述

为 nq-deribit 的 LLM Agent 系统提供 task 管理能力。Agent 可以将大任务拆分为多个子 task，通过 task 管理接口感知进度、协调依赖、汇总结果。

### 职责边界

| Crate | 职责 |
|-------|------|
| `vol-llm-task` | 纯管理：Task 数据模型、状态机、依赖图解析、Store 抽象、查询接口 |
| `vol-llm-agent` | 执行：spawn 子 agent、注册 LLM tools、结果汇总 |

`vol-llm-task` 是一个纯管理 crate，不执行任何 agent 操作。通过 Store trait 抽象实现可替换的持久化后端。

### 设计原则

- **Agent 驱动**：TaskScheduler 只提供查询和状态管理接口，执行由 agent 自行决定
- **Store 抽象**：遵循 `vol-session` 的 Store trait 模式，支持内存/文件/数据库多种实现
- **依赖感知**：支持 task 间的 DAG 依赖，无阻塞依赖的 task 可并行
- **结果摘要**：子 agent 完成后，由父 agent 侧负责 LLM 生成 summary 注入上下文
- **并发安全**：使用 `DashMap` 保证多线程安全

## 数据模型

### TaskStatus

```rust
pub enum TaskStatus {
    Pending,    // 已创建，等待前置依赖完成
    Running,    // 正在执行
    Completed,  // 成功完成
    Failed,     // 执行失败
    Killed,     // 被主动终止
}
```

状态流转：`Pending → Running → Completed | Failed | Killed`

### TaskKind

```rust
pub enum TaskKind {
    Agent {
        agent_type: String,           // "coding", "qa", "advice" 等
        prompt: String,               // 子 agent 的指令
        context_files: Vec<PathBuf>,  // 可选：注入到子 agent 的上下文文件
    },
    Manual,  // 人工标记型 task，用于 todo 拆分
}
```

所有 task 都是分发给 agent 的，没有 shell 类型。

### Task

```rust
pub struct Task {
    pub id: TaskId,
    pub status: TaskStatus,
    pub kind: TaskKind,
    pub description: String,       // 人类可读的描述
    pub dependencies: Vec<TaskId>, // 前置 task ID
    pub result: Option<TaskResult>,
    pub summary: Option<String>,   // LLM 生成的简短总结
    pub output_file: Option<PathBuf>,
    pub created_at: SystemTime,
    pub started_at: Option<SystemTime>,
    pub completed_at: Option<SystemTime>,
}
```

### TaskResult

```rust
pub struct TaskResult {
    pub success: bool,
    pub output_truncated: String,  // 前 2000 字符预览
    pub output_file: PathBuf,      // 完整输出文件路径
}
```

## Store 抽象

遵循 `vol-session` 的 `SessionStore` 模式：

### TaskStore trait

```rust
#[async_trait]
pub trait TaskStore: Send + Sync {
    /// 创建 task
    async fn create(&self, task: Task) -> Result<()>;

    /// 获取 task
    async fn get(&self, task_id: &TaskId) -> Result<Option<Task>>;

    /// 更新 task
    async fn update(&self, task: Task) -> Result<()>;

    /// 删除 task
    async fn delete(&self, task_id: &TaskId) -> Result<()>;

    /// 按状态查询
    async fn list(&self, status: Option<TaskStatus>) -> Result<Vec<Task>>;

    /// 获取所有可执行的 task（Pending 且依赖已满足）
    async fn get_ready_tasks(&self) -> Result<Vec<TaskId>>;
}
```

### 具体实现

| 实现 | 说明 |
|------|------|
| `InMemoryTaskStore` | 基于 `DashMap`，内存模式，零持久化，适合开发和测试 |
| `FileTaskStore` | 基于 JSON 文件，持久化到磁盘，适合生产环境 |
| 未来可扩展 | `SqliteTaskStore`、`PostgresTaskStore` 等 |

### StoreError

```rust
#[derive(Debug, Error)]
pub enum StoreError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, StoreError>;
```

与 `vol-session` 的 `StoreError` 保持一致的错误类型。

## TaskScheduler

TaskScheduler 持有 `Arc<dyn TaskStore>`，不关心具体实现：

```rust
pub struct TaskScheduler {
    store: Arc<dyn TaskStore>,
    work_dir: PathBuf,
}

impl TaskScheduler {
    /// 创建 TaskScheduler，注入 Store 实现
    pub fn new(store: Arc<dyn TaskStore>, work_dir: PathBuf) -> Self;

    /// 创建新 task 并加入调度队列
    pub async fn create_task(&self, kind: TaskKind, description: String, dependencies: Vec<TaskId>) -> Result<TaskId>;

    /// 终止运行中的 task
    pub async fn kill(&self, task_id: &TaskId) -> Result<()>;

    /// 检查是否所有 task 都已完成
    pub async fn all_complete(&self) -> Result<bool>;
}
```

调度逻辑由 `TaskStore::get_ready_tasks()` 提供——返回所有 `Pending` 且依赖已满足的 task ID。agent 侧拿到列表后自行决定如何并发执行，执行完调用 `update()` 回写状态。

### 调度语义

- 无依赖的 task 在 `get_ready_tasks()` 中全部返回，支持并行
- 有依赖的 task 等待所有 `dependencies` 中的 task 状态为 `Completed` 后才返回
- `TaskStore::update()` 中可触发依赖链刷新

## LLM Tools（6 个）

由 `vol-llm-agent` 侧实现和注册，操作 `TaskScheduler` / `TaskStore`。

| Tool | 输入 | 输出 | 说明 |
|------|------|------|------|
| `TaskCreate` | `description`, `agent_type`, `prompt`, `dependencies?` | `task_id` | 创建新 task 并加入调度队列 |
| `TaskList` | `status?`, `agent_type?` | `[{id, status, description, summary}]` | 列出 task 列表 |
| `TaskGet` | `task_id` | 完整 Task 结构 | 查看 task 详情 |
| `TaskUpdate` | `task_id`, `status?`, `description?` | 更新结果 | 标记状态或修改描述 |
| `TaskOutput` | `task_id`, `limit?`, `offset?` | 输出内容 | 读取 task 完整输出文件 |
| `TaskStop` | `task_id` | 成功/失败 | 终止运行中的 task |

## Agent 集成

### 执行流程

```
Agent ReAct Loop
    │
    ├── 收到用户指令
    ├── LLM 决定拆分 task → 调用 TaskCreate × N
    │
    ├── 循环：
    │   1. 调用 TaskList 查看进度
    │   2. TaskScheduler.get_ready_tasks() 获取可执行 task
    │   3. 对每个 ready task，spawn 子 agent 执行
    │   4. 子 agent 完成后，TaskStore.update() 回写状态 + result
    │   5. 父 agent 侧调用 LLM 生成 summary，注入下一轮上下文
    │   6. 重复直到所有 task 完成
    │
    └── 汇总所有 task 结果，生成最终回答
```

### 结果汇总机制

子 agent task 完成后，在父 agent 侧：
1. 子 agent 的完整输出写入 output file
2. 如果输出 < 500 字，直接使用原始内容作为 summary
3. 如果输出 >= 500 字，调用 LLM 生成 1-3 句 summary
4. summary 写入 `Task.summary` 字段，同时注入父 agent 下一轮对话上下文
5. 父 agent 需要详情时，通过 `TaskOutput` 读取完整 output file

### AgentConfig 集成

```rust
pub struct AgentConfig {
    // ... 现有字段 ...
    pub task_scheduler: Option<Arc<TaskScheduler>>,
}
```

Agent 创建时可选加载 `TaskScheduler`。加载后自动注册 6 个 task 相关 tool。

## Crate 结构

```
crates/vol-llm-task/
├── Cargo.toml
├── src/
│   ├── lib.rs          # 公开导出
│   ├── model.rs        # Task, TaskId, TaskStatus, TaskKind, TaskResult
│   ├── store.rs        # TaskStore trait, StoreError, Result type
│   ├── scheduler.rs    # TaskScheduler
│   └── stores/
│       ├── mod.rs
│       ├── memory.rs   # InMemoryTaskStore
│       └── file.rs     # FileTaskStore
└── tests/
    └── scheduler.rs    # 依赖解析、并发测试
```

## 依赖

`vol-llm-task` 的依赖：
- `tokio` — 异步运行时
- `serde` / `serde_json` — 序列化
- `dashmap` — 并发 HashMap（InMemoryTaskStore）
- `async-trait` — 异步 trait 宏
- `thiserror` — 错误类型
- `uuid` — TaskId 生成

`vol-llm-task` 不依赖：
- `vol-llm-agent` — 执行逻辑在 agent 侧
- `vol-llm-provider` — LLM 调用在 agent 侧
- `vol-llm-tools-builtin` — tools 在 agent 侧注册
