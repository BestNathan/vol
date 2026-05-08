# vol-llm-ui 架构设计

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 构建基于 Dioxus 的跨平台 UI（Web WASM + TUI），共享核心业务逻辑，支持本地和远程两种 agent 交互模式。

**Architecture:** 三层分离——共享核心库（状态/事件/通信）+ 两个渲染前端（ratatui TUI / Dioxus Web WASM）+ 远程 JSON-RPC WebSocket 服务。

**Tech Stack:** Rust, Dioxus 0.6+ (Web/WASM), ratatui (TUI), jsonrpsee (JSON-RPC 2.0 over WebSocket), tokio, axum

---

### File Map

| File | Responsibility |
|------|----------------|
| `crates/vol-llm-ui/Cargo.toml` | Workspace crate with two bins + lib |
| `crates/vol-llm-ui/src/lib.rs` | Shared core: `UiState`, `UiEvent`, hooks |
| `crates/vol-llm-ui/src/state/mod.rs` | `UiState` struct and `ConversationEntry`/`ToolCallEntry` types |
| `crates/vol-llm-ui/src/state/event_buffer.rs` | `AgentStreamEvent` → `UiEvent` → `UiState` mutation logic (migrated from `vol-llm-tui/src/render.rs`) |
| `crates/vol-llm-ui/src/state/workspace.rs` | Workspace file tree scanning |
| `crates/vol-llm-ui/src/connection/mod.rs` | `AgentConnection` trait + `UiEvent` stream type |
| `crates/vol-llm-ui/src/connection/local.rs` | Local in-process connection (direct ReActAgent) |
| `crates/vol-llm-ui/src/connection/remote.rs` | Remote JSON-RPC WebSocket connection |
| `crates/vol-llm-ui/src/hooks/` | `use_event_handler`, `use_agent`, `use_session`, `use_hitl` |
| `crates/vol-llm-ui/src/tui/bin/tui.rs` | TUI binary entry point (ratatui) |
| `crates/vol-llm-ui/src/tui/render.rs` | ratatui render functions (migrated/adapted from `vol-llm-tui/src/ui/`) |
| `crates/vol-llm-ui/src/tui/input.rs` | Keyboard input handling for ratatui |
| `crates/vol-llm-ui/src/web/bin/web.rs` | Web binary entry point (Dioxus WASM) |
| `crates/vol-llm-ui/src/web/components/` | Dioxus component definitions |
| `crates/vol-llm-agent-channel/src/jsonrpc/` | JSON-RPC WebSocket server implementation |
| `crates/vol-llm-agent-channel/src/jsonrpc/handler.rs` | JSON-RPC method handlers (agent, file, log, session) |
| `crates/vol-llm-agent-channel/examples/agent-service.rs` | Runnable remote agent service binary |

---

## Task 1: 共享状态模型 (`UiState` + `UiEvent` + `EventBuffer`)

**Files:**
- Create: `crates/vol-llm-ui/src/state/mod.rs`
- Create: `crates/vol-llm-ui/src/state/event_buffer.rs`
- Create: `crates/vol-llm-ui/src/state/workspace.rs`
- Modify: `crates/vol-llm-ui/src/lib.rs`

**Step 1: 定义 `UiState` 和事件类型**

