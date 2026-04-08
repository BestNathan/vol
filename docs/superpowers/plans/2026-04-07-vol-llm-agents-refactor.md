# vol-llm-agents 重构实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 vol-llm-bridge 重命名为 vol-llm-agents，并将所有代码移入 src/advice/ 子目录，重命名核心类型为 AdviceAgent。

**Architecture:** 重命名 crate，移动文件到 advice/ 目录，更新所有依赖和 import 路径，保持现有逻辑不变。

**Tech Stack:** Rust, tokio, existing vol-llm-* crates

---

## File Structure

**Files to Create:**
- `crates/vol-llm-agents/src/advice/mod.rs` - advice 模块入口

**Files to Move:**
- `crates/vol-llm-bridge/src/service.rs` → `crates/vol-llm-agents/src/advice/service.rs`
- `crates/vol-llm-bridge/src/limiter.rs` → `crates/vol-llm-agents/src/advice/limiter.rs`
- `crates/vol-llm-bridge/src/prompt.rs` → `crates/vol-llm-agents/src/advice/prompt.rs`

**Files to Modify:**
- `crates/vol-llm-bridge/Cargo.toml` → `crates/vol-llm-agents/Cargo.toml` (重命名)
- `crates/vol-llm-agents/src/lib.rs` - 更新导出
- `crates/vol-llm-agents/src/advice/service.rs` - 重命名类型
- `Cargo.toml` (workspace) - 更新 members 和依赖
- `crates/vol-monitor/Cargo.toml` - 更新依赖
- `crates/vol-monitor/src/main.rs` - 更新 import
- `crates/vol-engine/src/engine.rs` - 更新 import (如有)

---

### Task 1: 创建新目录结构并移动文件

**Files:**
- Create: `crates/vol-llm-agents/src/advice/mod.rs`
- Move: `crates/vol-llm-bridge/` → `crates/vol-llm-agents/`

- [ ] **Step 1: 重命名目录**

Run: `mv crates/vol-llm-bridge crates/vol-llm-agents`

Expected: Directory renamed successfully

- [ ] **Step 2: 创建 advice 子目录**

Run: `mkdir -p crates/vol-llm-agents/src/advice`

- [ ] **Step 3: 移动源文件到 advice 目录**

Run:
```bash
mv crates/vol-llm-agents/src/service.rs crates/vol-llm-agents/src/advice/
mv crates/vol-llm-agents/src/limiter.rs crates/vol-llm-agents/src/advice/
mv crates/vol-llm-agents/src/prompt.rs crates/vol-llm-agents/src/advice/
```

- [ ] **Step 4: 创建 advice/mod.rs**

Create `crates/vol-llm-agents/src/advice/mod.rs`:
```rust
//! Advice Agent: AI-powered alert analysis and advice.

mod service;
mod limiter;
mod prompt;

pub use service::{AdviceAgent, AdviceAgentConfig};
pub use limiter::FrequencyLimiter;
pub use prompt::{system_prompt, build_user_prompt, get_threshold_from_alert};
```

- [ ] **Step 5: 更新 lib.rs**

Modify `crates/vol-llm-agents/src/lib.rs`:
```rust
//! vol-llm-agents: Business Agents for LLM-powered analysis.

pub mod advice;

pub use advice::{AdviceAgent, AdviceAgentConfig};
pub use advice::FrequencyLimiter;
pub use advice::system_prompt;
```

- [ ] **Step 6: 提交**

```bash
git add crates/vol-llm-agents/
git commit -m "refactor: rename vol-llm-bridge to vol-llm-agents and move code to advice/"
```

---

### Task 2: 重命名核心类型

**Files:**
- Modify: `crates/vol-llm-agents/src/advice/service.rs`

- [ ] **Step 1: 重命名 struct 和 impl**

在 `crates/vol-llm-agents/src/advice/service.rs` 中替换：

```rust
// 原
pub struct AgentAdviceService { ... }
impl AgentAdviceService { ... }
impl NotificationHandler for AgentAdviceService { ... }

// 新
pub struct AdviceAgent { ... }
impl AdviceAgent { ... }
impl NotificationHandler for AdviceAgent { ... }
```

Run: `sed -i 's/AgentAdviceService/AdviceAgent/g' crates/vol-llm-agents/src/advice/service.rs`

- [ ] **Step 2: 重命名 Config 类型**

Run: `sed -i 's/AgentAdviceConfig/AdviceAgentConfig/g' crates/vol-llm-agents/src/advice/service.rs`

- [ ] **Step 3: 更新测试中的类型名**

在 `crates/vol-llm-agents/src/advice/service.rs` 测试中：

```rust
// 原
fn test_agent_advice_service_creation() { ... }

// 新
fn test_advice_agent_creation() { ... }
```

Run: `sed -i 's/test_agent_advice_service_creation/test_advice_agent_creation/g' crates/vol-llm-agents/src/advice/service.rs`

- [ ] **Step 4: 验证编译**

Run: `cd crates/vol-llm-agents && cargo check`

Expected: Compiles without errors

- [ ] **Step 5: 提交**

```bash
git add crates/vol-llm-agents/src/advice/service.rs
git commit -m "refactor: rename AgentAdviceService to AdviceAgent"
```

---

### Task 3: 更新 Workspace 配置

**Files:**
- Modify: `Cargo.toml` (root workspace)

- [ ] **Step 1: 更新 workspace members**

在根目录 `Cargo.toml` 中替换：

