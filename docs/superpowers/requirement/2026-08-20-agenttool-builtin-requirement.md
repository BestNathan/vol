# Requirements: AgentTool 内置化（agent 工具作为运行时内置工具）

> 日期：2026-08-20
> 状态：待用户审阅

## Background

`crates/vol-llm-agent/src/agent_tool.rs` 已有 `AgentTool`（工具名 `agent`）：按 `type` 从 `.agents/agents/*.md` 定义派发子 agent，跑完整 ReAct 循环后返回最终结果。但现状存在四个问题：

1. **从未注册进运行时**——`AgentRuntimeBuilder::build()` 只注册 builtin / task / fs / cli-tools / web / skill / MCP，没有任何生产路径使用 `AgentTool`，它只有单元测试。
2. **子 agent 会话不可观测**——每次派发新建内存 `InMemoryEntryStore`，用完即弃；其他 agent、UI、会话查询都无法看到这次执行。
3. **parent / depth 无记录**——嵌套派发时子 agent 继承的是父的注册表（同一 `AgentTool` 实例，`agent_path` 构造后固定），现有 `max_depth` 检查只在构造期生效、不随嵌套递增；循环派发（A→B→A）没有可用的防护 / 限制依据。
4. **能力控制未接入现有模型**——没有使用运行时已有的 CapabilityOverlay 机制。

目标：让 agent 通过内置的 `agent` 工具**自主决定**是否把子任务派发给另一个 agent（或自己的定义）协同处理——执行语义等价于数据面（web）直接提交一个任务，区别只是决策方从「人」变为「agent 自己」——并把最终结果返回给派发方。

## Goals

1. **内置注册**：`AgentRuntimeBuilder::build()` 无条件注册 `agent` 工具（`for_test()` 同步镜像），所有 agent 开箱即用，与 fs/task 同待遇。
2. **派发语义**：agent 调用 `agent`（参数 `type` / `prompt` / `description`，schema 保持现状）→ 子 agent 以完整 ReAct 循环执行任务 → 工具同步返回子 agent 最终结果给派发方。
3. **会话持久化**：子 agent 使用运行时共享的 SessionManager 创建并持久化自己的 session（文件或 DB，与运行时配置一致）；派发结束后其他 agent 可通过会话查询 / 上下文观测到该会话。
4. **能力覆盖复用**：派发封装内部复用现有 CapabilityOverlay 模型（键 `(agent_id, session_id)`，`effective_tools`）管理子 agent 的有效工具；**是否允许子 agent 再派发（嵌套）由 AgentTool 新增的 config 项控制**，禁用时子 agent 工具集中的 `agent` 通过 overlay 移除。不设计新的派发调用参数——这是 AgentTool 被使用时的内部封装逻辑。
5. **parent / depth 记录**：子 agent 携带并记录 `parent_agent` 与 `depth`，写入可观测的位置（会话元数据 / 配置）。depth 必须**随派发链路递增**（第 n 层子 agent 的 depth = n），而不是构造期固定值；本周期只记录、不强制。
6. **循环 / 自派发支持**：允许 A→B→A 或派发给自己的定义，每次派发等价于新开一个 session。本周期保留现有构造期 `max_depth` 检查作为唯一强制守卫；**按 depth 的强限制（嵌套层数上限）明确延后**，待 depth 记录落地后实现。

## Non-Goals

- **不落实** `AgentDef.tools` / `disallowed_tools` 的工具过滤（维持「子 agent 继承父工具集」现状；overlay 只处理 `agent` 工具本身）。
- **不落实** `AgentDef.model` 模型覆盖。子 agent 使用的 LLM 以内置工具注册时注入的 LLM 为准（运行时默认 provider），与父 agent 的逐代理 LLM 解析无关——不等同于「继承父 agent 的 LLM」。
- **不**为子 agent 接入父 PluginRegistry，**不**流式回传执行过程（派发方拿最终结果即可）。
- **不**新设计派发专用参数（`type` / `prompt` / `description` 不变）。
- **不**实现「按 agent frontmatter 声明获得 agent 工具」——无条件内置。
- **不**改变 `AgentTool` 的 Sensitivity（维持 `Safe`）。

## Scope

**In：**