```rust
// vol-llm-ui/src/state/mod.rs

use serde::{Serialize, Deserialize};
use std::collections::HashSet;
use std::time::Instant;

/// 统一事件类型——本地 AgentStreamEvent 和远程 JSON-RPC 事件都映射到此
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UiEvent {
    AgentStart { input: String },
    ThinkingStart,
    ThinkingDelta { delta: String },
    ThinkingComplete,
    ContentStart,
    ContentDelta { delta: String },
    ContentComplete { content: String },
    ToolCallBegin { tool_name: String, arguments: String },
    ToolCallArgumentDelta { delta: String },
    ToolCallComplete { tool_name: String, result: String, duration_ms: Option<u64> },
    ToolCallError { tool_name: String, error: String, duration_ms: Option<u64> },
    ToolCallSkipped { tool_name: String, reason: String, duration_ms: Option<u64> },
    AgentComplete { response: String },
    AgentAborted { reason: String },
    AgentError { message: String },
    MaxIterationsReached { current: u32, max: u32 },
    IterationContinued { from_iteration: u32 },
    IterationComplete { iteration: u32, final_answer: Option<String> },
    ApprovalRequest { tool_name: String, reason: String, arguments: String },
    ApprovalResolved { approved: bool },
}

#[derive(Debug, Clone)]
pub enum ToolCallStatus { Running, Success, Error, Skipped }

#[derive(Debug, Clone)]
pub struct ToolCallEntry {
    pub sequence: u32,
    pub tool_name: String,
    pub arg_preview: String,
    pub status: ToolCallStatus,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone)]
pub enum ConversationEntry {
    UserInput { text: String },
    Thinking { content: String },
    ContentStreaming { content: String },
    ToolCall { tool_name: String, arg_preview: String },
    ToolResult { tool_name: String, preview: String, success: bool },
    AgentAnswer { text: String },
    RunSummary { iterations: u32, tool_calls: u32, elapsed_ms: u128 },
    Error { message: String },
}

#[derive(Debug, Clone)]
pub struct WorkspaceTree {
    pub root: String,
    pub entries: Vec<WorkspaceEntry>,
}

#[derive(Debug, Clone)]
pub struct WorkspaceEntry {
    pub path: String,
    pub is_dir: bool,
    pub modified: bool,
    pub indent: usize,
}

pub struct ApprovalState {
    pub tool_name: Option<String>,
    pub reason: Option<String>,
    pub arguments: Option<String>,
    pub response: Option<(bool, Option<String>)>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ActiveTab { Conversation, Workspace, Skills, Logs }

pub struct UiState {
    pub session_id: String,
    pub run_count: u32,
    pub iteration: u32,
    pub tool_call_count: u32,
    pub run_start: Option<Instant>,
    pub run_elapsed: std::time::Duration,
    pub is_running: bool,
    pub exiting: bool,
    pub conversation: Vec<ConversationEntry>,
    pub tool_calls: Vec<ToolCallEntry>,
    pub workspace: WorkspaceTree,
    pub modified_files: HashSet<String>,
    pub active_tab: ActiveTab,
    pub conversation_scroll: u16,
    pub workspace_scroll: u16,
    pub tools_scroll: u16,
    pub conversation_auto_scroll: bool,
    pub approval_state: ApprovalState,
    pub last_error: Option<String>,
}

impl UiState {
    pub fn new(session_id: String, working_dir: &str) -> Self { ... }
    pub fn reset_for_run(&mut self) { ... }
}
```

- [ ] **Step: 创建 `state/mod.rs`，定义所有类型和 `UiState`**

**Step 2: 迁移 EventBuffer**

将现有 `vol-llm-tui/src/render.rs` 中的 `EventBuffer::apply()` 逻辑迁移，改为接收 `AgentStreamEvent` 输出 `UiEvent`：

```rust
// vol-llm-ui/src/state/event_buffer.rs

use crate::state::{UiState, UiEvent, ConversationEntry, ToolCallEntry, ToolCallStatus};
use vol_llm_core::AgentStreamEvent;

pub struct EventBuffer {
    thinking_active: bool,
    thinking_buffer: String,
    content_buffer: String,
}

impl EventBuffer {
    pub fn new() -> Self { ... }

    /// 将 AgentStreamEvent 转为 UiEvent 并应用到 state
    pub fn apply_stream(&mut self, event: &AgentStreamEvent, state: &mut UiState) {
        let ui_event = self.to_ui_event(event);
        if let Some(e) = ui_event {
            state.apply(e);
        }
    }

    /// 远程模式：直接应用 UiEvent
    pub fn apply_event(&mut self, event: &UiEvent, state: &mut UiState) {
        state.apply(event.clone());
    }

    fn to_ui_event(&mut self, event: &AgentStreamEvent) -> Option<UiEvent> { ... }
}
```

