# llm-ui Control Plane Dual-Connection Design

**Date**: 2026-07-21  
**Status**: draft  
**Author**: Claude + Nathan

## Motivation

`vol-llm-ui` currently assumes a single WebSocket connection to a fully-featured
data-plane. The control-plane (`agent-server` with `control_plane=true`) only
implements `agent.list`, `agent.status`, and `agent.submit` (routing). The UI
cannot subscribe to events, cancel runs, or use session/task/file/skill/MCP.
This spec adapts the UI to connect to the control-plane for management queries,
and to individual data-planes for agent interaction.

## Architecture

```
┌────────────────────────────────────────────────┐
│                  vol-llm-ui                     │
│                                                │
│  cp_client  ─────────────────→ agent-server    │
│  (管理查询, 常驻)                (control-plane)  │
│    node.list / agent.list                       │
│    capability.list / task.list                  │
│                                                │
│  dp_clients ─────────────────→ agent-server-dp │
│  (agent 交互, 按需)             agent-server-dingtalk
│    agent.submit / subscribe      agent-server-ansible
│    session.* / file.*            (data-planes)
│    skill.* / tool.* / mcp.*
└────────────────────────────────────────────────┘
```

## Control-Plane Changes

### agent.list — add `ws_url` field

Each agent entry must include a `ws_url` so the UI knows where to connect:

```json
{
  "agents": [{
    "id": "coding",
    "name": "Coding Agent",
    "description": "...",
    "status": "idle",
    "node_id": "dp-1",
    "ws_url": "wss://dp.vol.bestnathan.top/ws"
  }]
}
```

**`ws_url` source**: control-plane ConfigMap (`deploy/argocd/manifests/workloads/agent-server/configmap.yaml`)
with a static `node_id → ws_url` mapping. The `ClientHandler` reads it and
injects `ws_url` into `agent.list` responses. Static mapping is preferred
because ingress domains are static configuration, not node runtime state.

### task.list — cross-node aggregation

CP queries each connected data-plane node for its task list via the existing
node WebSocket connection, then merges results with `node_id` tags. This
requires adding `task.list` and `task.get` to the node-control protocol
(ControlOperation variants).

CP `task.list` returns aggregated results with origin `node_id`:

```json
{
  "tasks": [{
    "id": 1,
    "status": "in_progress",
    "subject": "Fix login bug",
    "node_id": "dp-1",
    ...
  }, {
    "id": 2,
    "status": "pending",
    "subject": "Deploy release",
    "node_id": "dingtalk",
    ...
  }]
}
```

`task.get` routes to the owning node based on `node_id` in the stored mapping.

### ConfigMap change

```toml
# agent-server.toml — new section
[node_ingress]
"dp-1" = "wss://dp.vol.bestnathan.top/ws"
"dingtalk" = "wss://dingtalk.vol.bestnathan.top/ws"
"ansible" = "wss://ansible.vol.bestnathan.top/ws"
```

Or as a separate `node_ingress` mapping in the existing ConfigMap YAML.

## New Ingress Manifests

| Data-plane node | Ingress domain |
|-----------------|---------------|
| `agent-server-dp` | `dp.vol.bestnathan.top` |
| `agent-server-dingtalk` | `dingtalk.vol.bestnathan.top` (existing) |
| `agent-server-ansible` | `ansible.vol.bestnathan.top` |

New files:
- `deploy/argocd/manifests/workloads/agent-server-dp/ingress.yaml`
- `deploy/argocd/manifests/workloads/agent-server-ansible/ingress.yaml`

Each ingress: Higress class, `/` → service port 3001, same annotations as existing.

## UI State Model Changes

### New types

```rust
// Per-node DP connection
struct DpConnection {
    client: JsonRpcClient,
    node_id: String,
    ws_url: String,
    agent_ids: Vec<String>,  // agents on this node
}

// AppState additions
struct AppState {
    cp_client: JsonRpcClient,                    // always connected
    dp_connections: Signal<HashMap<String, DpConnection>>,  // node_id → conn
    active_node_id: Signal<Option<String>>,       // current interaction target
    // ... existing fields
}
```

### Connection lifecycle

1. App mount → `cp_client` connects to `wss://cp.vol.bestnathan.top/ws`
2. CP `agent.list` returns agents with `node_id` + `ws_url`
3. User selects agent → UI checks `dp_connections[agent.node_id]`
   - Exists → activate it
   - Not exists → `JsonRpcClient::new(agent.ws_url)` → subscribe → store
4. DP connection auto-subscribes to `agent.event` on open (existing behavior)
5. DP connection idle → kept alive; close/reap on explicit disconnect or tab close

### Event routing

Each `DpConnection` has its own event stream. The WS event loop dispatches
events tagged with the source `node_id`. `ConversationState` already keys by
agent ID, so concurrent conversations across different nodes work naturally.

## UI Component Changes

### New: `NodesPanel` (tab: "Nodes")

Displays `node.list` results:
- Node name, status (online/offline), agent count, load
- Click to expand → capability snapshot (agents, tools, MCP servers, skills)

### Modified: `AgentsPanel`

- Each row shows `node_id` column
- Agents grouped by node
- Selecting an agent auto-creates/activates DP connection to its node

### Modified: `InputArea`

- On submit: look up agent's `node_id` → get `DpConnection` → `agent.submit` via DP
- If no DP connection exists yet, create one first

### Modified: `StatusBar`

- Left: CP connection status (always visible, green dot)
- Right: active DP node name + connection status

### Modified: TabBar

New order: `Nodes | Tasks | Agents | Tools | Workspace | Skills | MCP | Logs`

- `Nodes` → CP `node.list` / `capability.list`
- `Tasks` → CP `task.list` (cross-node aggregation)
- `Agents` → CP `agent.list` (cross-node, with node_id grouping)
- `Tools/Skills/MCP` → DP connection, show "Select an agent first" when no active node

## Data Flow

### Agent submission
```
1. User picks agent → AgentsPanel sets active agent
2. DpConnection for agent.node_id is activated (created if needed)
3. User types input → InputArea calls dp_client.submit(input, target=agent.id)
4. DP sends agent.event notifications → WS event loop → UiEvent → ConversationState
5. Streamed output renders in ConversationView
```

### Task management
```
1. TasksPanel calls cp_client.task_list()
2. CP aggregates tasks from all registered nodes (or fetches from data-plane)
3. TasksPanel renders with node_id column
4. task.get → CP routes to the node that owns the task
```

## Testing

- `JsonRpcClient`: existing unit tests continue to pass (single-connection model
  unchanged)
- New `DpConnection` management: unit tests for activate/deactivate/reuse logic
- `AppState`: test that `dp_connections` correctly caches by `node_id`
- Integration: CP `agent.list` returns `ws_url` field; UI creates DP connection

## Non-Goals

- Control-plane event relay (run_id → client_conn mapping): not needed in this
  design because all streaming events go DP→UI directly
- `agent.subscribe` on CP: not needed
- Client-side load balancing: the CP `agent.list` + `ws_url` already tells
  the UI which specific node to use

## Deployment

- `vol-llm-ui-cp` deployment (from previous work) will serve this updated UI
- New ingress manifests for `agent-server-dp` and `agent-server-ansible` added
  to `deploy/argocd/manifests/workloads/`
- CP ConfigMap updated with `node_ingress` mapping
