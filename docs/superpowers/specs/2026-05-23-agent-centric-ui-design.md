# Design: Agent-Centric UI + Protocol

## Summary

Restructure UI around agents. Move Agents tab to first position. Remove Conversation and Sessions tabs from the tab bar — they become sub-tabs inside the Agents panel, scoped to the selected agent. Input area also moves into Agents panel. Agent cards show live status and current task. Protocol gains `agent_id` filtering on session operations and status fields on `agent.list`.

## UI Layout

### Tab bar

```
[Agents] [Tools] [Workspace] [Skills] [MCP] [Logs]
```

Conversation and Sessions tabs removed from the bar.

### Agents panel

```
┌──────────────────────────────────────────────────┐
│ ┌──────────┐ ┌──────────┐ ┌──────────┐          │
│ │ ○ gen-pur│ │ ● explore│ │ ○ review │          │
│ │   idle   │ │ running  │ │   idle   │          │
│ └──────────┘ └──────────┘ └──────────┘          │
│                                                  │
│ ▸ explore — running: "find the auth middleware"  │
│                                                  │
│ [Conversation] [Sessions]                        │
│ ┌──────────────────────────────────────────────┐ │
│ │  >>> Find the auth middleware                 │ │
│ │  ...agent response...                         │ │
│ └──────────────────────────────────────────────┘ │
│ ┌──────────────────────────────────────────────┐ │
│ │ Type a message...                    [Send]   │ │
│ └──────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────┘
```

- **Top**: agent card grid. Each card shows status dot (green=idle, yellow=running), name, type. Click to select.
- **Selected agent bar**: name, type, description, current task (if running).
- **Sub-tabs**: Conversation and Sessions — scoped to selected agent.
- **Bottom**: input area — talks to selected agent.

## Protocol changes

### `agent.list` — add status fields

Response adds `status` and `current_input` per agent:

```json
{
  "agents": [{
    "id": "explore",
    "name": "explore",
    "type": "explore",
    "description": "...",
    "scope": "repo",
    "status": "running",
    "current_input": "find the auth middleware"
  }]
}
```

### `session.list` — add agent_id filter

`SessionPayload::List` gains optional `agent_id` field:
```rust
List {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    agent_id: Option<String>,
},
```

Wire format:
```json
{"method":"session.list","params":{"agent_id":"explore"}}
```

When `agent_id` is `None`, returns all sessions (backward compat).

### `session.entries` / `session.resume` — already have agent_id

Both already accept `agent_id: Option<String>` in their payloads. Frontend passes selected agent ID.

## Backend

### `AgentServerCore` — track agent status

Add status tracking alongside agent_defs:

```rust
agent_status: Arc<RwLock<HashMap<String, AgentStatus>>>,

struct AgentStatus {
    status: String,        // "idle" | "running"
    current_input: Option<String>,
    run_id: Option<String>,
}
```

Updated by `ConnectionHolder::listen()` on `AgentStart` / `AgentComplete` events. `AgentHandler::List` reads status from here.

### `SessionHandler::List` — agent_id filter

If `agent_id` param provided, only scan that agent's sessions directory. Otherwise scan all (current behavior).

## Frontend

### State changes

- `AppState`: add `selected_agent: Signal<Option<String>>`
- Remove `ActiveTab::Conversation` and `ActiveTab::Sessions`
- Add `ActiveTab::Agents` as first position; renumber others
- `AgentsState`: track `selected: Option<String>` + per-agent status

### Component changes

| Component | Change |
|-----------|--------|
| `AgentsPanel` | Rewrite — card grid, sub-tabs, embedded Conversation + Sessions + InputArea |
| `SessionsPanel` | Accept `agent_id` prop, pass to session.list |
| `InputArea` | Accept `selected_agent` prop, pass as submit target |
| `app.rs` | Remove Conversation/Sessions tabs, reorder tab bar |
| `conversation.rs` | Accept `agent_id` prop (for future scoping) |

### Removed tabs

`ActiveTab::Conversation` and `ActiveTab::Sessions` enum variants removed. Their content now lives inside `AgentsPanel` as sub-tabs (not routed through TabContent).

## Files

| File | Change |
|------|--------|
| `src/state/mod.rs` | Remove Conversation/Sessions from ActiveTab; add selected_agent to AppState; add AgentStatusInfo |
| `src/web/components/app.rs` | New tab bar order; remove old tab routing; wire selected_agent |
| `src/web/components/agents_panel.rs` | Rewrite with cards + sub-tabs + embedded conversation + input |
| `src/web/components/sessions_panel.rs` | Accept agent_id param |
| `src/web/components/input_area.rs` | Move into agents panel; accept target prop |
| `src/domain/agent.rs` | Return status/current_input in agent.list |
| `src/domain/session.rs` | Filter by agent_id in session.list |
| `src/server_core.rs` | Add agent_status tracking |
| `src/agent_server_protocol.rs` | Add agent_id to SessionPayload::List |
