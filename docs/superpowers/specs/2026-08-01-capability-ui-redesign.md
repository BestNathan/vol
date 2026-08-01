# Capability Selection UI Redesign

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 重新设计 Agent 运行时额外能力（Provider/Model、Tools、Skills、MCP Servers）的选择 UI，解决下拉面板定位问题，改为即时生效无需 Apply，并补全 Provider/Model 选择能力。

**Architecture:** 将当前的 `CapabilityDropdown`（absolute 弹出层）替换为右侧滑入 Drawer 面板，每个能力项使用 toggle switch 即时开关，Provider/Model 使用联动下拉菜单。

**Tech Stack:** Rust, Dioxus WASM, Tailwind CSS

---

## File Structure

| 文件 | 操作 | 说明 |
|------|------|------|
| `crates/vol-llm-ui/src/web/components/capability_bar.rs` | Rewrite | CapabilityBar 重写：入口不变，展开改为触发 Drawer |
| `crates/vol-llm-ui/src/web/components/capability_drawer.rs` | **Create** | 新增：右侧 Drawer 面板，包含所有能力选择 UI |
| `crates/vol-llm-ui/src/web/components/mod.rs` | Modify | 注册新模块 |
| `crates/vol-llm-ui/src/state/mod.rs` | Modify | 添加 `CapabilityDrawerState`；清理无用 `GlobalState.capabilities` |
| `crates/vol-llm-ui/src/web/client.rs` | Modify | 可能需要添加 `agent_get_providers` / `agent_update_provider` RPC（取决于后端接口） |
| `crates/vol-llm-ui/src/web/components/app.rs` | Modify | 注册 Drawer 组件（顶层渲染） |

---

## Design Details

### 1. 整体布局

```
┌─────────────────────────────────────────┐
│ StatusBar                               │
├────────┬────────────────────────────────┤
│        │ Agents Tab                     │
│ File   │ ┌────────────────────────┐     │
│ Tree   │ │ Conversation View      │     │
│        │ └────────────────────────┘     │
│        │ 🛠 12 tools · 5 skills · 3 MCPs [✎] ← CapabilityBar (position: fixed)
│        │ [Input Area]                    │
├────────┴────────────────────────────────┤
│        │                    ┌───────────┤ ← Drawer: fixed right-0 top-0 h-full
│        │                    │ Capabilities      │
│        │                    │ ┌───────────────┐ │
│        │                    │ │ Provider   ▼  │ │
│        │                    │ │ Model      ▼  │ │
│        │                    │ │───────────────│ │
│        │                    │ │ 🔍 search...  │ │
│        │                    │ │───────────────│ │
│        │                    │ │ ▼ Tools  (12) │ │
│        │                    │ │ ⬤ tool_a      │ │
│        │                    │ │ ⬤ tool_b      │ │
│        │                    │ │ ▼ Skills (5)  │ │
│        │                    │ │ ⬤ skill_a     │ │
│        │                    │ │ ▼ MCP    (3)  │ │
│        │                    │ │ ⬤ server_a    │ │
│        │                    │ └───────────────┘ │
│        │                    └───────────────────┤
└─────────────────────────────────────────────────┘
```

- **遮罩层**: 半透明黑色遮罩（`bg-black/50`），点击关闭 Drawer
- **Drawer 面板**: `fixed right-0 top-0 h-full w-80`，彻底解决 absolute 定位问题；`z-50` 确保在最上层
- **CapabilityBar**: 入口和计数展示不变

### 2. 组件树

Drawer 在 App 顶层渲染（`fixed` 定位，不嵌套在 CapabilityBar 内），CapabilityBar 通过共享 Signal 触发开关：

```
App
├── CapabilityBar               ← 入口，显示计数 + ✎ 按钮（点击设置 drawer_state.open = true）
├── ...其他面板...
└── CapabilityDrawer             ← 新增：顶层 fixed 定位，读取 drawer_state
      ├── DrawerHeader             ← 标题 "Capabilities" + 关闭按钮 ✕
      ├── ProviderSection          ← Provider + Model 联动下拉（可折叠）
      │     ├── ProviderDropdown
      │     └── ModelDropdown
      ├── SearchInput              ← 实时过滤所有分组
      ├── SectionGroup["Tools"]    ← 可折叠分组
      │     └── CapabilityToggle[] ← toggle switch × N
      ├── SectionGroup["Skills"]   ← 可折叠分组
      │     └── CapabilityToggle[]
      └── SectionGroup["MCP"]      ← 可折叠分组
            └── CapabilityToggle[]
```

### 3. 交互行为

| 操作 | 行为 |
|------|------|
| 点击 ✎ | 打开 Drawer，拉取最新能力列表 + Provider 列表 |
| 切换 toggle | **即时调用** `agent_update_capabilities`；toggle 右侧显示 loading spinner（`◌`），成功显示 `✓`（1.5s 后回到 idle），失败显示 `⚠`（hover 显示错误信息） |
| 选 Provider | 即时生效（调用对应 RPC），Model 下拉自动更新为该 Provider 的模型列表 |
| 选 Model | 即时生效 |
| 搜索 | 实时过滤所有分组中的项（名称匹配），匹配项高亮，空结果显示 "No matching capabilities" |
| 点击遮罩 / Drawer 外区域 / ✕ | 关闭 Drawer |
| 折叠/展开分组 | 点击分组标题栏，默认全部展开 |