- [ ] **Step: 创建 `state/event_buffer.rs`，迁移现有 render.rs 事件处理逻辑**

**Step 3: workspace 扫描**

```rust
// vol-llm-ui/src/state/workspace.rs
pub fn scan_workspace(root: &str) -> WorkspaceTree { ... }
```

- [ ] **Step: 创建 `state/workspace.rs`**
- [ ] **Step: 创建 `lib.rs` 导出所有公开类型**

---

## Task 2: 通信抽象层 (`AgentConnection` trait)

**Files:**
- Create: `crates/vol-llm-ui/src/connection/mod.rs`
- Create: `crates/vol-llm-ui/src/connection/local.rs`
- Create: `crates/vol-llm-ui/src/connection/remote.rs`

```rust
// vol-llm-ui/src/connection/mod.rs

use crate::state::UiEvent;
use async_trait::async_trait;
use tokio::sync::mpsc;

#[async_trait]
pub trait AgentConnection: Send + Sync {
    /// 提交用户输入，返回事件接收器和请求 ID
    async fn submit(&self, input: String) -> anyhow::Result<mpsc::Receiver<UiEvent>>;

    /// 请求工具审批
    async fn approve_tool(&self, req_id: String, approved: bool, reason: Option<String>) -> anyhow::Result<()>;

    /// 取消当前运行
    async fn cancel(&self, req_id: String) -> anyhow::Result<()>;

    /// 连接是否活跃
    fn is_connected(&self) -> bool;

    /// 文件操作（远程模式下走 JSON-RPC，本地模式走本地 fs）
    async fn list_files(&self, path: &str) -> anyhow::Result<Vec<FileEntry>>;
    async fn read_file(&self, path: &str) -> anyhow::Result<String>;
}

pub struct FileEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
}
```

- [ ] **Step: 创建 `connection/mod.rs`，定义 trait**

**LocalConnection**: 直接创建 ReActAgent，用 `EventObserver` 将 `AgentStreamEvent` 转为 `UiEvent` 通过 mpsc 发送：

```rust
// vol-llm-ui/src/connection/local.rs
pub struct LocalConnection {
    agent_cache: AgentCache,
    // ...
}

#[async_trait]
impl AgentConnection for LocalConnection {
    async fn submit(&self, input: String) -> anyhow::Result<mpsc::Receiver<UiEvent>> {
        let (tx, rx) = mpsc::channel(256);
        // 创建 agent，注册 observer 将事件转为 UiEvent 发送
        // ...
        Ok(rx)
    }
    // ...
}
```

- [ ] **Step: 创建 `connection/local.rs`**

**RemoteConnection**: 通过 JSON-RPC WebSocket 连接：

```rust
// vol-llm-ui/src/connection/remote.rs
pub struct RemoteConnection {
    url: String,
    ws_tx: tokio::sync::mpsc::Sender<JsonRpcRequest>,
    event_rx: tokio::sync::mpsc::Receiver<UiEvent>,
}

#[async_trait]
impl AgentConnection for RemoteConnection {
    async fn submit(&self, input: String) -> anyhow::Result<mpsc::Receiver<UiEvent>> {
        // 通过 WS 发送 JSON-RPC agent.submit
        // 返回 event receiver
    }
    // ...
}
```

- [ ] **Step: 创建 `connection/remote.rs`**

---

## Task 3: Hooks 层

**Files:**
- Create: `crates/vol-llm-ui/src/hooks/mod.rs`
- Create: `crates/vol-llm-ui/src/hooks/use_agent.rs`
- Create: `crates/vol-llm-ui/src/hooks/use_session.rs`
- Create: `crates/vol-llm-ui/src/hooks/use_hitl.rs`

