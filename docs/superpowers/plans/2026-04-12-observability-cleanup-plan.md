# Observability 冗余代码清理实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 删除 observability 包中冗余的 logger.rs 文件，并更新所有依赖。

**Architecture:** 完全删除 `observability/logger.rs`，从 `mod.rs` 和 `lib.rs` 中移除相关导出，只保留 `run_log` 子包作为唯一实现来源。

**Tech Stack:** Rust, cargo

---

## File Structure

| File | 职责 | 变更类型 |
|------|------|----------|
| `crates/vol-llm-agent/src/observability/logger.rs` | 冗余代码 | 删除 |
| `crates/vol-llm-agent/src/observability/mod.rs` | 模块导出 | 修改 |
| `crates/vol-llm-agent/src/lib.rs` | 公开 API 导出 | 修改 |
| `docs/ai-agent/06-observability-plugin.md` | 文档 | 可能需要更新 |

---

### Task 1: 删除 logger.rs 文件

**Files:**
- Delete: `crates/vol-llm-agent/src/observability/logger.rs`

- [ ] **Step 1: 确认 logger.rs 内容**

读取文件确认：
```bash
wc -l crates/vol-llm-agent/src/observability/logger.rs
```
Expected: 约 153 行

- [ ] **Step 2: 删除 logger.rs 文件**

```bash
rm crates/vol-llm-agent/src/observability/logger.rs
```

- [ ] **Step 3: 验证文件已删除**

```bash
ls crates/vol-llm-agent/src/observability/
```
Expected: 不再显示 logger.rs

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent/src/observability/logger.rs
git commit -m "refactor: delete redundant logger.rs file"
```

---

### Task 2: 更新 observability/mod.rs

**Files:**
- Modify: `crates/vol-llm-agent/src/observability/mod.rs`

- [ ] **Step 1: 读取当前 mod.rs 内容**

当前内容：
```rust
//! Observability plugin for structured logging and log retention.

pub mod cleanup;
pub mod logger;
pub mod plugin;
pub mod run_log;

pub use cleanup::{cleanup_old_logs, cleanup_run_logs, cleanup_session_logs};
pub use logger::{LogEntry, ObservabilityLogger};
pub use plugin::ObservabilityPlugin;
pub use run_log::{LogEntry as RunLogEntry, RunLogLogger};
```

- [ ] **Step 2: 更新 mod.rs**

替换为：
```rust
//! Observability plugin for structured logging and log retention.

pub mod cleanup;
pub mod plugin;
pub mod run_log;

// Re-export from run_log sub-package
pub use run_log::{LogEntry, RunLogLogger};
pub use cleanup::{cleanup_old_logs, cleanup_run_logs, cleanup_session_logs};
pub use plugin::ObservabilityPlugin;
```

关键变更：
- 移除 `pub mod logger;`
- 移除 `use logger::{...}`
- 从 `run_log` 直接导出 `LogEntry`（不再使用别名）
- 保留 `RunLogLogger`

- [ ] **Step 3: 验证编译**

```bash
cargo check -p vol-llm-agent
```
Expected: 编译成功，无错误

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent/src/observability/mod.rs
git commit -m "refactor: update observability mod.rs to remove logger module"
```

---

### Task 3: 更新 vol-llm-agent/src/lib.rs

**Files:**
- Modify: `crates/vol-llm-agent/src/lib.rs`

- [ ] **Step 1: 读取当前 lib.rs 导出**

当前内容（行 14）：
```rust
pub use observability::{LogEntry, ObservabilityLogger, ObservabilityPlugin, RunLogLogger};
```

- [ ] **Step 2: 更新 lib.rs 导出**

替换为：
```rust
pub use observability::{LogEntry, ObservabilityPlugin, RunLogLogger};
```

移除 `ObservabilityLogger` 导出。

- [ ] **Step 3: 验证编译**

```bash
cargo check -p vol-llm-agent
```
Expected: 编译成功

- [ ] **Step 4: 运行测试**

```bash
cargo test -p vol-llm-agent --lib observability
```
Expected: 所有 observability 测试通过

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-agent/src/lib.rs
git commit -m "refactor: remove ObservabilityLogger from public API exports"
```

---

### Task 4: 更新文档（可选）

**Files:**
- Modify: `docs/ai-agent/06-observability-plugin.md`

- [ ] **Step 1: 检查文档中 ObservabilityLogger 的使用**

```bash
grep -n "ObservabilityLogger" docs/ai-agent/06-observability-plugin.md
```

- [ ] **Step 2: 更新文档中的代码示例**

将文档中所有 `ObservabilityLogger` 替换为 `RunLogLogger`：

示例变更（行 193-251）：
```rust
// 原来:
### 6.1 ObservabilityLogger

pub struct ObservabilityLogger {
    agent_id: String,
    agent_path: PathBuf,
}

impl ObservabilityLogger {
    pub fn new(agent_id: String, log_base_path: PathBuf) -> Self { ... }
}

// 改为:
### 6.1 RunLogLogger

pub struct RunLogLogger {
    agent_id: String,
    agent_path: PathBuf,
}

impl RunLogLogger {
    pub fn new(agent_id: String, log_base_path: PathBuf) -> Self { ... }
}
```

- [ ] **Step 3: 更新 diagram**

更新行 34 的架构图：
```
│   RunLogLogger                   Cleanup Functions           │
```

- [ ] **Step 4: Commit**

```bash
git add docs/ai-agent/06-observability-plugin.md
git commit -m "docs: update observability documentation to use RunLogLogger"
```

---

### Task 5: 最终验证和清理

**Files:**
- Test: Full test suite

- [ ] **Step 1: 运行完整测试套件**

```bash
cargo test -p vol-llm-agent
```
Expected: 所有 100+ 测试通过

- [ ] **Step 2: 检查 dead_code 警告**

```bash
cargo clippy -p vol-llm-agent -- -W dead_code 2>&1 | grep -E "(warning|error)" | head -20
```
Expected: 无 dead_code 警告

- [ ] **Step 3: 验证 git status**

```bash
git status
```
Expected: 显示删除的文件和修改的文件

- [ ] **Step 4: 查看变更统计**

```bash
git diff --stat HEAD~5
```
Expected: 显示删除的代码行数

---

## Self-Review Checklist

**1. Spec coverage:**
- ✅ 删除 logger.rs 文件 (Task 1)
- ✅ 更新 mod.rs (Task 2)
- ✅ 更新 lib.rs (Task 3)
- ✅ 更新文档 (Task 4)
- ✅ 最终验证 (Task 5)

**2. Placeholder scan:**
- ✅ 无 TBD/TODO
- ✅ 所有代码步骤都有具体代码
- ✅ 所有命令都有预期输出

**3. Type consistency:**
- ✅ `LogEntry` 从 `run_log` 导出
- ✅ `RunLogLogger` 保留
- ✅ `ObservabilityLogger` 完全移除

---

Plan complete and saved to `docs/superpowers/plans/2026-04-12-observability-cleanup-plan.md`. Two execution options:

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

**Which approach?**