- `vol-llm-agent`：`AgentTool` 派发封装优化（session 创建方式、overlay 接入、parent/depth 记录、新增 config 项）
- `vol-llm-runtime`：`build()` 与 `for_test()` 注册 `agent` 工具及配套测试
- 测试：单元 + 集成，覆盖 gate
- `wiki-ingest` 摄入 wiki

**Out：**

- AgentDef 工具过滤 / 模型覆盖（见 Non-Goals）
- 数据面 / 控制面协议改动
- 前端 UI 改动

## Constraints

- 复用现有能力覆盖模型 `vol_llm_core::capability_overlay::CapabilityOverlay`，不另造机制。
- crate 边界：`vol-llm-runtime` 不得依赖 `vol-agent-server`；`AgentTool` 留在 `vol-llm-agent`，依赖方向保持。
- 子 agent 会话使用与运行时一致的 SessionManager（文件 / DB 取决于 runtime config），不使用一次性内存 store。
- 仓库约定：每个新 `pub fn` / handler 至少一个测试；无 doc tests；覆盖 gate `just cover-gate <crate> 80`（`main.rs` / `app.rs` / `health.rs` 除外）。

## Success Criteria

1. `AgentRuntimeBuilder::build()` 产出的共享 tool registry 包含 `agent` 工具（集成测试断言 tool_names）。
2. 派发返回最终结果：调用 `agent` 后，返回值是子 agent 的最终内容（现有单测保留并扩展）。
3. 会话可观测：一次派发完成后，能从运行时 SessionManager 读到该子 agent 的持久化会话（集成测试）。
4. parent/depth 记录正确：嵌套派发（A→B→C）时 depth 依次为 1、2，parent_agent 逐级正确（集成测试）。
5. 嵌套开关生效：config 禁用嵌套时，子 agent 工具集中不含 `agent`（测试断言）；启用时嵌套派发可成功。
6. 构造期深度守卫：达到 `max_depth` 的派发返回错误（现有单测保留）。该检查本周期不随嵌套递增（见 Goal 6），嵌套层数限制延后到 depth 记录落地后。
7. 覆盖率：`just cover-gate vol-llm-agent 80` 与 `just cover-gate vol-llm-runtime 80` 通过。
8. wiki-ingest 完成，wiki 索引更新。

## Edge Cases

- **type 不存在**：返回错误并列出可用 agent 类型（现状保留）。
- **无任何 agent 定义**：返回提示「在 .agents/agents/ 下创建 .md 定义」（现状保留）。
- **循环 / 自派发**：每次派发新开 session；depth 记录递增；构造期 `max_depth` 检查照常生效（本周期不按嵌套层数强制，见 Goal 6）。
- **禁用嵌套时**：子 agent 注册表中无 `agent` 工具，模型即使尝试调用也因工具不存在而失败。
- **并发派发**：同一 agent 并发派发多个子任务 → 各自独立 session，互不干扰。
- **参数缺失 / 非法**：serde 校验返回 `InvalidArguments`（现状保留）。
- **子 agent 执行失败**：返回 `ExecutionFailed`，错误信息包含子 agent 失败原因（现状保留）。

## Open Questions

1. AgentTool 新增 config 项的具体形态与默认值：建议 `allow_nested_dispatch`（默认 `true`，与「支持循环派发」一致；`false` 时 overlay 移除子 agent 的 `agent` 工具）。设计阶段定。
2. 子 agent session 的键规则（session_id 生成方式、按 agent type/name 存储）——设计阶段结合 SessionManager 现状确定。overlay 键 `(agent_id, session_id)` 中的子 agent 瞬态身份如何确定（子 agent 不走 `register_agent`，其 ReActAgent 由派发封装直接构建）一并设计。
3. `max_depth` 内置注册的默认值：建议 3；是否暴露为运行时配置待设计阶段定。
4. 共享实例的配置通道：`allow_nested_dispatch` 与 `max_depth` 均需在注册时作为构造参数传入共享的 `AgentTool` 实例（工具没有每次派发的配置通道），运行时 config 是否暴露这两项待设计。
5. 派发链路上 depth 的来源：共享 `AgentTool` 实例如何获知「当前调用方 agent」的 depth / 身份（如 `ToolContext` 携带 agent 身份与 depth），设计阶段确定。
