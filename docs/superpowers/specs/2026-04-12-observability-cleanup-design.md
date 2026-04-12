# Observability 冗余代码清理设计文档

**日期:** 2026-04-12  
**状态:** 已批准

---

## 概述

在 run_log 子包重构完成后，observability 包中存在冗余代码：
- `logger.rs` 中的 `LogEntry` 与 `run_log/logger.rs` 中的 `LogEntry` 完全重复
- `ObservabilityLogger` 与 `RunLogLogger` 功能几乎相同

本设计文档描述如何彻底删除这些冗余代码，并更新所有依赖。

---

## 当前状态

### 文件结构
```
observability/
├── run_log/
│   ├── mod.rs          # 导出 LogEntry, RunLogLogger
│   ├── logger.rs       # LogEntry, RunLogLogger 完整实现
│   └── cleanup.rs      # cleanup 函数
├── cleanup.rs          # 重新导出 run_log cleanup
├── logger.rs           # ❌ 冗余：LogEntry, ObservabilityLogger
├── plugin.rs           # ObservabilityPlugin 使用 RunLogLogger
└── mod.rs              # 导出 logger 和 run_log 中的类型
```

### 冗余代码
1. `logger.rs:82-152` - `LogEntry` 结构体（与 run_log/logger.rs 重复）
2. `logger.rs:10-60` - `ObservabilityLogger`（与 `RunLogLogger` 功能相同）
3. `logger.rs:64-79` - `append_to_file` 辅助函数（与 run_log/logger.rs 重复）

---

## 目标架构

### 文件结构
```
observability/
├── run_log/
│   ├── mod.rs          # 导出 LogEntry, RunLogLogger
│   ├── logger.rs       # 完整实现
│   └── cleanup.rs      # cleanup 函数
├── cleanup.rs          # 重新导出 run_log cleanup
├── plugin.rs           # ObservabilityPlugin 使用 RunLogLogger
└── mod.rs              # 只导出 run_log 中的类型
```

### 导出变更
| 类型 | 变更前 | 变更后 |
|------|--------|--------|
| `LogEntry` | `observability::LogEntry` (from logger.rs) | `observability::LogEntry` (re-export from run_log) |
| `ObservabilityLogger` | `observability::ObservabilityLogger` | **删除** |
| `RunLogLogger` | `observability::RunLogLogger` | `observability::RunLogLogger` (保留) |

---

## 实施步骤

### 1. 删除 logger.rs 文件
```bash
rm crates/vol-llm-agent/src/observability/logger.rs
```

### 2. 更新 observability/mod.rs
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

### 3. 更新 vol-llm-agent/src/lib.rs
```rust
// 变更前:
pub use observability::{LogEntry, ObservabilityLogger, ObservabilityPlugin, RunLogLogger};

// 变更后:
pub use observability::{LogEntry, ObservabilityPlugin, RunLogLogger};
```

### 4. 验证和测试
```bash
cargo check -p vol-llm-agent
cargo test -p vol-llm-agent
cargo clippy -p vol-llm-agent -- -W dead_code
```

---

## 验收标准

- [ ] `logger.rs` 文件已删除
- [ ] `mod.rs` 不再声明 `pub mod logger`
- [ ] `lib.rs` 不再导出 `ObservabilityLogger`
- [ ] 编译通过
- [ ] 所有测试通过
- [ ] 无 dead_code 警告
- [ ] 提交代码

---

## 影响范围

### 内部影响
- `ObservabilityPlugin` 已使用 `RunLogLogger`，不受影响
- `ObservabilityLogger` 在代码库中无直接使用者

### 外部影响（API 破坏）
任何直接使用 `ObservabilityLogger` 的外部代码需要迁移到 `RunLogLogger`：
```rust
// 原来:
use vol_llm_agent::ObservabilityLogger;
let logger = ObservabilityLogger::new(agent_id, path);

// 迁移后:
use vol_llm_agent::RunLogLogger;
let logger = RunLogLogger::new(agent_id, path);
```

---

## 后续工作

清理完成后，observability 包结构更加清晰：
- `run_log` 子包：唯一实现来源
- `plugin.rs`：只使用 `RunLogLogger`
- `mod.rs`：简洁的重新导出
