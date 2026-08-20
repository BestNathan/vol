---
type: entity
category: product
tags: [crate, agent-tool, subagent, dispatch, injector]
created: 2026-08-20
updated: 2026-08-20
source_count: 1
---

# vol-llm-agent-tool Crate

## Overview
高层组合 crate：`AgentTool`（内置 `agent` 工具，派发子 agent）与 `AgentInjector`（上下文贡献者，提示可用 agent）。依赖底层实现（vol-llm-agent / vol-session / vol-llm-tool / vol-llm-core / vol-llm-context），被 vol-llm-runtime 依赖并注册为内置工具。

## Key Facts
- `AgentTool`：按唯一 `AgentDef.id`（`"{scope}:{name}"`）派发 `.agents/agents/` 内置定义，跑完整 ReAct 循环，同步返回最终结果；参数 `id`/`prompt`/`description`；Sensitivity `Safe`。
- 构造签名 `AgentTool::new(loader: Arc<AgentLoader>, llm: Arc<dyn LLMClient>, session_manager: Arc<dyn SessionManager>, parent_tools: Weak<ToolRegistry>)`；注册用 `Arc::new_cyclic`（见 [[arc-new-cyclic-registration]]）。
- 深度守卫：调用方 `AgentDef.depth >= 调用方 tool_config.agent.max_depth`（缺省 `DEFAULT_MAX_DEPTH = 1`）→ 拒绝；子 agent 定义写入 `parent_agent`/`depth+1`。
- 会话持久化：`entry_store_for_agent(&def.name)` —— 按 name 键，`session.list` 可查。
- `AgentInjector`：`ContextContributor`（名 `"agents"`，anchor `Head(1)`），照 `SkillInjector` 模式（无 filter）；有定义时输出「`agent` 工具提示 + id/name/description 列表」，无定义输出空块保持固定槽位。
- 测试：6 个 AgentTool 单测 + 2 个 injector 单测；覆盖 89.32%。

## Related
- [[agenttool-subagent-dispatch]]
- [[arc-new-cyclic-registration]]
- [[vol-llm-runtime-crate]]
- [[vol-llm-agent-crate]]
- [[agenttool-builtin-impl]]
