---
type: concept
category: framework
tags: [agent-tool, subagent, dispatch, depth-guard, session]
created: 2026-08-20
updated: 2026-08-20
source_count: 1
---

# AgentTool Sub-agent Dispatch

**Category:** 子 agent 派发框架机制
**Related:** [[vol-llm-agent-tool-crate]], [[arc-new-cyclic-registration]], [[react-pattern]], [[tool-registry]], [[tool-context]], [[runtime-session-store-configuration]], [[agenttool-builtin-impl]]

## Definition

`agent` 内置工具让 agent 自主决定把子任务派发给 `.agents/agents/` 中已定义的另一个 agent（或自己的定义）协同处理：按唯一 `AgentDef.id` 派发，子 agent 跑完整 ReAct 循环后同步返回最终结果，会话按 name 持久化可被其他 agent / UI 观测。

## Key Points
- **派发 = 等价于数据面提交任务**：走共享 SessionManager / 工具注册表，决策方从「人」变为「agent 自己」
- **派发链元数据在 AgentDef 上**：`parent_agent: Option<String>`、`depth: u32`（根 = 0，每次派发 +1）；经 `ToolContext.agent_def`（ReAct 循环执行工具时自动填充）随链传递，无需额外通道
- **深度守卫是唯一嵌套控制**：execute 时读调用方 `tool_config.agent.max_depth`（缺省 1 = 只允许派发一层：根可派发，depth≥1 拒绝）；不另设开关、不新建全局配置文件
- **会话按 name 键持久化**：`entry_store_for_agent(&sub_def.name)`，与 `register_agent` 及 `session.list` 查询一致——按 id 存将查不到
- **循环/自派发允许**：每次派发等价新开 session，受 depth 限制
- **上下文贡献**：`AgentInjector` 照 `SkillInjector` 模式把可用 agent 列表（id/name/description）注入上下文，提示可用 `agent` 工具派发

## How It Works

1. 模型调用 `agent { id, prompt, description }`；execute 读 `context.agent_def`（调用方定义，无则按根处理）
2. 深度守卫：`调用方 depth >= max_depth` → `ExecutionFailed`
3. `loader.get_by_id(id)` 精确查找（含 scope 前缀）；未命中 → 报错并列出可用 agent
4. `sub_def = def.clone()`；写 `parent_agent` / `depth+1`（tool_config 随 clone 保留，子 agent 再派发读自己的配置）
5. `Session::new(session_manager.entry_store_for_agent(&sub_def.name))`
6. 构建 `AgentConfig` → `ReActAgent::new(...).run(&prompt)` → 返回最终内容

## Related Concepts
- [[arc-new-cyclic-registration]]: parent_tools 的 Weak 注册方式（本工具依赖它）
- [[tool-context]]: `agent_def` 字段承载调用方身份
- [[react-pattern]]: 子 agent 的执行引擎
