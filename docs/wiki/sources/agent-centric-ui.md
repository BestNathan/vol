---
type: source
source_type: code
date: 2026-05-23
ingested: 2026-05-23
tags: [ui, agent, protocol, agent-centric, redesign]
---

# Agent-Centric UI + Protocol

**Authors/Creators:** BestNathan
**Date:** 2026-05-23
**Link:** docs/superpowers/specs/2026-05-23-agent-centric-ui-design.md

## TL;DR

Restructured UI around agents. Moved Agents tab to first position. Removed Conversation and Sessions tabs from the tab bar — now embedded as sub-tabs inside the Agents panel, scoped to the selected agent. Backend gained agent status tracking, agent_id-filtered session listing, and enriched agent.list with status/current_input.

## Key Takeaways

- Tab bar simplified to 6 tabs: Agents, Tools, Workspace, Skills, MCP, Logs
- Conversation and Sessions are now sub-tabs inside Agents panel, scoped to selected agent
- Agent cards show live status (idle/running) and current task
- `session.list` accepts optional `agent_id` parameter for filtering
- `agent.list` returns `status` and `current_input` per agent
- `AgentServerCore` tracks agent status via `ConnectionHolder` event intercepts

## Detailed Summary

Protocol: `SessionPayload::List` gained `agent_id: Option<String>`. `AgentServerCore` got `agent_status` field tracked by `ConnectionHolder::listen()` on AgentStart/AgentComplete events. `AgentHandler::List` reads status and current_input from the status map.

Frontend: `ActiveTab` removed `Conversation`/`Sessions` variants. `AgentsPanel` rewritten with card grid (status dot, name, type, description), sub-tab bar with Conversation/Sessions, embedded `ConversationView` and `SessionsPanel` accepting `agent_id`. `InputArea` moved into the agents panel. `client.session_list()` accepts optional `agent_id`.

## Entities Mentioned

- [[vol-llm-agent-channel-crate]]: session.list agent_id, agent status tracking, agent.list enrichment
- [[vol-llm-ui-crate]]: agent-centric layout, agents panel rewrite

## Concepts Covered

- [[agent-server-protocol]]: session.list with agent_id filtering