Hooks 是纯异步函数（不绑定特定 UI 框架），接收 `UiState` 和 `AgentConnection`：

```rust
// vol-llm-ui/src/hooks/mod.rs
pub mod use_agent;
pub mod use_session;
pub mod use_hitl;

pub struct UiHooks {
    pub state: Arc<tokio::sync::RwLock<UiState>>,
    pub connection: Arc<dyn AgentConnection>,
    pub event_buffer: Arc<tokio::sync::Mutex<EventBuffer>>,
}
```

- [ ] **Step: 创建 hooks 模块和 `UiHooks` struct**

---

## Task 4: TUI 前端（ratatui）

**Files:**
- Create: `crates/vol-llm-ui/src/tui/bin/tui.rs`
- Create: `crates/vol-llm-ui/src/tui/render.rs`
- Create: `crates/vol-llm-ui/src/tui/input.rs`

从 `vol-llm-tui/src/ui/` 迁移 9 个渲染模块到 `src/tui/render.rs`，适配新的 `UiState`：

- `render_ui` — 主布局
- `render_status_bar`
- `render_tab_bar`
- `render_conversation`
- `render_input_area`
- `render_tools_panel`
- `render_workspace`
- `render_log_viewer`
- `render_skills_panel`
- `render_session_dialog`

键盘处理从 `vol-llm-tui/src/main.rs` 的 `handle_key` 迁移到 `src/tui/input.rs`。

- [ ] **Step: 迁移所有 ratatui 渲染函数，适配 UiState**
- [ ] **Step: 迁移键盘输入处理**
- [ ] **Step: 创建 TUI bin 入口（terminal 初始化 + 事件循环）**

---

## Task 5: Web 前端（Dioxus WASM）

**Files:**
- Create: `crates/vol-llm-ui/src/web/bin/web.rs`
- Create: `crates/vol-llm-ui/src/web/components/mod.rs`
- Create: `crates/vol-llm-ui/src/web/components/app.rs`
- Create: `crates/vol-llm-ui/src/web/components/conversation.rs`
- Create: `crates/vol-llm-ui/src/web/components/tools_panel.rs`
- Create: `crates/vol-llm-ui/src/web/components/workspace.rs`
- Create: `crates/vol-llm-ui/src/web/components/log_viewer.rs`
- Create: `crates/vol-llm-ui/src/web/components/skills_panel.rs`
- Create: `crates/vol-llm-ui/src/web/components/status_bar.rs`
- Create: `crates/vol-llm-ui/src/web/components/input_area.rs`

每个组件用 Dioxus `rsx!` 宏编写，对应 ratatui 的渲染函数：

```rust
// vol-llm-ui/src/web/components/conversation.rs
use dioxus::prelude::*;
use crate::state::{ConversationEntry, UiState};

#[component]
fn ConversationView(state: Signal<UiState>) -> Element {
    rsx! {
        div { class: "conversation",
            for entry in state.read().conversation.iter() {
                {render_entry(entry)}
            }
        }
    }
}
```

- [ ] **Step: 创建所有 Dioxus 组件**
- [ ] **Step: 创建 Web bin 入口（Dioxus launch + WASM 配置）**

---

## Task 6: 远程 JSON-RPC 服务

**Files:**
- Create: `crates/vol-llm-agent-channel/src/jsonrpc/mod.rs`
- Create: `crates/vol-llm-agent-channel/src/jsonrpc/handler.rs`
- Create: `crates/vol-llm-agent-channel/examples/agent-service.rs`
- Modify: `crates/vol-llm-agent-channel/Cargo.toml`

### JSON-RPC 方法定义

```
// 客户端 → 服务端
agent.submit({ input, session_id? }) → { req_id }
agent.cancel({ req_id }) → { ok }
agent.approve({ req_id, approved, reason? }) → { ok }

file.list({ path }) → { entries: [{ name, is_dir, size }] }
file.read({ path }) → { content, encoding: "utf8" }

log.list() → { runs: [{ id, timestamp, count }] }
log.read({ run_id }) → { entries }

session.list() → { sessions: [{ id, entry_count, created_at }] }
session.resume({ session_id }) → { ok }
```

