# Agent Context Tab Design

## Summary

在 Agent Panel 新增 "Context" sub-tab，展示当前 agent 的 context contributor 列表（元数据），点击任意 contributor 弹出模态窗口查看其完整内容快照。

## Backend

### 新增 JSON-RPC 操作

**agent.context_config**

```json
// Request
{ "agent_id": "my-agent" }

// Response
{
  "contributors": [
    {
      "name": "system",
      "anchor_zone": "head",
      "estimated_tokens": 120,
      "message_count": 1
    },
    {
      "name": "skills",
      "anchor_zone": "head",
      "estimated_tokens": 340,
      "message_count": 1
    }
  ]
}
```

**agent.context_snapshot**

```json
// Request
{ "agent_id": "my-agent", "contributor_name": "skills" }

// Response
{
  "messages": [
    { "role": "user", "content": "Available skills:\n- skill-a: ..." }
  ]
}
```

### 实现

1. `agent_server_protocol.rs` — 加 `AgentOperation::ContextConfig` / `ContextSnapshot`，对应 Payload variants
2. `domain/agent.rs` — AgentHandler 新增两个分支：
   - `context_config`: 从 agent 的 `ContextBuilder` 获取 contributors，调 `name()` + `estimate_size()` + `contribute()` 提取 message_count
   - `context_snapshot`: 按 name 匹配 contributor，调 `contribute()` 展平 messages 返回
3. `AgentDispatcher` 加 `fn with_agent<T>(&self, f: impl FnOnce(&ReActAgent) -> T) -> T`，AgentHandler 通过它访问 `ContextBuilder`
4. `anchor_zone` 从 contributor 的第一个 `ContextBlock.anchor` 推导

## Frontend

### State (`state/mod.rs`)

```rust
// AgentSubTab 加 Context
pub enum AgentSubTab { Conversation, Sessions, Context }

// 新增 ContextState
pub struct ContextState {
    pub contributors: Vec<ContributorInfo>,
    pub dialog_contributor: Option<String>,   // 当前弹窗展示的 contributor
    pub dialog_messages: Vec<ContextMessage>,  // 弹窗中的完整消息
    pub loading_dialog: bool,
    pub error: Option<String>,
}
```

### 组件

**ContextPanel** (`context_panel.rs`) — 新文件

- 选中 agent 时加载 `context_config`，渲染 contributor 列表
- 每行：name、anchor zone 色标（Head=蓝 Middle=黄 Tail=绿）、token 数、消息数
- 点击行 → 调 `context_snapshot`，用结果填充 `dialog_messages`，打开 ContextDialog

**ContextDialog** — 模态窗口

- 标题栏显示 contributor name
- 消息列表：每条消息带 role 标签（system/user/assistant/tool 用不同颜色）+ 等宽字体可滚动内容区
- 关闭按钮

**AgentsPanel 改动**

- sub-tab bar 加 "Context" 按钮
- 路由新增 `AgentSubTab::Context => rsx! { ContextPanel {} }`

### 客户端 (`client.rs`)

```rust
pub fn agent_context_config(&self, agent_id: &str, cb: ...)
pub fn agent_context_snapshot(&self, agent_id: &str, contributor_name: &str, cb: ...)
```

## Data Flow

```
AgentPanel
  └─ select agent
       └─ ContextPanel
            └─ agent.context_config → contributor list
            └─ click row
                 └─ agent.context_snapshot(name) → open ContextDialog
```
