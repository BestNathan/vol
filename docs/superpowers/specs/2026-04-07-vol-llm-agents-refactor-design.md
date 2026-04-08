# vol-llm-agents 重构设计

**日期**: 2026-04-07  
**作者**: Claude (with user collaboration)  
**状态**: Approved

## Overview

将 `vol-llm-bridge` 重命名为 `vol-llm-agents`，定位为业务智能体（Agent）的集合包。当前包含 `AdviceAgent`，未来可扩展更多场景的智能体。

## Goals

1. 重命名包：`vol-llm-bridge` → `vol-llm-agents`
2. 重命名核心类型：
   - `AgentAdviceService` → `AdviceAgent`
   - `AgentAdviceConfig` → `AdviceAgentConfig`
3. 目录结构调整：所有当前代码移入 `src/advice/` 子目录
4. 保持现有逻辑不变，不提前抽象复用代码

## Non-Goals

- 不抽取通用代码到上层目录（有复用需求时再抽）
- 不修改现有业务逻辑
- 不添加新的智能体（未来按需扩展）

---

## Architecture

### 目录结构变更

**变更前**:
```
crates/vol-llm-bridge/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── service.rs      # AgentAdviceService
    ├── limiter.rs      # FrequencyLimiter
    └── prompt.rs       # Prompt 构建
```

**变更后**:
```
crates/vol-llm-agents/
├── Cargo.toml
└── src/
    ├── lib.rs
    └── advice/
        ├── mod.rs      # 导出 AdviceAgent
        ├── service.rs  # AdviceAgent (重命名)
        ├── limiter.rs  # FrequencyLimiter (保持原名)
        └── prompt.rs   # Prompt 构建 (保持原名)
```

### 类型重命名

| 原类型名 | 新类型名 | 说明 |
|----------|----------|------|
| `AgentAdviceService` | `AdviceAgent` | 更简洁，符合 Agent 定位 |
| `AgentAdviceConfig` | `AdviceAgentConfig` | 保持一致 |
| `FrequencyLimiter` | `FrequencyLimiter` | 保持不变（可能复用） |

### 导出变更

**变更前** (`lib.rs`):
```rust
pub use limiter::FrequencyLimiter;
pub use service::{AgentAdviceService, AgentAdviceConfig};
pub use prompt::system_prompt;
```

**变更后** (`lib.rs`):
```rust
pub mod advice;

pub use advice::{AdviceAgent, AdviceAgentConfig};
pub use advice::limiter::FrequencyLimiter;
pub use advice::prompt::system_prompt;
```

**向后兼容**（可选）:
```rust
// 可选：为过渡期提供别名
#[deprecated(since = "0.2.0", note = "Use AdviceAgent instead")]
pub type AgentAdviceService = AdviceAgent;

#[deprecated(since = "0.2.0", note = "Use AdviceAgentConfig instead")]
pub type AgentAdviceConfig = AdviceAgentConfig;
```

---

## Implementation Details

### 文件移动

1. **移动文件到 advice/ 目录**
   - `src/service.rs` → `src/advice/service.rs`
   - `src/limiter.rs` → `src/advice/limiter.rs`
   - `src/prompt.rs` → `src/advice/prompt.rs`

2. **创建 `src/advice/mod.rs`**
   ```rust
   mod service;
   mod limiter;
   mod prompt;

   pub use service::{AdviceAgent, AdviceAgentConfig};
   pub use limiter::FrequencyLimiter;
   pub use prompt::system_prompt;
   ```

3. **更新 `src/lib.rs`**
   ```rust
   //! vol-llm-agents: Business Agents for LLM-powered analysis.

   pub mod advice;

   pub use advice::{AdviceAgent, AdviceAgentConfig};
   ```

4. **更新 `Cargo.toml`**
   ```toml
   [package]
   name = "vol-llm-agents"  # 原：vol-llm-bridge
   ```

### 代码内重命名

**`src/advice/service.rs`**:
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

### 依赖更新

**`vol-monitor/Cargo.toml`**:
```toml
# 原
vol-llm-bridge = { workspace = true }

# 新
vol-llm-agents = { workspace = true }
```

**`vol-monitor/src/main.rs`**:
```rust
// 原
use vol_llm_bridge::{AgentAdviceService, AgentAdviceConfig};

// 新
use vol_llm_agents::{AdviceAgent, AdviceAgentConfig};
```

**`vol-engine/src/engine.rs`**:
```rust
// 类似更新 import 路径
```

---

## Testing Strategy

**单元测试**:
- `AdviceAgent` 的 `test_agent_advice_service_creation` 重命名为 `test_advice_agent_creation`
- 验证重命名后类型正确创建

**集成测试**:
- 现有测试应全部通过（只改类型名，不改逻辑）
- `vol-monitor` 启动测试验证 Agent 正常工作

---

## Backward Compatibility

**破坏性变更**:
- 类型名变更：`AgentAdviceService` → `AdviceAgent`
- Crate 名变更：`vol-llm-bridge` → `vol-llm-agents`

**迁移指南**:
1. 更新 `Cargo.toml` 依赖
2. 更新 `use` 语句
3. 替换类型名（全局搜索替换）

**可选过渡**:
如需向后兼容，可在 `lib.rs` 添加类型别名（标记 `#[deprecated]`）：
```rust
#[deprecated(since = "0.2.0")]
pub type AgentAdviceService = AdviceAgent;
```

---

## Future Work

**扩展新 Agent**:
```rust
// 未来添加
pub mod risk_analysis;   // 风险分析 Agent
pub mod reporting;       // 报告生成 Agent
pub mod anomaly_detect;  // 异常检测 Agent
```

**通用能力抽取**:
- 当多个 Agent 需要 `FrequencyLimiter` 时，移到 `src/limiter.rs`
- 当多个 Agent 需要相似 Prompt 时，移到 `src/prompt/` 并泛化

---

## Appendix: Files to Modify

| 文件 | 变更类型 | 说明 |
|------|---------|------|
| `crates/vol-llm-bridge/Cargo.toml` | 重命名 | → `crates/vol-llm-agents/Cargo.toml` |
| `crates/vol-llm-bridge/src/*.rs` | 移动 | → `crates/vol-llm-agents/src/advice/` |
| `crates/vol-llm-agents/src/lib.rs` | 修改 | 更新导出 |
| `crates/vol-monitor/Cargo.toml` | 修改 | 依赖名变更 |
| `crates/vol-monitor/src/main.rs` | 修改 | import 更新 |
| `crates/vol-engine/src/engine.rs` | 修改 | import 更新 |
| `Cargo.toml` (workspace) | 修改 | members 和依赖更新 |
