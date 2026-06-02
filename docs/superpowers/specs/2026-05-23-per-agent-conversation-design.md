# Design: Per-Agent Conversation State

## Summary

Replace the single global `ConversationState` with a per-agent map. Each agent has independent conversation entries. Switching agents loads that agent's saved state. Events update the running agent's state. Resume restores entries into the correct agent's state.

## Root cause

`ConversationState` is a single global signal. All agents share `entries`. Switching agents shows the same conversation. Resume loads into global state, not the agent's state.

## Solution

### State changes

```rust
pub struct ConversationState {
    pub agents: HashMap<String, AgentConversation>,
    pub active_agent: Option<String>,
}

pub struct AgentConversation {
    pub entries: Vec<ConversationEntry>,
    pub auto_scroll: bool,
}
```

Key behaviors:
- `agent_conversation(agent_id)` returns or creates the agent's conversation
- `current_mut()` returns the active agent's mutable conversation
- `current_entries()` returns the active agent's entries

### Event routing

Events currently update `ConversationState.entries` globally. After the change, events update `agents[running_agent_id].entries`. The running agent is tracked via the `target` parameter of `agent.submit`.

When `AgentStart` fires, the conversation for that agent is reset and the user input is added. When `ContentDelta`/`ContentComplete` fire, they append to that agent's entries.

### Agent switch

When the user clicks an agent card:
1. Current agent's entries are preserved in `agents[old_agent_id]`
2. `active_agent` is set to the new agent_id
3. `ConversationView` reads from `agents[new_agent_id].entries`
4. If the new agent has no entries, show the "No messages yet" placeholder

### Resume

When a session is resumed for agent X:
1. `conv.with_mut(|s| { s.agents[agent_id].entries = conv_entries; })` — entries go to the correct agent
2. Sub-tab switches to Conversation
3. `ConversationView` shows the restored entries for agent X

## Files

| File | Change |
|------|--------|
| `state/mod.rs` | Rewrite `ConversationState` as per-agent map; add `AgentConversation`; update `apply()` and helpers |
| `web/components/app.rs` | Event processing reads `active_agent` to route to correct agent |
| `web/components/conversation.rs` | Read from `agents[active_agent]` instead of global entries |
| `web/components/agents_panel.rs` | On agent switch, set `active_agent` |
| `web/components/sessions_panel.rs` | Resume stores entries under correct agent_id |
