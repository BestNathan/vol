# Requirements: Agent Definition System (Simplified)

## Background

The project has a mature ReActAgent system (`vol-llm-agent`) with specialized agents (`vol-llm-agents`: CodingAgent, AdviceAgent, etc.) hardcoded in Rust. We want to enable user-defined agents through markdown files with YAML frontmatter in `.agents/agents/`, similar to how skills are defined in `.agents/skills/`. This allows users to create custom sub-agents for specialized tasks without writing Rust code.

The original requirement doc included AgentBus, AgentCommTool, and complex inter-agent communication. This simplified version removes all of that and focuses on the core foundation: define an agent, dispatch it as a tool, get a result.

## Goals

1. **Agent Definition via md+frontmatter**: Agents are defined as `.md` files in `.agents/agents/` with YAML frontmatter containing configuration (name, type, tools, model, etc.) and markdown body as the system prompt.

2. **AgentLoader**: Discovers, loads, and caches agent definitions from user (`~/.agents/agents`) and repo (`{working_dir}/.agents/agents`) scopes, following the same pattern as `SkillLoader`.

3. **AgentTool**: A tool implementing `ExecutableTool` that LLMs can call to dispatch sub-agents by `type`. The tool creates a ReActAgent instance from the agent definition template, runs the full ReAct loop (with tool use, thinking, iteration), and returns the final answer.

4. **Tool inheritance**: If an agent definition doesn't specify `tools`, it inherits the default coding tools (Read, Write, Edit, Glob, Grep, Bash).

## Non-Goals

- AgentBus / inter-agent communication (send, broadcast, request)
- AgentCommTool
- Template vs instance distinction (no runtime agent_id, no parent-child channels)
- Fork path (context inheritance / cache sharing)
- Remote agent execution
- MCP server management
- Agent Teams / multi-agent coordination
- UI color, hooks, permission modes (Claude Code specific)
- JSON format for agent definitions (md+frontmatter only)
- Dispatch by name (type-based dispatch only)

## Scope

### In Scope
- New crate: `crates/vol-llm-agent-def/` (standalone, following `vol-llm-skill` pattern)
- `AgentDef` struct with frontmatter fields
- `AgentLoader` with user/repo scope discovery
- `AgentTool` implementing `ExecutableTool` trait
- Integration with existing `ReActAgent` loop (sub-agent spawned from AgentTool)

### Out of Scope
- Modifying existing specialized agents (CodingAgent, etc.)
- Changing the ReActAgent core loop
- Modifying the skill system

## Constraints

- Must use the existing `md-frontmatter` crate for parsing
- Must follow the existing `ExecutableTool` trait pattern from `vol-llm-tool`
- Must use async I/O (`tokio::fs`) — this is an async-first codebase
- Must use `serde::Deserialize` for frontmatter parsing

## Agent Frontmatter Schema

```yaml
---
name: test-runner              # Required. Unique identifier for this agent template
type: test-runner              # Required. Dispatch key (what AgentTool matches on)
description: "Run tests..."    # Required. Guides LLM when to dispatch this agent
tools: [Bash, Read, Glob]      # Optional. Allowed tools. Defaults to full coding toolset
disallowed_tools: [Write]      # Optional. Blacklisted tools
model: sonnet                  # Optional. Model override
max_iterations: 20             # Optional. Max ReAct loop iterations (default: 5)
---

System prompt / instructions in markdown body...
```

## AgentTool Input Schema

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `type` | string | Yes | Agent type to match (dispatch key, matches frontmatter `type` field) |
| `prompt` | string | Yes | Full task instruction for the sub-agent |
| `description` | string | Yes | Short (3-5 word) description of the task |

**Dispatch logic:**
1. Find all agents with matching `type` field → if exactly one, use it; if multiple, pick the first (name match not required since type is the unique dispatch key in this simplified scope)
2. Create a ReActAgent instance with the agent's system prompt, tools, and config
3. Run the ReAct loop with the `prompt` as user input
4. Return the final answer content

## Success Criteria

1. `AgentLoader` discovers agents from both user and repo roots, with repo agents taking priority over user agents with the same name
2. `AgentTool` is registerable in `ToolRegistry` and callable by LLM
3. AgentTool correctly spawns a ReActAgent sub-agent with the defined tools, system prompt, and config
4. Sub-agent runs a full ReAct loop (thinking → tool calls → tool results → final answer)
5. AgentTool returns the sub-agent's final answer content to the calling LLM
6. At least one integration test verifying the full dispatch → spawn → run → return cycle
7. Clippy clean, tests pass

## Edge Cases

- **Duplicate names**: Repo scope overrides user scope (same as skills). Log a warning.
- **Missing frontmatter**: File is skipped with a warning (same as skill loading).
- **Invalid frontmatter fields**: Log warning and skip (strict error handling).
- **Type not found**: AgentTool returns an error message to the LLM so it can retry.
- **No agents defined**: AgentTool is registered but returns an informative error.
- **Circular dispatch**: Agent A dispatches Agent B which dispatches Agent A — need depth limit or recursion guard on AgentTool calls.
- **Agent with no system prompt body**: Use a default generic prompt.
- **tools list contains unknown tool names**: Filter out unknown tools, log warning, proceed with valid ones.
- **Sub-agent exceeds max_iterations**: Return partial result with a warning about iteration limit.

## Open Questions

1. Should the agent definition crate be standalone (`vol-llm-agent-def`) or integrated into `vol-llm-agent`? (Recommendation: standalone for clean separation, same as `vol-llm-skill`)
2. Should `type` default to the `name` if not specified? (Recommendation: yes, simplifies the common case where there's one agent per type)
3. Should the sub-agent share the parent's session or create its own? (Recommendation: own session — sub-agents are independent)
