# Design: AgentTool 内置化（agent 工具作为运行时内置工具）

> 日期：2026-08-20
> 状态：待用户审阅
> 需求文档：[[2026-08-20-agenttool-builtin-requirement]]

## 1. Background

`AgentTool`（工具名 `agent`）已存在于 `vol-llm-agent`，可从 `.agents/agents/*.md` 定义派发子 agent 跑完整 ReAct 循环，但从未注册进运行时、子 agent 会话用后即弃、无 parent/depth 记录、按 `type` 而非唯一 `AgentDef.id` 派发。本设计将其重构为独立高层 crate 并注册为运行时内置工具：agent 可自主决定把子任务派发给内置定义的另一个 agent（或自己的定义）协同处理，最终结果返回派发方，子 agent 会话持久化可观测。

已批准的设计决策（brainstorming 输出）：

1. AgentTool 迁移至**独立新 crate `vol-llm-agent-tool`**（高层组合 crate，依赖底层实现）
2. parent/depth 记录落点：**扩展 `vol-llm-core::AgentDef`**
3. 子 agent 会话存储键：**按 `def.name`**（与 `register_agent` 及现有 `session.list` 查询一致）
4. 嵌套控制：`max_depth` **随 agent 定义配置**（内置 `tool_config` 机制），默认 1，执行时检查
5. 上下文贡献：新增 **AgentInjector**（参考 `SkillInjector` 模式）
6. 架构方案：**直接注入依赖**（SessionManager / LLM 进构造），不引入新 trait

## 2. Architecture: crate 布局

```
crates/
├── vol-llm-agent-tool/        # 新 crate（workspace 新增成员）
│   ├── lib.rs                 # 导出 AgentTool、AgentInjector
│   ├── agent_tool.rs          # 从 vol-llm-agent 迁入 + 重构
│   └── injector.rs            # 新增 AgentInjector
├── vol-llm-agent/             # 删除 agent_tool.rs 与 pub use（无其他调用方，迁移干净）
├── vol-llm-core/              # AgentDef 增加 parent_agent / depth 字段
├── vol-llm-agent/             # AgentLoader 新增 get_by_id
└── vol-llm-runtime/           # 新增依赖 vol-llm-agent-tool，注册内置工具
```

依赖方向（单向无环）：

```
vol-llm-runtime ──→ vol-llm-agent-tool ──→ vol-llm-agent / vol-session / vol-llm-tool / vol-llm-core / vol-llm-context
```

## 3. Components

### 3.1 AgentDef 扩展（vol-llm-core）

```rust
pub struct AgentDef {
    // ...现有字段
    /// 派发方 agent id（根 agent 为 None）
    pub parent_agent: Option<String>,   // 默认 None
    /// 派发层级：根 = 0，每次派发 +1
    pub depth: u32,                     // 默认 0
}
```

- `#[derive(Debug, Clone)]` 保持，clone 时随 def 一起携带（含 `tool_config`）
- `ToolContext.agent_def`（vol-llm-tool/src/tool.rs:49）在工具执行时由 ReAct 循环填充（react/agent.rs:746），因此 **AgentTool 执行时天然可读调用方 depth/parent/tool_config**，无需新增传递通道

### 3.2 AgentLoader::get_by_id（vol-llm-agent）

```rust
pub async fn get_by_id(&self, id: &str) -> Option<Arc<AgentDef>>
```

按 `AgentDef.id`（`"{scope}:{name}"`，如 `repo:test-runner`）精确查找，替代 `get_by_type` 作为派发查找。

### 3.3 AgentTool（vol-llm-agent-tool）

**构造**（注册时注入依赖，无 max_depth 参数）：

```rust
AgentTool::new(
    loader: Arc<AgentLoader>,           // 按 id 查找内置定义
    llm: Arc<dyn LLMClient>,            // 运行时默认 provider
    session_manager: Arc<dyn SessionManager>, // 子 agent 会话持久化
    parent_tools: Arc<ToolRegistry>,    // 子 agent 继承的工具集
)
```

