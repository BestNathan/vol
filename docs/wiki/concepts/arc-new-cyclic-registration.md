---
type: concept
category: pattern
tags: [rust, arc, weak, registration, self-reference]
created: 2026-08-20
updated: 2026-08-20
source_count: 1
---

# Arc::new_cyclic 自引用注册

**Category:** Rust 模式
**Related:** [[agenttool-subagent-dispatch]], [[vol-llm-agent-tool-crate]], [[agenttool-builtin-impl]]

## Definition

当一个工具（如 `AgentTool`）需要在构造时持有指向**它即将被注册进去的同一个** `Arc<ToolRegistry>` 的 `Weak`，且注册接口是 `&mut self`（`ToolRegistry::register`）时，唯一正确解法是 `Arc::new_cyclic`：Weak 在分配创建时就指向最终被持有的那个分配。

## Key Points
- **`Arc::get_mut` 不行**：std 要求 `strong_count == 1 && weak_count == 1`。先 `Arc::downgrade` 出 Weak 再注册，weak_count 变 2，`get_mut` 恒 None。
- **`Arc::try_unwrap` 更糟**：它在 strong==1 时成功，但会把值搬出并**立即释放原分配**——在此之前创建的 Weak 变成死引用（`upgrade()` 恒 None；分配被复用后读取他对象头是 UB）。它容忍弱引用的意思是「不拒绝」，不是「保持弱引用有效」。
- **正确写法**（AgentTool 内置化实际采用的形态）：
```rust
let tool_registry = Arc::new_cyclic(|registry_weak| {
    registry.register(AgentTool::new(
        agent_loader.clone(),
        agent_tool_llm,
        session_manager.clone(),
        registry_weak.clone(),
    ));
    registry
});
```
async 注册步骤（MCP 等）必须在 `new_cyclic` 之前完成（闭包是同步的）；闭包内只做同步注册并返回注册表本体。
- **测试必须走活派发**：只断言 `tool_names` 包含工具名抓不住死 Weak（查找失败发生在 upgrade 之前）；要用真实 id 派发并断言结果不是 "tool registry unavailable"（回归测试 `agent_tool_dispatch_parent_tools_stay_alive` 证明还原 try_unwrap 即失败）。

## How It Works

`Arc::new_cyclic` 先创建分配，再把指向该分配的 `&Weak<T>` 交给闭包构造值。闭包返回的注册表与运行时最终持有的 Arc 是同一分配，因此工具内的 Weak 只要运行时存活就能 `upgrade()` 成功。

## Related Concepts
- [[agenttool-subagent-dispatch]]: 依赖本模式注入 parent_tools
