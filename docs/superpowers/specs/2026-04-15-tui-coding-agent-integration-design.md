# TUI 集成 CodingAgent 设计文档

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 TUI 从直接操作 ReActAgent 改为委托 CodingAgent，保持 TUI 专注 REPL/事件渲染/HITL，CodingAgent 处理 Agent 内部逻辑。

**Architecture:** TUI 在启动时创建单一 CodingAgent 实例，注册 TuiEventObserver 做实时事件渲染。Session 持久化跨 run 使用。`/unsafe` 命令切换 unsafe_mode。

**Tech Stack:** vol-llm-tui, vol-llm-agents (CodingAgent, ChannelledEventObserver), vol-llm-agent (AgentStreamEvent)

---

## 决策记录

1. **Scope**: TUI 只集成 CodingAgent，不包含 AdviceAgent/QaAgent
2. **Session 模式**: 持久化 Session 跨 REPL 输入，Agent 保留对话上下文
3. **Session 实现**: CodingAgentConfig 添加 `session: Option<Arc<Session>>`，TUI 创建一次传入
4. **Working Dir**: TUI 自动识别当前运行目录作为 work_dir
5. **事件渲染**: 渲染全部 18 种事件，不做过滤
6. **HITL**: 默认开启，通过 `/unsafe` 切换 unsafe_mode

---

## 修改清单

### 1. `vol-llm-agents` — CodingAgentConfig

**File:** `crates/vol-llm-agents/src/coding/config.rs`

添加字段：
```rust
/// Persistent session to reuse across runs.
/// If Some, CodingAgent.run() reuses this session instead of creating a new one per run.
pub session: Option<Arc<vol_session::Session>>,
```

Default 设为 None（保持向后兼容）。Debug impl 添加 `"<Session>"` 占位。

### 2. `vol-llm-agents` — CodingAgentBuilder

**File:** `crates/vol-llm-agents/src/coding/agent.rs`

添加 builder 方法：
```rust
pub fn session(mut self, session: Arc<vol_session::Session>) -> Self {
    self.config.session = Some(session);
    self
}
```

### 3. `vol-llm-agents` — CodingAgent.run()

**File:** `crates/vol-llm-agents/src/coding/agent.rs`

修改 `run()` 方法中的 Session 创建逻辑：

```rust
// 原来：
let session = Arc::new(Session::new(
    format!("coding_{}", chrono::Utc::now().format("%Y%m%d_%H%M%S")),
    Arc::new(InMemorySessionStore::new()),
    Arc::new(InMemoryMessageStore::new()),
));

// 改为：
let session = self.config.session.clone()
    .unwrap_or_else(|| Arc::new(Session::new(
        format!("coding_{}", chrono::Utc::now().format("%Y%m%d_%H%M%S")),
        Arc::new(InMemorySessionStore::new()),
        Arc::new(InMemoryMessageStore::new()),
    )));
```

### 4. `vol-llm-agents` — 导出 Session 类型

**File:** `crates/vol-llm-agents/src/lib.rs`

添加 re-export（方便 TUI 使用）：
```rust
pub use vol_session::Session;
```

### 5. `vol-llm-tui` — TuiEventObserver（新建）

**File:** `crates/vol-llm-tui/src/observer.rs`

```rust
//! TUI event observer — renders AgentStreamEvent to colored terminal output.

use async_trait::async_trait;
use vol_llm_agents::coding::observer::EventObserver;
use vol_llm_agents::coding::error::ObserverError;
use vol_llm_core::AgentStreamEvent;
use crate::render;

pub struct TuiEventObserver;

#[async_trait::async_trait]
impl EventObserver for TuiEventObserver {
    async fn on_event(&self, event: &AgentStreamEvent) -> Result<(), ObserverError> {
        render::render_event(event);
        Ok(())
    }

    async fn on_complete(&self) -> Result<(), ObserverError> {
        println!();
        Ok(())
    }
}
```

### 6. `vol-llm-tui` — main.rs 重写

核心变化：
- 删除直接创建 ReActAgent 的代码
- 用 CodingAgentBuilder 构建 CodingAgent
- 注册 TuiEventObserver
- 创建持久 Session 传入 config
- working_dir = 当前目录
- `/unsafe` 切换模式

伪代码流程：
```rust
// 启动时
let session = Arc::new(Session::new(
    format!("tui_{}", uuid::Uuid::new_v4().simple()),
    Arc::new(InMemorySessionStore::new()),
    Arc::new(InMemoryMessageStore::new()),
));

let observer = Arc::new(TuiEventObserver);

let agent = CodingAgentBuilder::new()
    .llm(llm)
    .working_dir(PathBuf::from("."))
    .session(session)
    .hitl_enabled(true)
    .unsafe_mode(false)  // 默认开启 HITL
    .build()
    .await?;

// REPL 循环中
match agent.run(input).await {
    Ok(response) => { /* 显示 summary */ }
    Err(e) => { /* 显示错误 */ }
}
```

### 7. `vol-llm-tui` — Cargo.toml

添加依赖：
```toml
vol-llm-agents = { path = "../vol-llm-agents" }
vol-session = { path = "../vol-session" }
```

（已存在，无需修改）

### 8. `vol-llm-tui` — 添加 thiserror

Cargo.toml 添加：
```toml
thiserror = { workspace = true }
```

### 9. `vol-llm-tui` — render.rs（无需修改）

现有 `render_event()` 已处理全部 18 种事件变体。

---

## 数据流

```
用户输入 → CodingAgent.run(input)
    → ReActAgent 通过 broadcast channel 发射事件
    → PluginStream → ObserverPlugin → TuiEventObserver
    → on_event() → render::render_event() → stdout 彩色输出
    → AgentComplete → 返回 response
TUI 显示 response → 下一个提示符
```

## HITL 流程

```
Agent 检测到危险 bash 命令
    → RunContext.request_tool_approval()
    → Approval channel 发送请求
    → TUI 提示: Approve? [y/n] >
    → 用户输入 → ApprovalResponse
    → 批准 → 工具执行
    → 拒绝 → 工具跳过，Agent 继续
```

`/unsafe` 切换：unsafe_mode=true → 所有审批自动通过。

---

## 测试策略

1. **编译验证**: `cargo check -p vol-llm-agents -p vol-llm-tui`
2. **现有测试不变**: CodingAgentConfig 新增字段有 default，不影响现有测试
3. **TUI 手动测试**: `cargo run -p vol-llm-tui`
   - 简单问题 → 彩色事件流式输出
   - 危险命令 → 出现 y/n 提示
   - `/unsafe` → 无提示直接执行
   - 多次输入 → Session 保持上下文
   - `/quit` → 优雅退出