- 移除死代码 `working_dir` 字段与 `agent_path`（深度守卫改用 `AgentDef.depth`）
- 工具名 `agent`；参数 schema：`id` / `prompt` / `description`（原 `type` 更名 `id`）；Sensitivity 维持 `Safe`

**execute 流程**：

1. 解析参数 → 非法则 `InvalidArguments`
2. 读 `context.agent_def`（调用方）；无则按根 agent（depth=0、parent=None）处理
3. 读取调用方 `tool_config.agent.max_depth`（缺省/非法 → 默认 1）
4. **深度守卫**：`调用方 depth >= max_depth` → `ExecutionFailed("maximum dispatch depth …")`
5. `loader.get_by_id(id)` → 未命中则 `ExecutionFailed` + 列出可用 agent（id/name/description）；无任何定义时提示在 `.agents/agents/` 创建
6. `sub_def = (*def).clone()`；写 `parent_agent = 调用方 id`、`depth = 调用方 depth + 1`（`tool_config` 随 clone 保留——子 agent 再派发时读的是自己的 tool_config）
7. `Session::new(session_manager.entry_store_for_agent(&sub_def.name))` —— 按 name 持久化，`session.list` 可查
8. 构建 `AgentConfig`（with_def / with_llm / with_tools / with_session，`PluginRegistry::new()` 不变）→ `ReActAgent::new(...).run(&prompt)`
9. 成功返回 `ToolResult::success(最终内容)`；子 agent 失败 → `ExecutionFailed` 含原因

### 3.4 max_depth 配置（随 agent 定义）

复用内置 `tool_config` 机制（与 `tool_config: { bash: { sandbox: … } }` 同构），**一般无需设置**：

```yaml
# .agents/agents/foo.md frontmatter
---
name: foo
tool_config:
  agent:
    max_depth: 3   # 该 agent 在派发链中继续派发的深度上限；缺省 1
---
```

- 执行时从 `context.agent_def.tool_config["agent"]["max_depth"]` 读取调用方配置
- 语义：调用方 def 的 max_depth 决定「该 agent 还能不能再派发」——默认 1 时根 agent（depth 0）可派发一层，子 agent（depth 1 ≥ 1）被拒；需要更深链路时在链路中各 agent 定义里设更大值
- 不新建全局配置文件；运行时 config 不暴露该值

### 3.5 AgentInjector（vol-llm-agent-tool）

参考 `SkillInjector`（vol-llm-skill/src/injector.rs）：

```rust
pub struct AgentInjector {
    loader: Arc<AgentLoader>,
    anchor: AttentionAnchor,          // Head(1)，跟随 skill 惯例
    cached_size: tokio::sync::Mutex<usize>,
}
impl ContextContributor for AgentInjector // name: "agents"
```

- `contribute()`：`list_metadata()` 非空时输出一个 ContextBlock：

```text
You can dispatch sub-agents to handle tasks collaboratively using the `agent` tool
(args: id, prompt, description). Available agents:
- repo:explore (explore): 搜索代码库…
- repo:review (review): 代码审查…
```

- 无定义时输出为空（不注入）
- 挂接：`register_agent` 构建 AgentConfig 时 `with_contributor(...)`（静态挂接；无 per-run 过滤需求，不需要 skill 那种 run 时 resolve+替换机制）

### 3.6 运行时接线（vol-llm-runtime）

`AgentRuntimeBuilder::build()` 变更：

1. 创建共享 `Arc<AgentLoader>`，存入新增的 `AgentRuntime.agent_loader` 字段；`discover_agents` 复用（不再自建）——保证注册定义与派发定义同源
2. registry **先 Arc 化再注册**（解决 AgentTool 需 `Arc<ToolRegistry>` 的循环引用）：`Arc::new(ToolRegistry::new())` → 注册 builtin/task/fs/cli-tools/web/skill/MCP → 注册 `AgentTool` → 使用
3. AgentTool 的 LLM：`llm_registry` 默认 provider（与 `resolve_llm_for_agent` 的 first_id 回退一致）
4. `for_test()` 同步镜像（含 AgentTool 注册）

