# Requirements: ReAct Agent Config Unification

## Background

当前 ReAct Agent 的构造方式是分离的：`ReActAgent::new(llm, tools, config, session)` 接收 4 个独立参数，`AgentConfig` 只包含部分运行时配置（max_iterations、plugin_registry 等）。`AgentDef` 的声明式字段（tools、disallowed_tools、max_iterations、model）在构造时未被使用（YAGNI 注释标注）。此外还有 `AgentBuilder` 和 `CodingAgentBuilder` 各自维护构造逻辑，导致配置分散、职责不清。

## Goals

1. **统一配置入口**：`AgentConfig` 包含 agent 所需的一切配置（`AgentDef`、`llm`、`tools`、`plugins`、`session`、`sandbox`、`max_iterations` 等），`ReActAgent::new(config)` 单参数构造
2. **声明式工具过滤**：运行时根据 `AgentDef` 的 `tools`（白名单）和 `disallowed_tools`（黑名单）字段过滤可用工具集，控制 LLM 可使用的工具
3. **AgentConfig 自带 builder**：废弃 `AgentBuilder`，替换为 `AgentConfig::builder()` 链式构建方式
4. **适配下游调用方**：`AgentTool` 和 `CodingAgent` 等使用新 `AgentConfig` 结构

## Non-Goals

1. 不改变 `AgentDef` 的加载和发现机制（`AgentLoader` 保持原样）
2. 不改变 `AgentPlugin` trait 和插件拦截机制
3. 不改变 ReAct Agent 循环逻辑本身
4. 不改变 `CodingAgent` 的业务逻辑，只适配新的 `AgentConfig` 构造方式

## Scope

### Included

- `AgentConfig` 新增字段：
  - `def: Option<AgentDef>` — 声明式 agent 定义
  - `llm: Option<Arc<dyn LLMClient>>` — LLM 客户端
  - `tools: ToolRegistry` — 完整工具注册表
  - `session: Option<Arc<Session>>` — 会话（未提供时 builder 自动创建）
  - `sandbox: Option<SandboxRef>` — 沙箱配置
- `AgentConfig` 原有字段保留（但合并到 config 结构内）：
  - `max_iterations`、`max_history_messages`、`context_builder`、`plugin_registry`、`agent_id`、`working_dir`
- `ReActAgent` struct 改为从 `config` 读取所有字段，移除独立参数
- `AgentConfig::builder()` 提供链式构建方法（`with_llm`、`with_tool`、`with_session` 等）
- `AgentBuilder` 废弃（文件删除或标记 deprecated）
- 运行时工具过滤逻辑：`AgentDef.tools` 和 `AgentDef.disallowed_tools` 在 `run()` 中生效

### Excluded

- `AgentLoader` 的 .md 文件发现机制不变
- `AgentPlugin` trait 不变
- ReAct Agent 循环逻辑不变
- `CodingAgent` 的业务逻辑不变

## Constraints

- `AgentDef` 必须在 `AgentConfig` 中以 `Option<AgentDef>` 存在
- 工具过滤规则：
  - `def.tools` 为空（None）→ 不过滤，全部工具可用
  - `def.tools` 有值 → 白名单模式，仅允许这些工具
  - `def.disallowed_tools` 有值 → 从可用工具中排除这些
  - 两者同时存在 → 先白名单，再黑名单（交集后排除）
- 所有测试必须通过

## Success Criteria

1. **单参数构造**：`ReActAgent::new(config)` 只需一个 `AgentConfig` 参数
2. **工具过滤生效**：通过 LogQL 可验证（或通过单元测试验证），`AgentDef` 的 `tools` 和 `disallowed_tools` 限制了 LLM 收到的工具定义
3. **AgentConfig::builder() 可用**：链式构建能正确构造出 `AgentConfig` 并构建出 `ReActAgent`
4. **所有测试通过**：`cargo test --workspace` 全部通过

## Edge Cases

| Edge Case | Handling |
|-----------|----------|
| `AgentDef.tools` 指定的工具不在已注册 tools 里 | 静默忽略 |
| `AgentDef.disallowed_tools` 排除了所有工具 | agent 无工具可用，LLM 只能纯文本回答 |
| `AgentConfig` 未提供 `llm` | builder 构建时报错 |
| `AgentConfig` 未提供 `session` | builder 自动创建内存 session |
| `AgentDef` 为 `None` | 不做工具过滤，使用全部已注册工具 |
| 既有 `AgentBuilder` 代码 | 废弃，替换为 `AgentConfig::builder()` |

## Open Questions

- 无
