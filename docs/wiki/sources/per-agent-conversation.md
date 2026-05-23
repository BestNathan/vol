---
type: source
source_type: code
date: 2026-05-23
ingested: 2026-05-23
tags: [conversation, per-agent, state, ui]
---

# Per-Agent Conversation State

**Authors/Creators:** BestNathan
**Date:** 2026-05-23
**Link:** docs/superpowers/specs/2026-05-23-per-agent-conversation-design.md

## TL;DR

Replaced global `ConversationState` with a per-agent `HashMap<String, AgentConversation>`. Each agent now has independent conversation entries. Switching agents restores that agent's conversation. Events update the active agent's state. Resume stores entries under the correct agent key.

## Key Takeaways

- `ConversationState.entries` → `HashMap<String, AgentConversation>` keyed by agent_id
- `active_agent` field tracks which agent is currently viewed
- `reduce_conversation` routes events through `s.active_mut()` 
- `ConversationView` reads via `s.active_entries()`
- Agent card click sets `active_agent` alongside `selected`
- Session resume stores entries via `s.get_or_create(&agent_id)`

## Entities Mentioned

- [[vol-llm-ui-crate]]: ConversationState, AgentsPanel, SessionsPanel, ConversationView changes

## Concepts Covered

- [[agent-centric-ui]]: conversation state now scoped per-agent