`register_agent`：挂接 AgentInjector contributor。

## 4. Data Flow（一次完整派发）

```
用户 → 根 agent（def.depth=0, parent=None）
  ├─ 上下文含 AgentInjector 输出的可用 agent 列表 + 派发提示
  ├─ 模型调用 agent { id: "repo:explore", prompt, description }
  │    ├─ ToolContext.agent_def = 根 agent def
  │    ├─ 守卫：tool_config.agent.max_depth（默认1）→ depth 0 < 1 ✓
  │    ├─ get_by_id → 命中
  │    ├─ sub_def = clone；parent_agent=根.id；depth=1（tool_config 随 clone 保留）
  │    ├─ Session::new(entry_store_for_agent("explore")) → 持久化（UI/其他 agent 可查）
  │    ├─ ReActAgent(sub_def).run(prompt) —— 子 agent 若再调 agent：
  │    │     ToolContext.agent_def = sub_def（depth=1）→ 守卫 1 ≥ 1 → 报错（默认）
  │    └─ 返回最终内容
  └─ 根 agent 拿到结果继续任务
```

## 5. Error Handling

| 场景 | 行为 |
|------|------|
| 参数缺失/非法 | `InvalidArguments` |
| `id` 不存在 | `ExecutionFailed` + 列出可用 agent（id/name/description） |
| 无任何 agent 定义 | `ExecutionFailed` + 提示在 `.agents/agents/` 创建定义 |
| depth ≥ max_depth | `ExecutionFailed("maximum dispatch depth …")` |
| 子 agent 执行失败 | `ExecutionFailed` 含子 agent 失败原因 |

## 6. Testing

| 位置 | 测试 |
|------|------|
| `vol-llm-agent-tool`（迁入原 3 个单测并重写为 id/depth 语义） | 深度守卫（默认 1：depth≥1 拒绝；tool_config 设 3 时多层通过）、id 不存在列出可用、派发成功返回内容、parent/depth 写入 sub_def（断言）、子 agent 会话落入 `entry_store_for_agent(name)`（测试版 SessionManager 验证） |
| `vol-llm-agent-tool` AgentInjector | 有定义时输出含 id 列表与派发提示；无定义时输出为空 |
| `vol-llm-core` | AgentDef 新字段默认值（parent=None、depth=0） |
| `vol-llm-agent` | `get_by_id` 命中/未命中 |
| `vol-llm-runtime` 集成 | `build()` registry 含 `agent`；`for_test()` 镜像；注册不破坏现有工具集 |
| 质量门 | `just cover-gate vol-llm-agent-tool 80`、`just cover-gate vol-llm-runtime 80`、无 doc tests、`./scripts/check-agent-boundaries.sh` |

## 7. Non-Goals（承接需求文档）

- 不落实 `AgentDef.tools` / `disallowed_tools` 工具过滤与 `AgentDef.model` 模型覆盖
- 不支持工具调用中直接传 AgentDef（内联定义，后续可做）
- 不为子 agent 接入父 PluginRegistry、不流式回传执行过程
- 不实现按 frontmatter 声明获得 agent 工具（无条件内置）
- 子 agent 不注册进运行时 agent_defs/status（观测只在 session 层）

## 8. Implementation Notes

- 新 crate 加入 workspace members；`docs/wiki` 与 CLAUDE.md 结构清单同步（wiki-ingest）
- `vol-llm-agent` Cargo.toml：迁出 agent_tool.rs 后如无 crate 使用 `vol-llm-tool` 的某些依赖保持不变即可，无新边界问题（`check-agent-boundaries.sh` 验证）
- 现有 agent_tool 单测（depth limit / type not found / dispatch）随文件迁入新 crate，按 id/depth 语义重写
