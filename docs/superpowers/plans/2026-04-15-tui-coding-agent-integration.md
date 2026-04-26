# TUI 集成 CodingAgent 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** TUI 从直接操作 ReActAgent 改为委托 CodingAgent，支持持久化 Session 跨 run 使用，/unsafe 切换 HITL 模式。

**Architecture:** TUI 启动时创建单一 CodingAgent 实例，通过 TuiEventObserver 渲染事件到彩色终端。CodingAgentConfig 新增 `session: Option<Arc<Session>>` 支持会话复用。

**Tech Stack:** vol-llm-tui, vol-llm-agents (CodingAgent, CodingAgentBuilder, EventObserver), vol-session

---

### Task 1: 添加 session 字段到 CodingAgentConfig

**Files:**
- Modify: `crates/vol-llm-agents/src/coding/config.rs:1-80`

- [ ] **Step 1: 添加 vol_session 依赖到 vol-llm-agents/Cargo.toml**

Run: `grep vol-session crates/vol-llm-agents/Cargo.toml` — 如果已存在则跳过。

否则添加：
```toml
vol-session = { path = "../vol-session" }
```

- [ ] **Step 2: 添加 session 字段到 CodingAgentConfig**

在 `config.rs` 的结构体定义中（`html_report_path` 字段之后）添加：

```rust
/// Persistent session to reuse across runs.
/// If Some, CodingAgent.run() uses this session instead of creating a new one per run.
pub session: Option<Arc<vol_session::Session>>,
```

- [ ] **Step 3: 添加 session 到 Default impl**

在 `Default` impl 中添加：

```rust
            session: None,
```

- [ ] **Step 4: 添加 session 到 Debug impl**

在 `Debug` impl 的 `finish()` 调用前添加：

```rust
            .field("session", &"<Session>")
```

- [ ] **Step 5: 验证编译**

Run: `cargo check -p vol-llm-agents`
Expected: Compiles successfully

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-agents/Cargo.toml crates/vol-llm-agents/src/coding/config.rs
git commit -m "feat: add session field to CodingAgentConfig for persistent sessions"
```

---

### Task 2: 添加 session builder 方法和使用 session

**Files:**
- Modify: `crates/vol-llm-agents/src/coding/agent.rs:1-290`

- [ ] **Step 1: 添加 session builder 方法**

在 `CodingAgentBuilder` impl 中（`unsafe_mode` 方法之后，`build` 方法之前）添加：

```rust
    /// Set a persistent session for this agent.
    /// If set, all run() calls will reuse this session.
    pub fn session(mut self, session: Arc<vol_session::Session>) -> Self {
        self.config.session = Some(session);
        self
    }
```

- [ ] **Step 2: 修改 run() 使用 config.session**

在 `run()` 方法中，替换 Session 创建部分（约第 153-159 行）：

```rust
        // Use persistent session if configured, otherwise create a new one per run
        use vol_llm_agent::session::{InMemorySessionStore, InMemoryMessageStore};
        let session = self.config.session.clone()
            .unwrap_or_else(|| Arc::new(Session::new(
                format!("coding_{}", chrono::Utc::now().format("%Y%m%d_%H%M%S")),
                Arc::new(InMemorySessionStore::new()),
                Arc::new(InMemoryMessageStore::new()),
            )));
```

- [ ] **Step 3: 验证编译**

Run: `cargo check -p vol-llm-agents`
Expected: Compiles successfully

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agents/src/coding/agent.rs
git commit -m "feat: CodingAgent supports persistent session via config"
```

---

### Task 3: 导出 Session 类型

**Files:**
- Modify: `crates/vol-llm-agents/src/lib.rs:1-13`
- Modify: `crates/vol-llm-agents/src/coding/mod.rs:1-25`

- [ ] **Step 1: 在 coding/mod.rs 中 re-export Session**

在 `coding/mod.rs` 末尾添加：

```rust
// Re-export Session for persistent session configuration
pub use vol_session::Session;
```

- [ ] **Step 2: 在 lib.rs 中 re-export Session**

在 `lib.rs` 的 coding export 行后添加：

```rust
pub use coding::Session;
```

- [ ] **Step 3: 验证编译**

