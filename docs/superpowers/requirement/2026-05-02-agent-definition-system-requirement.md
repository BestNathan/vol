# Requirements: Agent Definition System (md+frontmatter)

## Background

The project has a mature ReActAgent system (`vol-llm-agent`) with specialized agents (`vol-llm-agents`: CodingAgent, AdviceAgent, etc.) hardcoded in Rust. We want to enable user-defined agents through markdown files with YAML frontmatter in `.agents/agents/`, similar to how skills are defined in `.agents/skills/`. This allows users to create custom sub-agents for specialized tasks without writing Rust code.

## Goals

1. **Agent Definition via md+frontmatter**: Agents are defined as `.md` files in `.agents/agents/` directories with YAML frontmatter containing configuration and markdown body as system prompt.
2. **AgentLoader**: Discovers, loads, and caches agent definitions from user (`~/.agents/agents`) and repo (`{working_dir}/.agents/agents`) scopes, following the same pattern as `SkillLoader`.
3. **AgentTool**: A tool that LLMs can call to dispatch sub-agents. Supports two invocation modes:
   - `type` + `prompt` + `description`: Dispatch a new agent by type (matches against agent definitions' `type` field)
   - `name` + `prompt` + `description`: Dispatch a task to an existing agent instance by name
4. **Template vs Instance**: Agent definition files on disk are **templates** — they define the blueprint (name, type, tools, skills, system prompt). When AgentTool dispatches an agent, it creates a **runtime instance** from the template with a unique `agent_id` (generated at spawn time). The file's `name` field uniquely identifies that template. The `type` field is the dispatch key — multiple templates can share the same type (e.g., two different "code-reviewer" templates with different focuses), and AgentTool picks the best match by `description`. A runtime instance is identified by its spawned `agent_id`, not by the template's `name`.

The `name` field on a file IS required (every template must have one), but it serves as the template identifier, not a running instance ID. The running instance gets its own auto-generated ID at spawn time.
5. **Tool/Skill inheritance**: If an agent definition doesn't specify `tools` or `skills`, it inherits from the parent agent that spawned it.
6. **AgentBus — 跨 Agent 通信总线**: 所有运行中的 agent 实例都注册到一个全局事件总线上，支持三种通信模式：
   - **直接投递（Send）**：通过 `agent_id` 或 `agent_name` 向特定 agent 实例发送消息
   - **类型广播（Broadcast）**：通过 `agent_type` 向所有匹配该类型的 agent 实例广播消息
   - **父子通道（Local）**：父 agent 与直接子 agent 之间自动建立专用通道，支持双向通信
   - **等待回复（Request）**：发送消息并阻塞等待目标 agent 回复（类似 RPC 调用）
   - 隔离：通过 agent_id 隔离，只有目标 agent 能收到消息。父子通道天然隔离。类型广播是可选的开放通道。

## Non-Goals

- Fork path (context inheritance / cache sharing) — not in scope for v1
- Remote agent execution
- MCP server management
- Agent Teams / multi-agent coordination（复杂团队编排）
- UI color, hooks, permission modes (these are Claude Code specific)
- JSON format for agent definitions (md+frontmatter only for now)

## Scope

### In Scope
- New crate: `crates/vol-llm-agent-def/` (or integrate into existing crate)
- `AgentDef` struct with frontmatter fields
- `AgentLoader` with user/repo scope discovery
- `AgentTool` implementing `ExecutableTool` trait
- `AgentBus` — 全局事件总线，支持 send/broadcast/request 三种操作
- `AgentCommTool` — 专门用于 agent 间通信的工具（send, broadcast, request, wait）
- 集成到现有 `ReActAgent` 循环（AgentBus 作为 ContextContributor 注入）
- Integration with existing `SkillLoader` for skill references in agent defs

### Out of Scope
- Modifying existing specialized agents (CodingAgent, etc.)
- Changing the ReActAgent core loop
- Modifying the skill system

## Constraints

- Must use the existing `md-frontmatter` crate for parsing
- Must follow the existing `ExecutableTool` trait pattern from `vol-llm-tool`
- Must use async I/O (`tokio::fs`) — this is an async-first codebase
- Agent definitions must be compatible with the existing `AgentConfig` struct
- Must use `serde::Deserialize` for frontmatter parsing

## Agent Frontmatter Schema

```yaml
---
name: test-runner              # Required. Unique identifier for this agent
type: test-runner              # Required. Dispatch key (what AgentTool matches on)
description: "Run tests..."    # Required. Guides LLM when to dispatch
tools: [Bash, Read, Glob]      # Optional. Allowed tools. Inherits from parent if omitted
disallowed_tools: [Write]      # Optional. Blacklisted tools
skills: [testing]              # Optional. Pre-loaded skills. Inherits if omitted
model: sonnet                  # Optional. Model override
max_iterations: 20             # Optional. Max ReAct loop iterations
max_turns: 50                  # Optional. Alias for max_iterations
---

System prompt / instructions in markdown body...
```

## AgentTool Input Schema

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `description` | string | Yes | 3-5 word task description |
| `prompt` | string | Yes | Full task instruction for the agent |
| `subagent_type` | string | No | Agent type to match (dispatch key). Defaults to matching any |
| `name` | string | No | Specific agent name to target |

**Dispatch logic:**
1. If `name` is provided → find agent by name → run with prompt
2. If `subagent_type` is provided → find agents matching type → pick best match by description → run with prompt
3. If neither → use a default general-purpose behavior (inherit parent's tools/skills)

## AgentBus 通信架构

### Agent 注册模型

每个运行中的 agent 实例启动时向 AgentBus 注册：
```
agent_id: "agent_a1b2c3"       # 自动生成的唯一 ID
agent_name: "test-runner"       # 来自模板 name 字段
agent_type: "test-runner"       # 来自模板 type 字段
parent_id: "agent_x9y8z7"       # 父 agent 的 ID（根 agent 为 None）
children: ["agent_d4e5f6"]      # 直接子 agent 列表
```

### 三种通信模式

**1. Send（直接投递）**：
```
agent_send(agent_id="agent_a1b2c3", message="Please review the test results")
agent_send(agent_name="test-runner", message="Check the new module")
```
- 精准投递到单个 agent
- 消息进入目标 agent 的收件箱，在下一次 LLM 调用前自动注入到 context

**2. Broadcast（类型广播）**：
```
agent_broadcast(agent_type="code-reviewer", message="New PR needs review")
```
- 所有匹配 `agent_type` 的 agent 都会收到消息
- 用于一对多场景

**3. Request（请求-回复）**：
```
agent_request(agent_id="agent_a1b2c3", prompt="What's the status?", timeout=30)
```
- 发送请求并阻塞等待回复
- 目标 agent 收到消息后处理，通过 `agent_reply` 返回结果
- 超时后返回错误

### AgentCommTool 工具定义

AgentBus 通过 `AgentCommTool` 暴露给 agent 使用：

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `action` | string | Yes | `send`, `broadcast`, `request`, `reply`, `wait` |
| `target_id` | string | No | 目标 agent_id（用于 send/request） |
| `target_name` | string | No | 目标 agent name（用于 send/request） |
| `target_type` | string | No | 目标 agent type（用于 broadcast） |
| `message` | string | Yes (除 wait) | 消息内容 |
| `prompt` | string | No | 更详细的任务指令（request 时使用） |
| `timeout` | number | No | 等待超时（秒），默认 60 |
| `request_id` | string | No | 回复时关联的请求 ID |

### Agent 上下文注入

每次子 agent LLM 调用前，AgentBus 自动检查收件箱：
- 如果有新消息 → 在 context 中注入 `"You have N new messages: [from: agent_X] message content..."`
- 如果有 pending request 回复 → 注入 `"Reply to your request from agent_Y: ..."`

这使得 agent 能被动接收消息，而不需要主动轮询。

## Success Criteria

1. `AgentLoader` discovers agents from both user and repo roots, with repo agents taking priority over user agents with the same name
2. `AgentTool` is registerable in `ToolRegistry` and callable by LLM
3. AgentTool correctly spawns a ReActAgent subagent with the defined tools, skills, system prompt, and config
4. Template agents (from files) can be dispatched by `type`
5. Agents without `tools`/`skills` specified inherit from the calling context
6. `AgentBus` supports send, broadcast, and request operations
7. Spawned agents can communicate via AgentCommTool (send messages, receive replies)
8. Messages are automatically injected into target agent's context
9. At least one integration test verifying the full dispatch → spawn → communicate → run cycle
10. Clippy clean, tests pass

## Edge Cases

- **Duplicate names**: Repo scope overrides user scope (same as skills). Log a warning.
- **Missing frontmatter**: File is skipped with a warning (same as skill loading).
- **Invalid frontmatter fields**: Log warning and skip (strict error handling, consistent with md-frontmatter).
- **Type not found**: AgentTool returns an error message to the LLM so it can retry.
- **No agents defined**: AgentTool is still registered but returns an informative error.
- **Circular dispatch**: Agent A dispatches Agent B which dispatches Agent A — need depth limit or recursion guard.
- **Agent with no system prompt body**: Use a default generic prompt.
- **tools list contains unknown tool names**: Filter out unknown tools, log warning, proceed with valid ones.
- **Send to non-existent agent**: Return error with "agent not found" message.
- **Request timeout**: Target agent is busy or dead — return timeout error after N seconds.
- **Message overflow**: Too many messages in inbox — cap at N messages, oldest are dropped with a warning.
- **Broadcast to zero agents**: Return informative message "no agents of type X are running".
- **Agent termination cleanup**: When agent finishes, deregister from AgentBus, cancel pending requests.

## Open Questions

1. Should the agent definition crate be standalone (`vol-llm-agent-def`) or integrated into `vol-llm-agent`? (Recommendation: standalone for clean separation, same as `vol-llm-skill`)
2. Should `type` default to the `name` if not specified? (Recommendation: yes, simplifies the common case where there's one agent per type)
