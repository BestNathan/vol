# Requirements: AgentTool 内置化（agent 工具作为运行时内置工具）

> 日期：2026-08-20
> 状态：待用户审阅

## Background

`crates/vol-llm-agent/src/agent_tool.rs` 已有 `AgentTool`（工具名 `agent`）：按 `type` 从 `.agents/agents/*.md` 定义派发子 agent，跑完整 ReAct 循环后返回最终结果。但现状存在四个问题：

1. **从未注册进运行时**——`AgentRuntimeBuilder::build()` 只注册 builtin / task / fs / cli-tools / web / skill / MCP，没有任何生产路径使用 `AgentTool`，它只有单元测试。
2. **子 agent 会话不可观测**——每次派发新建内存 `InMemoryEntryStore`，用完即弃；其他 agent、UI、会话查询都无法看到这次执行。
3. **parent / depth 无记录**——嵌套派发时子 agent 继承的是父的注册表（同一 `AgentTool` 实例，`agent_path` 构造后固定），现有 `max_depth` 检查只在构造期生效、不随嵌套递增；循环派发（A→B→A）没有可用的防护 / 限制依据。
4. **派发键与内置定义不匹配**——现有实现按 `type` 派发；子 agent 本质就是 `.agents/agents/` 中已内置好的 agent 定义（配置与定义均固定），应按其唯一 `AgentDef.id`（如 `repo:test-runner`）派发。

目标：让 agent 通过内置的 `agent` 工具**自主决定**是否把子任务派发给另一个 agent（或自己的定义）协同处理——派发执行走与数据面提交任务一致的运行时设施（共享 SessionManager、工具注册表），区别只是决策方从「人」变为「agent 自己」——并把最终结果返回给派发方。

## Goals

1. **内置注册**：`AgentRuntimeBuilder::build()` 无条件注册 `agent` 工具（`for_test()` 同步镜像），所有 agent 开箱即用，与 fs/task 同待遇。
2. **按 agent id 派发**：工具参数为 `id`（`AgentDef.id` 唯一标识，含 scope 前缀）+ `prompt` + `description`；按 id 从 AgentLoader 查找**内置定义**，子 agent 以完整 ReAct 循环执行任务，工具同步返回最终结果给派发方。除此之外不新增参数。
3. **会话持久化**：子 agent 使用运行时共享的 SessionManager 创建并持久化自己的 session（文件或 DB，与运行时配置一致）；派发结束后其他 agent 可通过会话查询 / 上下文观测到该会话。
4. **深度守卫（唯一嵌套控制）**：派发层数由 **`max_depth` 配置**直接控制——注册 AgentTool 时作为构造参数保存，执行派发时检查。depth 语义为派发层级：根 agent = 0，每次派发子 agent depth = 父 + 1；调用 `agent` 前检查「自身 depth ≥ max_depth → 拒绝」。**默认 1 = 只允许派发一层**（根 agent 可派发，子 agent 不可再派发）；配置更大值可多层派发。不另设嵌套开关。
5. **parent / depth 记录**：子 agent 携带并记录 `parent_agent` 与 `depth`（第 n 层子 agent 的 depth = n），写入可观测的位置（会话元数据 / 配置）；depth 同时是守卫依据（见 Goal 4）。
6. **上下文贡献（参考 SkillInjector 方式）**：提供一个 `ContextContributor`（AgentInjector 式），把可用 agent 定义（id / name / description）贡献进 agent 上下文，**提示 agent 可以使用 `agent` 工具派发 subagent 协同处理任务**。无可用定义时输出为空。
7. **循环 / 自派发支持**：允许 A→B→A 或派发给自己的定义，每次派发等价于新开一个 session，受 `max_depth` 限制。

## Non-Goals

- **不落实** `AgentDef.tools` / `disallowed_tools` 的工具过滤（子 agent 继承父工具集，现状不变）。
- **不落实** `AgentDef.model` 模型覆盖。子 agent 使用的 LLM 以内置工具注册时注入的 LLM 为准（运行时默认 provider），与父 agent 的逐代理 LLM 解析无关——不等同于「继承父 agent 的 LLM」。
- **不支持在工具调用中直接传递 AgentDef**（内联 toml / 自定义定义）——子 agent 只限 `.agents/agents/` 中已内置的定义；动态传定义后续可做。
- **不**为子 agent 接入父 PluginRegistry，**不**流式回传执行过程（派发方拿最终结果即可）。
- **不**设计除 `id` / `prompt` / `description` 之外的派发参数。
- **不**实现「按 agent frontmatter 声明获得 agent 工具」——无条件内置。
- **不**改变 `AgentTool` 的 Sensitivity（维持 `Safe`）。

## Scope

**In：**