Run: `cargo check -p vol-llm-agents`
Expected: Compiles successfully

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agents/src/lib.rs crates/vol-llm-agents/src/coding/mod.rs
git commit -m "feat: re-export Session type from vol-llm-agents"
```

---

### Task 4: 创建 TuiEventObserver

**Files:**
- Create: `crates/vol-llm-tui/src/observer.rs`

- [ ] **Step 1: 编写 TuiEventObserver 实现**

```rust
//! TUI event observer — renders AgentStreamEvent to colored terminal output.

use async_trait::async_trait;
use vol_llm_agents::coding::observer::EventObserver;
use vol_llm_agents::coding::error::ObserverError;
use vol_llm_core::AgentStreamEvent;
use crate::render;

/// Observer that renders agent events to the terminal using the render module.
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

- [ ] **Step 2: 验证编译**

Run: `cargo check -p vol-llm-tui`
Expected: Compiles successfully (observer module not yet imported in main.rs, which is fine for now)

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-tui/src/observer.rs
git commit -m "feat: add TuiEventObserver for rendering agent events in TUI"
```

---

### Task 5: 重写 TUI main.rs 委托 CodingAgent

**Files:**
- Modify: `crates/vol-llm-tui/src/main.rs:1-176`

- [ ] **Step 1: 更新 imports**

替换 main.rs 的 import 部分：

```rust
//! vol-llm-tui: Interactive CLI for the coding agent.
//!
//! Delegates to CodingAgent for all agent logic.
//! TUI handles: REPL loop, event rendering, HITL approval, session management.

mod observer;
mod render;

use crossterm::{
    style::{Color, Print, ResetColor, SetForegroundColor},
    execute,
};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::Arc;
use vol_llm_agents::coding::{CodingAgentBuilder, EventObserver, Session};
use vol_llm_agents::coding::CodingAgent;
use vol_llm_provider::{AnthropicProvider, LLMConfig};
use vol_session::{InMemoryMessageStore, InMemorySessionStore};

fn print_colored(color: Color, text: &str) {
    let _ = execute!(io::stdout(), SetForegroundColor(color), Print(text), ResetColor);
}

fn print_help() {
    println!();
    println!("Commands:");
    println!("  /quit, /exit  - Exit the TUI");
    println!("  /help         - Show this help message");
    println!("  /clear        - Clear screen");
    println!("  /unsafe       - Toggle unsafe mode (auto-approve all tool calls)");
    println!();
    println!("Type any message to send to the coding agent.");
}
```

- [ ] **Step 2: 重写 main 函数**

替换 main 函数的主体部分：

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for diagnostics
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("vol_llm_tui=info".parse()?)
                .add_directive("vol_llm_agents=info".parse()?)
                .add_directive("vol_llm_agent=info".parse()?)
                .add_directive("vol_llm_provider=info".parse()?),
        )
        .with_target(false)
        .init();

    // Load API key
    let api_key = std::env::var("ANTHROPIC_AUTH_TOKEN")
        .expect("ANTHROPIC_AUTH_TOKEN must be set");

    // Create LLM provider
    let llm_config = LLMConfig::with_literal_key(
        vol_llm_core::LLMProvider::Anthropic,
        "qwen3.5-plus",
        api_key,
        "https://coding.dashscope.aliyuncs.com/apps/anthropic",
    );
    let provider = AnthropicProvider::new(&llm_config)?;
    let llm: Arc<dyn vol_llm_core::LLMClient> = Arc::new(provider);

    // Create persistent session
    let session_id = format!("tui_{}", uuid::Uuid::new_v4().simple());
    let session = Arc::new(Session::new(
        session_id.clone(),
        Arc::new(InMemorySessionStore::new()),
        Arc::new(InMemoryMessageStore::new()),
    ));

    // Create coding agent
    let agent = CodingAgentBuilder::new()
        .llm(llm)
        .working_dir(PathBuf::from("."))
        .session(session)
        .hitl_enabled(true)
        .unsafe_mode(false)
        .max_iterations(50)
        .build()
        .await?;

    // Register event observer for terminal rendering
    let agent = agent.with_observer(Arc::new(observer::TuiEventObserver));

    // Print startup banner
    println!();
    print_colored(Color::Cyan, "=== Coding Agent TUI ===\n");
    println!();
    print_colored(Color::White, &format!("Session: {}\n", session_id));
    print_colored(Color::Yellow, "HITL: enabled (use /unsafe to toggle)\n");
    println!();
    print_help();

    // REPL loop — mutable agent needed for unsafe_mode toggle
    // We use an UnsafeMode wrapper via interior mutability
    let unsafe_mode = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    let stdin = io::stdin();
    loop {
        println!();
        print_colored(Color::Cyan, "> ");
        let _ = io::stdout().flush();

        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {}
            Err(e) => {
                eprintln!("Error reading input: {}", e);
                continue;
            }
        }

        let input = line.trim();

        match input {
            "" => continue,
            "/quit" | "/exit" => {
                print_colored(Color::Yellow, "Goodbye!\n");
                break;
            }
            "/help" => {
                print_help();
                continue;
            }
            "/clear" => {
                print!("\x1B[2J\x1B[1;1H");
                let _ = io::stdout().flush();
                continue;
            }
            "/unsafe" => {
                let current = unsafe_mode.load(std::sync::atomic::Ordering::Relaxed);
                let new_val = !current;
                unsafe_mode.store(new_val, std::sync::atomic::Ordering::Relaxed);
                if new_val {
                    print_colored(Color::Red, "⚠ Unsafe mode ON (auto-approve all tool calls)\n");
                } else {
                    print_colored(Color::Green, "✓ HITL mode ON (prompt for dangerous commands)\n");
                }
                continue;
            }
            _ => {
                // Display user input
                render::render_event(&vol_llm_agent::AgentStreamEvent::agent_start(input.to_string()));

                match agent.run(input).await {
                    Ok(response) => {
                        if !response.summary.is_empty() {
                            println!();
                            print_colored(Color::Green, &format!("{}\n", response.summary));
                        }
                    }
                    Err(e) => {
                        println!();
                        print_colored(Color::Red, &format!("Error: {}\n", e));
                    }
                }
            }
        }
    }

    Ok(())
}
```