### 4. Toggle 即时反馈状态

```
idle:    [══════════○] tool_name     ← 默认态
saving:  [══════◌════] tool_name     ← 请求中，spinner 动画
saved:   [══════✓════] tool_name     ← 成功，1.5s 后自动回到 idle
error:   [══════⚠════] tool_name     ← 失败，hover tooltip 显示错误信息
```

实现方式：每个 toggle 维护一个本地 `saving_state: Option<SavingState>` 枚举：

```rust
enum ToggleSavingState {
    Saving,
    Saved,
    Error(String),
}
```

- 基础能力（agent 默认自带）与额外能力（用户手动添加）通过名称颜色区分：
  - 基础: `text-[#e0e0e0]`（正常色）
  - 额外: `text-[#80a0ff]`（蓝色）

### 5. Provider/Model 选择器

```
┌───────────────────────────────────────┐
│ Provider  [Anthropic            ▼]    │
│ Model     [claude-sonnet-5      ▼]    │
└───────────────────────────────────────┘
```

- 加载时获取可用的 Provider 列表和当前选中的 Provider/Model
- 切换 Provider → Model 下拉自动刷新为该 Provider 的模型列表，自动选中第一个模型
- 切换 Model → 即时调用更新 RPC
- 如果后端尚无 Provider/Model 查询/更新 RPC，此区域先以 `disabled` 状态占位，后续协议扩展后启用

### 6. 状态管理

新增 `CapabilityDrawerState`：

```rust
pub struct CapabilityDrawerState {
    pub open: bool,
    pub search: String,
    pub collapsed_sections: HashSet<String>,  // "Provider" | "Tools" | "Skills" | "Mcp"
    pub providers: Vec<ProviderOption>,
    pub selected_provider: String,
    pub selected_model: String,
    pub saving_states: HashMap<String, ToggleSavingState>, // key: "tools:name" / "skills:name" / "mcps:name"
}
```

- 清理 `GlobalState.capabilities` 死字段
- `CapabilityOverlayState` 继续用于数据加载，Drawer 从它读取 `available_*` / `effective_*` / `base_*`

### 7. 数据流

```
Drawer open
  → dp_client.agent_get_capabilities(agent_id, session_id)  ← 现有
  → dp_client.agent_get_providers(agent_id)                  ← 新增（或复用现有接口）
  → 填充 available_* / effective_* / providers

Toggle change
  → 设置 saving_state = Saving
  → dp_client.agent_update_capabilities(agent_id, session_id, tools, skills, mcps)
  → 成功: saving_state = Saved, 1.5s timer 后 → None
  → 失败: saving_state = Error(msg)
  → 无论成败都刷新 effective_* 到 drawer state

Provider/Model change
  → dp_client.agent_update_provider(agent_id, provider, model)
  → 成功: 更新 selected_provider / selected_model
  → 失败: 回滚选择 + toast 错误
```

### 8. 边界情况

| 场景 | 处理 |
|------|------|
| 无 Agent 选中 | ✎ 按钮 disabled，点击无反应 |
| DP 连接不可用 | 打开 Drawer 时显示 "No DP connection" 错误状态 |
| Skills 后端返回空 | 折叠分组仍显示，内容区显示 "No skills discovered" |
| 搜索无匹配 | 各分组保留标题，内容区统一显示 "No matching capabilities" |
| 快速连续切换同一个 toggle | 上一次请求的响应到达时检查是否为最新选中状态，过期响应丢弃 |
| Drawer 打开时切换 Agent | 关闭 Drawer，等新 Agent 加载完毕后再打开需重新点击 ✎ |
| Provider 列表加载失败 | Provider 下拉显示 disabled + "Failed to load" |

### 9. Provider/Model 协议说明

当前后端 `vol-llm-core` 中 `AgentDef` 已有 `model: Option<String>` 字段，`vol-llm-provider` 支持多 Provider。但前端目前无任何 Provider 选择 UI。

本次优先在 UI 中预留 Provider/Model 选择区域（用现有 `agent.config` 或扩展 RPC）。如果当前协议不支持动态切换 Provider/Model，则：

- **Phase 1（本次）**: UI 区域显示当前 `AgentDef.model` 值（只读），toggle 为 disabled 状态并标注 "Coming soon"
- **Phase 2（后续）**: 后端新增 `agent.get_providers` / `agent.update_provider` RPC，前端激活交互

---

## Acceptance Criteria

1. CapabilityBar 入口显示正确的 effective 计数（tools / skills / MCPs）
2. 点击 ✎ 打开右侧 Drawer，遮罩层半透明，点击遮罩或 ✕ 关闭
3. Drawer 中 Tools / Skills / MCP 分组可折叠，默认展开
4. 每个能力项显示名称和 toggle，基础能力与额外能力颜色区分
5. 切换 toggle **即时生效**，无需 Apply 按钮，显示保存状态（saving → saved/error）
6. 搜索框实时过滤所有分组
7. Drawer 定位正确（fixed right-0 top-0 h-full），不受父容器 overflow 影响
8. Provider/Model 区域存在并根据协议可用性显示只读或可交互状态
