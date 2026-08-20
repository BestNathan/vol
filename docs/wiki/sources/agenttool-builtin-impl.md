---
type: source
source_type: code
date: 2026-08-20
ingested: 2026-08-20
tags: [agent-tool, subagent, dispatch, runtime, builtin-tool]
---

# AgentTool 内置化实现

**Authors/Creators:** Nathan (with Claude, subagent-driven execution)
**Date:** 2026-08-20
**Link:** docs/superpowers/specs/2026-08-20-agenttool-builtin-design.md

## TL;DR

把 `AgentTool` 从 vol-llm-agent 迁入新 crate `vol-llm-agent-tool` 并注册为运行时内置工具：agent 按唯一 `AgentDef.id` 派发 `.agents/agents/` 内置定义子 agent，深度守卫读调用方 `tool_config.agent.max_depth`（默认 1 = 只允许一层派发），子 agent 会话按 name 经 SessionManager 持久化可观测，`AgentInjector` 把可用 agent 列表贡献进上下文提示派发。

## Key Takeaways

- 派发语义：`agent` 工具参数 `id`/`prompt`/`description`；`AgentLoader::get_by_id` 按 `"{scope}:{name}"` 精确查找；子 agent 跑完整 ReAct 循环后同步返回最终结果。
- 派发链元数据落在 `AgentDef`（vol-llm-core）新字段 `parent_agent` / `depth` 上，经 `ToolContext.agent_def`（ReAct 循环执行工具时填充）随链传递；子 agent 定义 = clone + 写 parent/depth + depth+1。
- 深度守卫在 AgentTool execute 内：`调用方 depth >= max_depth → ExecutionFailed`；max_depth 来自调用方定义 `tool_config.agent.max_depth`，缺省 1。
- 子 agent 会话：`Session::new(session_manager.entry_store_for_agent(&def.name))` —— 按 name 键存，与 `register_agent` 及前端 `session.list` 查询一致。
- 运行时接线：`AgentRuntime` 新增共享 `agent_loader`；`build()`/`for_test()` 均注册 `agent` 工具；`register_agent` 挂 `AgentInjector` contributor；`discover_agents` 复用共享 loader。
- 关键实现教训：AgentTool 以 `Weak<ToolRegistry>` 持有父注册表（execute 时 upgrade），注册必须用 `Arc::new_cyclic` —— 见 [[arc-new-cyclic-registration]]（`Arc::get_mut` 要求 weak_count==1、`try_unwrap` 会让先建 Weak 悬空，两者都不行）。

## Detailed Summary

七任务计划（subagent-driven）：① AgentDef 扩展 parent/depth（含四处字面量修复）→ ② `AgentLoader::get_by_id` → ③ 新 crate 骨架 + 机械迁移 → ④ AgentTool 语义重构（TDD 六测试：深度守卫默认/覆盖、id 未命中列出可用、parent/depth 写入、会话按 name 持久化、成功派发返回内容）→ ⑤ AgentInjector（照 SkillInjector 模式，去 filter；contributor 名 `"agents"`，anchor Head(1)，无定义输出空块保持固定槽位）→ ⑥ 运行时接线（四处 `AgentRuntime` 构造点、集成测试含活派发回归 `agent_tool_dispatch_parent_tools_stay_alive`）→ ⑦ 质量门与收尾。

任务⑥首版注册用 `Arc::downgrade → Arc::try_unwrap → re-Arc` 产生**死 Weak**（审查者独立实证 upgrade 恒 None），修复轮改用 `Arc::new_cyclic` 并在测试中证明还原即失败。

覆盖 gate 终值：vol-llm-core 92.79%、vol-llm-agent 85.78%、vol-llm-agent-tool 89.32%、vol-llm-runtime 83.19%（均 ≥80）。

## Entities Mentioned
- [[vol-llm-agent-tool-crate]]: 新建 — AgentTool + AgentInjector 所在高层组合 crate
- [[vol-llm-runtime-crate]]: 注册 agent 内置工具、共享 agent_loader、挂 AgentInjector
- [[vol-llm-agent-crate]]: agent_tool 迁出、AgentLoader 增 get_by_id
- [[vol-llm-core-crate]]: AgentDef 增 parent_agent/depth

## Concepts Covered
- [[agenttool-subagent-dispatch]]: 子 agent 派发语义与守卫
- [[arc-new-cyclic-registration]]: Arc::new_cyclic 自引用 Weak 注册模式

## Notes
- `AgentDef.tools`/`disallowed_tools`/`model` 仍不生效（继承父工具集、用注册时 LLM）——设计 Non-Goals，后续可做。
- 工具调用中直接传 AgentDef（内联定义）暂不支持，后续可做。
- `check-agent-boundaries.sh` 未覆盖 agent-tool→runtime 边界，需人工 `cargo tree` 验证（本次已验）。