### 服务端实现

```rust
// vol-llm-agent-channel/src/jsonrpc/handler.rs

pub struct JsonRpcHandler {
    router: AgentRouter,
    holders: Arc<RwLock<HashMap<String, Arc<ConnectionHolder>>>>,
    // 文件/日志/会话服务
}

impl JsonRpcHandler {
    pub async fn handle_request(&self, method: &str, params: Value) -> Result<Value, Error> {
        match method {
            "agent.submit" => self.agent_submit(params).await,
            "agent.cancel" => self.agent_cancel(params).await,
            "agent.approve" => self.agent_approve(params).await,
            "file.list" => self.file_list(params).await,
            "file.read" => self.file_read(params).await,
            "log.list" => self.log_list(params).await,
            "log.read" => self.log_read(params).await,
            "session.list" => self.session_list(params).await,
            "session.resume" => self.session_resume(params).await,
            _ => Err(Error::method_not_found()),
        }
    }
}
```

### 事件推送

服务端通过 WebSocket 推送 `UiEvent` JSON-RPC notifications（`method: "ui.event"`）：

```json
{
  "jsonrpc": "2.0",
  "method": "ui.event",
  "params": { "req_id": "...", "type": "thinking_delta", "delta": "..." }
}
```

- [ ] **Step: 添加 jsonrpsee 0.26 依赖到 Cargo.toml**
- [ ] **Step: 创建 jsonrpc/mod.rs + handler.rs**
- [ ] **Step: 创建 agent-service.rs 示例**

---

## Task 7: Cargo.toml 和 workspace 配置

**Files:**
- Modify: `Cargo.toml` (workspace root)
- Create: `crates/vol-llm-ui/Cargo.toml`

```toml
# crates/vol-llm-ui/Cargo.toml
[package]
name = "vol-llm-ui"
version.workspace = true
edition.workspace = true

[lib]
path = "src/lib.rs"

[[bin]]
name = "vol-llm-tui"
path = "src/tui/bin/tui.rs"

[[bin]]
name = "vol-llm-ui-web"
path = "src/web/bin/web.rs"

[dependencies]
vol-llm-agent = { path = "../vol-llm-agent" }
vol-llm-agent-channel = { path = "../vol-llm-agent-channel" }
vol-llm-core = { path = "../vol-llm-core" }
vol-llm-provider = { path = "../vol-llm-provider" }
vol-llm-tool = { path = "../vol-llm-tool" }
vol-llm-agents = { path = "../vol-llm-agents" }
vol-llm-skill = { path = "../vol-llm-skill" }
vol-session = { path = "../vol-session" }

tokio = { workspace = true }
async-trait = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tracing = { workspace = true }

ratatui = { version = "0.30", default-features = false, features = ["crossterm_0_29"] }
crossterm = { version = "0.29", features = ["event-stream"] }
dioxus = { version = "0.6", features = ["web"] }
jsonrpsee = { version = "0.26", features = ["client", "wasm-client"] }
anyhow = "1.0"
```

- [ ] **Step: 创建 vol-llm-ui/Cargo.toml**
- [ ] **Step: 添加 vol-llm-ui 到 workspace members**

---

## Task 8: 测试与验证

- [ ] **Step: `cargo check -p vol-llm-ui --all-targets`** — 确保编译通过
- [ ] **Step: `cargo build -p vol-llm-ui --bin vol-llm-tui`** — TUI 构建成功
- [ ] **Step: `cargo build -p vol-llm-ui --bin vol-llm-ui-web --target wasm32-unknown-unknown`** — WASM 构建成功
- [ ] **Step: `cargo run --example agent-service -p vol-llm-agent-channel`** — 远程服务可启动
