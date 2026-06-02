---
type: source
source_type: code
date: 2026-05-23
ingested: 2026-05-23
tags: [agent, discovery, directory, frontend-selector]
---

# Agent Directory Discovery

**Authors/Creators:** BestNathan
**Date:** 2026-05-23
**Link:** docs/superpowers/specs/2026-05-23-agent-discovery-design.md

## TL;DR

Replaced manual `AgentDef::new()` + `register_agent()` with `discover_agents()` in `jsonrpc_agent_service.rs`. Created 3 agent definition files (general-purpose, explore, review) under `.agents/agents/`. Enriched `agent.list` to return type/description/scope metadata. Added agent selector dropdown to frontend input area.

## Key Takeaways

- Agent definitions are now `.md` files with YAML frontmatter under `.agents/agents/`
- `AgentLoader` auto-discovers agents from `~/.agents/agents/` (user) and `{working_dir}/.agents/agents/` (repo)
- `AgentServerCore::discover_agents()` registers all discovered agents automatically
- `agent.list` now returns `type`, `description`, `scope` alongside `id` and `name`
- Frontend `submit()` accepts optional `target` parameter for agent routing
- Input area has a `<select>` dropdown to choose which agent to talk to

## Detailed Summary

Three agent definitions created. `AgentServerCore` gained an `agent_defs` field (`Arc<RwLock<HashMap<String, AgentDef>>>`) populated during `discover_agents()`. The `AgentHandler::List` handler reads from this to return full metadata. The example was updated to call `discover_agents()` instead of manual registration.

Frontend: `client.submit()` now takes `target: Option<&str>`. `AgentsState` gained `selected: Option<String>`. `InputArea` reads agents from context and renders a `<select>` dropdown above the textarea. On submit, the selected agent ID is passed as the `target` parameter.

## Entities Mentioned

- [[vol-llm-agent-channel-crate]]: AgentServerCore agent_defs, discover_agents, agent.list enrichment
- [[vol-llm-ui-crate]]: Frontend agent selector and submit target param

## Concepts Covered

- [[agent-builder-pattern]]: AgentDef construction from YAML frontmatter files
