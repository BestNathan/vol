# Agents Panel Design Spec

## Goal

Add an "Agents" tab to the web UI that lists all registered agents with expandable detail views showing their definitions.

## Architecture

### Backend: `agent.list` JSON-RPC Method

Add a new `agent.list` method to the JSON-RPC server in `vol-llm-agent-channel`. This method returns metadata for all registered agents.

**Request:** `{"jsonrpc":"2.0","method":"agent.list","id":1}`

**Response:** `{"jsonrpc":"2.0","id":1,"result":{"agents":[{"id":"repo:test-runner","name":"Test Runner","type":"test-runner","description":"Runs tests","scope":"Repo"},...]}}`

### Frontend: New AgentsPanel Component

A new panel component rendered when `ActiveTab::Agents` is selected. Uses request/response pattern (no EventBus subscription) to fetch agent list on mount and on refresh.

## Backend Changes

### 1. `JsonRpcRequest` enum (serde_helpers.rs)

Add `AgentList { id: u64 }` variant.

### 2. Parser (serde_helpers.rs)

Parse `"agent.list"` method into `JsonRpcRequest::AgentList`.

### 3. `JsonRpcConnection` (connection.rs)

Add `handle_agent_list()` method and wire it into the dispatch match.

The method reads `self.dispatchers.keys()` for registered agent IDs. Since the current example (`jsonrpc_agent_service.rs`) only registers one agent by hardcoded ID `"general-assistant"`, the response will include that ID. For richer metadata (name, description), we can add an `agent_metadata: HashMap<String, AgentMetadata>` field to `JsonRpcServer` or derive metadata from the dispatcher/holder info.

**Pragmatic choice:** Return `id`, `name` (derived from id), and a placeholder description for now. When `AgentLoader` integration lands, the metadata will be populated. The frontend should handle partial data gracefully.

**Response payload fields:**
- `id` — agent dispatch key (e.g., `"general-assistant"`)
- `name` — display name
- `description` — short description
- `type` — dispatch type (same as name)
- `scope` — `"Server"` for now (no file-based discovery via RPC yet)

### 4. RemoteConnection (vol-llm-ui/src/web/client.rs)

Add `agent_list()` method to `JsonRpcClient` that sends `agent.list` and parses the response into `Vec<AgentListEntry>`.

## Frontend Changes

### 1. `ActiveTab` enum (state/mod.rs)

Add `Agents` variant. Update `toggle()` method.

### 2. `AgentsState` struct (state/mod.rs)

```rust
pub struct AgentListEntry {
    pub id: String,
    pub name: String,
    pub r#type: String,
    pub description: String,
    pub scope: String,
}

pub struct AgentsState {
    pub agents: Vec<AgentListEntry>,
    pub expanded: HashSet<usize>,
    pub loading: bool,
    pub error: Option<String>,
}
```

### 3. `AgentsPanel` component (web/components/agents_panel.rs)

New component following existing panel patterns:
- On mount: set loading=true, call `rpc.agent_list()`, set agents or error
- Render: list of `AgentItem` rows with chevron, name, description, scope badge
- Click: toggle expanded state for that agent
- Refresh button on the panel header

### 4. TabBar (web/components/app.rs)

Add `TabButton { tab: ActiveTab::Agents, label: "Agents" }`.

### 5. TabContent (web/components/app.rs)

Add `ActiveTab::Agents => rsx! { AgentsPanel {} }` match arm.

## UI Layout

```
Agents Panel
┌─────────────────────────────────────────────┐
│ Agents                           [refresh]  │
├─────────────────────────────────────────────┤
│ > general-assistant                         │
│   Code assistant with bash/edit tools       │
│   [Server]                                  │
│                                             │
│   ┌─ Expanded Detail ──────────────────┐   │
│   │ Type: general-assistant            │   │
│   │ Scope: Server                      │   │
│   │ Model: default                     │   │
│   │ Tools: Bash, Edit, Read, ...       │   │
│   └────────────────────────────────────┘   │
│ > test-runner                               │
│   Runs test suites                          │
│   [Repo]                                    │
└─────────────────────────────────────────────┘
```

Scope badges: Server=#c0c040, Repo=#4080ff, User=#40c040

## Error Handling

- Empty list: show "No agents discovered" message
- RPC error: show error message with retry button
- Loading: show "Loading agents..." placeholder

## Testing

- Backend: unit test `handle_agent_list()` returns correct format
- Frontend: unit test agent list parsing, serialization
- Integration: manual verification via browser

## Files to Modify

- `crates/vol-llm-agent-channel/src/jsonrpc/serde_helpers.rs` — add AgentList variant + parser
- `crates/vol-llm-agent-channel/src/jsonrpc/connection.rs` — add handle_agent_list
- `crates/vol-llm-ui/src/state/mod.rs` — add ActiveTab::Agents, AgentsState, AgentListEntry
- `crates/vol-llm-ui/src/web/client.rs` — add agent_list() method
- `crates/vol-llm-ui/src/web/components/app.rs` — add Agents tab button + route
- `crates/vol-llm-ui/src/web/components/agents_panel.rs` — new component