- `vol-llm-agent`：`AgentTool` 派发封装优化（按 id 查找、session 创建方式、depth 守卫、parent/depth 记录、`max_depth` 构造配置；如需给 `AgentLoader` 增加按 id 查找）；新增 AgentInjector（`ContextContributor`，参考 `vol-llm-skill::SkillInjector` 模式）
- `vol-llm-runtime`：`build()` 与 `for_test()` 注册 `agent` 工具 + 挂接 AgentInjector 及配套测试
- 测试：单元 + 集成，覆盖 gate
- `wiki-ingest` 摄入 wiki

**Out：**

- AgentDef 工具过滤 / 模型覆盖 / 内联传 AgentDef（见 Non-Goals）
- 数据面 / 控制面协议改动
- 前端 UI 改动

## Constraints

- 子 agent = `.agents/agents/` 中已定义的 agent（`AgentDef`）；配置与定义内置固定，派发仅按 id 引用，不新建定义。
- crate 边界：`vol-llm-runtime` 不得依赖 `vol-agent-server`；`AgentTool` 与 AgentInjector 留在 `vol-llm-agent`，依赖方向保持。
- 子 agent 会话使用与运行时一致的 SessionManager（文件 / DB 取决于 runtime config），不使用一次性内存 store。
- AgentInjector 复用 `vol_llm_context::ContextContributor` 协议，不改协议本身。
- 仓库约定：每个新 `pub fn` / handler 至少一个测试；无 doc tests；覆盖 gate `just cover-gate <crate> 80`（`main.rs` / `app.rs` / `health.rs` 除外）。

## Success Criteria

1. `AgentRuntimeBuilder::build()` 产出的共享 tool registry 包含 `agent` 工具（集成测试断言 tool_names）。
2. 按 id 派发返回最终结果：用已内置定义的 `AgentDef.id` 调用 `agent`，返回值是子 agent 的最终内容；id 不存在时返回错误并列出可用 id（集成测试）。
3. 会话可观测：一次派发完成后，能从运行时 SessionManager 读到该子 agent 的持久化会话（集成测试）。
4. parent/depth 记录正确：嵌套派发（A→B→C）时 depth 依次为 1、2，parent_agent 逐级正确（集成测试）。
5. 深度守卫按配置生效：默认 `max_depth=1` 时根 agent 可派发一层、depth≥1 的 agent 再派发返回错误；配置更大值（如 3）时多层派发成功（测试）。
6. 上下文贡献生效：挂接 AgentInjector 后，agent 上下文中包含可用 agent 定义列表与「可用 `agent` 工具派发 subagent」提示（测试断言 contributor 输出）；无定义时输出为空。
7. 覆盖率：`just cover-gate vol-llm-agent 80` 与 `just cover-gate vol-llm-runtime 80` 通过。
8. wiki-ingest 完成，wiki 索引更新。

## Edge Cases

- **id 不存在**：返回错误并列出可用 agent（id、name、description），替换现有按 type 的报错方式。
- **无任何 agent 定义**：AgentTool 返回提示「在 .agents/agents/ 下创建 .md 定义」；AgentInjector 输出为空（不注入提示）。
- **循环 / 自派发**：每次派发新开 session；depth 记录递增；depth ≥ max_depth 时报错（默认 1 时子 agent 即不可再派发）。
- **depth ≥ max_depth 的 agent 调用 `agent`**：直接返回错误（工具内守卫）；depth 0 的根 agent 在默认配置下派发不受影响。
- **并发派发**：同一 agent 并发派发多个子任务 → 各自独立 session，互不干扰。
- **参数缺失 / 非法**：serde 校验返回 `InvalidArguments`。
- **子 agent 执行失败**：返回 `ExecutionFailed`，错误信息包含子 agent 失败原因。

## Open Questions

1. 子 agent session 的键规则（session_id 生成方式、按 agent id/name 存储）——设计阶段结合 SessionManager 现状确定。
2. depth 语义对齐：派发层级语义为「根 = 0」，而现有 `AgentPath::root().depth()` 按 segments 计数（root = 1）——设计阶段对齐两者（复用 AgentPath 或改为派发层级计数）。
3. 共享实例的配置通道：`max_depth` 在注册时作为构造参数保存（已确认）；运行时 config 是否暴露该值（默认 1）待设计。
4. 派发链路上 depth 的来源：共享 `AgentTool` 实例如何获知「当前调用方 agent」的 depth / 身份（如 `ToolContext` 携带 agent 身份与 depth），设计阶段确定。
5. 按 id 查找：`AgentLoader` 目前只有 `get(name)` / `get_by_type(type)`，无 `get_by_id`，需要新增（实现细节，一并确认 scope 前缀行为）。
6. AgentInjector 挂接位置与刷新时机（agent 定义变更后是否实时反映到上下文；是否像 SkillInjector 一样支持外部过滤）——设计阶段确定。
