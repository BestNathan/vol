# ToolContext Simplification Design

**日期**: 2026-04-07  
**作者**: Claude (with user collaboration)  
**状态**: Approved

## Overview

简化 `ToolContext` 结构，移除与 LLM Agent 无关的字段，只保留与对话相关的核心字段。

## Goals

1. 移除 `alert: Option<Alert>` 字段（特定于报警系统，与 Agent 核心无关）
2. 移除 `instrument: String` 字段（工具从 args 获取参数，不依赖 context）
3. 只保留 `messages: Vec<Message>` 用于传递对话历史
4. 移除 `vol-llm-tool` 对 `vol-core` 的依赖

## Non-Goals

- 不添加新的字段或功能
- 不提供向后兼容的旧字段访问方法
- 不修改工具的参数传递方式（仍通过 args JSON）

---

## Architecture

### 变更前结构

```rust
/// Tool execution context
#[derive(Debug, Clone, Default)]
pub struct ToolContext {
    pub alert: Option<Alert>,
    pub instrument: String,
    pub messages: Vec<Message>,
    pub metadata: std::collections::HashMap<String, String>,
}
```

**问题**：
- `alert` 依赖 `vol-core::Alert`，使 `vol-llm-tool` 耦合到特定业务域
- `instrument` 在实际工具实现中未被使用
- `metadata` 暂无实际使用场景

### 变更后结构

```rust
/// Tool execution context
#[derive(Debug, Clone, Default)]
pub struct ToolContext {
    pub messages: Vec<Message>,
}
```

**优点**：
- 简洁，符合 YAGNI 原则
- 移除对 `vol-core` 的依赖
- `messages` 是对话历史，与 Agent 强相关
- 工具参数通过 `args` 传递，不依赖 context

---

## Implementation Details

### 文件修改

| 文件 | 变更类型 | 描述 |
|------|---------|------|
| `crates/vol-llm-tool/src/tool.rs` | 修改 | 简化 `ToolContext` 结构 |
| `crates/vol-llm-tool/Cargo.toml` | 修改 | 移除 `vol-core` 依赖 |
| `crates/vol-llm-bridge/src/service.rs` | 修改 | 更新 context 创建代码 |
| `crates/vol-llm-agent/tests/*` | 修改 | 更新测试中的 context 创建 |

### 变更详情

**1. tool.rs**

```rust
// 变更前
#[derive(Debug, Clone, Default)]
pub struct ToolContext {
    pub alert: Option<Alert>,
    pub instrument: String,
    pub messages: Vec<Message>,
    pub metadata: std::collections::HashMap<String, String>,
}

// 变更后
#[derive(Debug, Clone, Default)]
pub struct ToolContext {
    pub messages: Vec<Message>,
}
```

**2. vol-llm-tool/Cargo.toml**

移除：
```toml
vol-core = { workspace = true }
```

**3. vol-llm-bridge/src/service.rs**

```rust
// 变更前
let context = ToolContext {
    alert: Some(alert.clone()),
    instrument: alert.symbol.clone(),
    messages: Vec::new(),
    metadata: std::collections::HashMap::new(),
};

// 变更后
let context = ToolContext {
    messages: Vec::new(),
};
```

**4. 测试代码**

```rust
// 变更前
let context = ToolContext {
    instrument: "btc_usd".to_string(),
    ..Default::default()
};

// 变更后
let context = ToolContext::default();
```

---

## Error Handling

**潜在问题**：
- 如果有工具通过 `context.instrument` 获取参数，会编译失败
- 解决方案：工具应从 `args` 解析参数，而非依赖 context

**当前工具分析**：
- `vol-llm-tdengine` 的 4 个工具均未使用 `context` 参数（使用 `_context` 忽略）
- 所有工具从 `args` 解析所需参数

---

## Testing Strategy

**单元测试**：
- 验证 `ToolContext::default()` 创建空 messages
- 验证 `ToolContext { messages: vec![...] }` 正常创建

**集成测试**：
- 现有测试更新后应全部通过
- 工具执行测试应正常工作

---

## Backward Compatibility

**破坏性变更**：
- `ToolContext` 不再包含 `alert` 和 `instrument` 字段
- 使用这些字段的代码会编译失败

**迁移指南**：
1. 移除代码中对 `context.alert` 和 `context.instrument` 的引用
2. 工具参数通过 `args` JSON 传递
3. 如需传递额外上下文，使用 `messages` 传递对话历史

---

## Future Work

如果未来需要扩展 context：
- 考虑添加通用的 `data: HashMap<String, Value>` 字段
- 或创建特定领域的 context 扩展类型
- 保持核心 `ToolContext` 简洁

---

## Appendix: Files to Modify

| 文件 | 变更 |
|------|------|
| `crates/vol-llm-tool/src/tool.rs` | 简化 `ToolContext` 结构，移除 `Alert` import |
| `crates/vol-llm-tool/Cargo.toml` | 移除 `vol-core` 依赖 |
| `crates/vol-llm-bridge/src/service.rs` | 更新 context 创建 |
| `crates/vol-llm-agent/tests/*.rs` | 更新测试中的 context 创建 |