Wait — the unsafe_mode toggle needs to actually affect the agent. The agent was already built with `unsafe_mode(false)`. I need a different approach: rebuild the agent when toggling, or use a shared config.

The simplest approach: rebuild the agent on each run with the current unsafe_mode value. Let me refactor.

- [ ] **Step 2 (revised): 重写 main 函数 — 每次 run 时重建 agent 以应用 unsafe_mode**

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for diagnostics
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("vol_llm_tui=info".parse()?)
                .add_directive("vol_llm_agents=info".parse()?)
                .add_directive("vol_llm_agent=info".parse()?)
                .add_directive("vol_llm_provider=info".parse()?),
        )
        .with_target(false)
        .init();

    // Load API key
    let api_key = std::env::var("ANTHROPIC_AUTH_TOKEN")
        .expect("ANTHROPIC_AUTH_TOKEN must be set");

    // Create LLM provider (shared across all agent instances)
    let llm_config = LLMConfig::with_literal_key(
        vol_llm_core::LLMProvider::Anthropic,
        "qwen3.5-plus",
        api_key,
        "https://coding.dashscope.aliyuncs.com/apps/anthropic",
    );
    let provider = AnthropicProvider::new(&llm_config)?;
    let llm: Arc<dyn vol_llm_core::LLMClient> = Arc::new(provider);

    // Create persistent session (shared across all runs)
    let session_id = format!("tui_{}", uuid::Uuid::new_v4().simple());
    let session = Arc::new(Session::new(
        session_id.clone(),
        Arc::new(InMemorySessionStore::new()),
        Arc::new(InMemoryMessageStore::new()),
    ));

    // HITL toggle state
    let mut unsafe_mode = false;

    // Print startup banner
    println!();
    print_colored(Color::Cyan, "=== Coding Agent TUI ===\n");
    println!();
    print_colored(Color::White, &format!("Session: {}\n", session_id));
    print_colored(Color::Yellow, "HITL: enabled (use /unsafe to toggle)\n");
    println!();
    print_help();

    // REPL loop
    let stdin = io::stdin();
    loop {
        println!();
        print_colored(Color::Cyan, "> ");
        let _ = io::stdout().flush();

        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {}
            Err(e) => {
                eprintln!("Error reading input: {}", e);
                continue;
            }
        }

        let input = line.trim();

        match input {
            "" => continue,
            "/quit" | "/exit" => {
                print_colored(Color::Yellow, "Goodbye!\n");
                break;
            }
            "/help" => {
                print_help();
                continue;
            }
            "/clear" => {
                print!("\x1B[2J\x1B[1;1H");
                let _ = io::stdout().flush();
                continue;
            }
            "/unsafe" => {
                unsafe_mode = !unsafe_mode;
                if unsafe_mode {
                    print_colored(Color::Red, "⚠ Unsafe mode ON (auto-approve all tool calls)\n");
                } else {
                    print_colored(Color::Green, "✓ HITL mode ON (prompt for dangerous commands)\n");
                }
                continue;
            }
            _ => {
                // Build agent with current unsafe_mode
                let agent = CodingAgentBuilder::new()
                    .llm(llm.clone())
                    .working_dir(PathBuf::from("."))
                    .session(session.clone())
                    .hitl_enabled(true)
                    .unsafe_mode(unsafe_mode)
                    .max_iterations(50)
                    .build()
                    .await?;

                let agent = agent.with_observer(Arc::new(observer::TuiEventObserver));

                // Display user input
                render::render_event(&vol_llm_agent::AgentStreamEvent::agent_start(input.to_string()));

                match agent.run(input).await {
                    Ok(response) => {
                        if !response.summary.is_empty() {
                            println!();
                            print_colored(Color::Green, &format!("{}\n", response.summary));
                        }
                    }
                    Err(e) => {
                        println!();
                        print_colored(Color::Red, &format!("Error: {}\n", e));
                    }
                }
            }
        }
    }

    Ok(())
}
```

Note: Rebuilding CodingAgent per run is lightweight — it just re-registers tools and creates a config. The persistent session ensures conversation history is maintained across runs.

- [ ] **Step 3: 删除旧的 render 重复函数**

main.rs 中的 `print_colored` 函数已保留（TUI 本地使用）。旧的 `render` module 中的 `print_colored` 也保留（render.rs 内部使用）。两者不冲突。

- [ ] **Step 4: 验证编译**

Run: `cargo check -p vol-llm-tui`
Expected: Compiles successfully

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-tui/src/main.rs crates/vol-llm-tui/src/observer.rs
git commit -m "feat: TUI delegates to CodingAgent with persistent session and /unsafe toggle"
```