```toml
# 原
members = [
    ...
    "crates/vol-llm-bridge",
    ...
]

# 新
members = [
    ...
    "crates/vol-llm-agents",
    ...
]
```

- [ ] **Step 2: 更新 workspace dependencies**

```toml
# 原
vol-llm-bridge = { path = "crates/vol-llm-bridge" }

# 新
vol-llm-agents = { path = "crates/vol-llm-agents" }
```

- [ ] **Step 3: 验证 workspace 解析**

Run: `cargo metadata --format-version 1 | jq '.packages[] | select(.name == "vol-llm-agents")'`

Expected: vol-llm-agents package info displayed

- [ ] **Step 4: 提交**

```bash
git add Cargo.toml
git commit -m "chore: update workspace to use vol-llm-agents"
```

---

### Task 4: 更新 vol-monitor 依赖

**Files:**
- Modify: `crates/vol-monitor/Cargo.toml`
- Modify: `crates/vol-monitor/src/main.rs`

- [ ] **Step 1: 更新 Cargo.toml 依赖**

在 `crates/vol-monitor/Cargo.toml` 中替换：

```toml
# 原
vol-llm-bridge = { workspace = true }

# 新
vol-llm-agents = { workspace = true }
```

- [ ] **Step 2: 更新 main.rs import**

在 `crates/vol-monitor/src/main.rs` 第 17 行附近替换：

```rust
// 原
use vol_llm_bridge::{AgentAdviceService, AgentAdviceConfig};

// 新
use vol_llm_agents::{AdviceAgent, AdviceAgentConfig};
```

- [ ] **Step 3: 更新类型使用**

在 `crates/vol-monitor/src/main.rs` 中替换所有出现：

```rust
// 原
AgentAdviceConfig::default()
AgentAdviceService::new(...)

// 新
AdviceAgentConfig::default()
AdviceAgent::new(...)
```

Run: `sed -i 's/AgentAdviceConfig/AdviceAgentConfig/g' crates/vol-monitor/src/main.rs`

Run: `sed -i 's/AgentAdviceService/AdviceAgent/g' crates/vol-monitor/src/main.rs`

- [ ] **Step 4: 验证编译**

Run: `cargo check -p vol-monitor`

Expected: Compiles without errors

- [ ] **Step 5: 提交**

```bash
git add crates/vol-monitor/
git commit -m "refactor: update vol-monitor to use AdviceAgent"
```

---

### Task 5: 更新 vol-engine 依赖（如有）

**Files:**
- Modify: `crates/vol-engine/src/engine.rs` (if exists)

- [ ] **Step 1: 检查 vol-engine 是否使用**

Run: `grep -n "vol_llm_bridge\|AgentAdvice" crates/vol-engine/src/*.rs 2>/dev/null || echo "Not used"`

- [ ] **Step 2: 如果有使用，更新 import**

如果 Step 1 有输出，替换：

```rust
// 原
use vol_llm_bridge::AgentAdviceService;

// 新
use vol_llm_agents::AdviceAgent;
```

- [ ] **Step 3: 验证编译**

Run: `cargo check -p vol-engine`

Expected: Compiles without errors

- [ ] **Step 4: 提交（如有变更）**

```bash
git add crates/vol-engine/
git commit -m "refactor: update vol-engine to use AdviceAgent"
```

---

### Task 6: 验证所有测试通过

**Files:**
- Test: All workspace tests

- [ ] **Step 1: 运行 vol-llm-agents 测试**

Run: `cargo test -p vol-llm-agents -- --nocapture`

Expected output:
```
running 5 tests
test advice::limiter::tests::test_first_analysis_allowed ... ok
test advice::limiter::tests::test_cooldown_blocks_analysis ... ok
test advice::limiter::tests::test_different_symbols_independent ... ok
test advice::limiter::tests::test_hourly_limit ... ok
test advice::prompt::tests::test_system_prompt_not_empty ... ok
test advice::prompt::tests::test_get_threshold_from_alert ... ok

test result: ok. 6 passed; 0 failed; 0 ignored
```

- [ ] **Step 2: 运行完整 workspace 测试**

Run: `cargo test --workspace 2>&1 | grep "test result"`

Expected: All crates show `ok. X passed; 0 failed`

- [ ] **Step 3: 构建 vol-monitor release**

Run: `cargo build -p vol-monitor --release`

Expected: `Finished release profile [optimized]`

- [ ] **Step 4: 提交（如需要）**

如果测试有修复，提交：

```bash
git add .
git commit -m "test: fix tests after AdviceAgent refactoring"
```

---

## Self-Review

**1. Spec Coverage:**
- [x] 重命名 crate: vol-llm-bridge → vol-llm-agents - Task 1
- [x] 移动文件到 advice/ 目录 - Task 1
- [x] 重命名类型：AgentAdviceService → AdviceAgent - Task 2
- [x] 更新 workspace 配置 - Task 3
- [x] 更新 vol-monitor 依赖 - Task 4
- [x] 更新 vol-engine (如有) - Task 5
- [x] 验证所有测试 - Task 6

**2. No Placeholders:**
- 所有步骤包含实际代码和命令
- 文件路径都是精确的
- 预期输出已指定

**3. Type Consistency:**
- `AdviceAgent` 和 `AdviceAgentConfig` 在所有文件中一致使用
- `vol-llm-agents` crate 名称一致

---

Plan complete and saved to `docs/superpowers/plans/2026-04-07-vol-llm-agents-refactor.md`. Two execution options:

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

**Which approach?**