---

### Task 6: 验证和清理

- [ ] **Step 1: 完整 workspace 检查**

Run: `cargo check --workspace`
Expected: All crates compile without errors

- [ ] **Step 2: 运行 vol-llm-agents 测试**

Run: `cargo test -p vol-llm-agents --lib 2>&1 | tail -20`
Expected: All tests pass (new session field has default, no test impact)

- [ ] **Step 3: 手动测试 TUI**

Run: `cargo build -p vol-llm-tui`
Expected: Builds successfully

Test scenarios:
1. 输入简单文本 → 看到彩色事件流式输出，最终显示回答
2. 输入含 bash 命令的任务 → 看到 y/n 审批提示
3. 输入 `/unsafe` → 显示 Unsafe mode ON，再输命令不再提示
4. 再输入 `/unsafe` → 显示 HITL mode ON，恢复审批提示
5. 输入 `/help` → 显示帮助信息
6. 输入 `/clear` → 清屏
7. 输入 `/quit` → 优雅退出

- [ ] **Step 4: Commit (if any fixes needed)**

---

## Summary of Changes

| Crate | Files Changed | Purpose |
|-------|---------------|---------|
| `vol-llm-agents` | `config.rs` | Add `session` field to CodingAgentConfig |
| `vol-llm-agents` | `agent.rs` | Add `session()` builder, use config.session in run() |
| `vol-llm-agents` | `lib.rs`, `coding/mod.rs` | Re-export Session type |
| `vol-llm-tui` | `main.rs` | Rewrite: delegate to CodingAgent, persistent session, /unsafe toggle |
| `vol-llm-tui` | `observer.rs` | New: TuiEventObserver implementing EventObserver |
