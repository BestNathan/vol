# Design Spec: Agent Manager Loader Integration

## Background

`vol-agent-manager` currently manages runtime-connected agents via WebSocket. Agent definitions (`.agents/agents/*.md` frontmatter files) are discovered by `AgentLoader` in `vol-llm-agent` but unused by the manager. This design adds file-based agent discovery and dynamic WS routing to agent instances.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        vol-agent-manager                         │
│                                                                  │
│  ┌──────────────┐  ┌──────────────┐  ┌───────────────────────┐  │
│  │ WS Router    │  │ Instance     │  │ AgentLoader           │  │
│  │ /ws/agents/  │◄─│ Registry     │◄─│ (from vol-llm-agent)  │  │
│  │ :type/...    │  │              │  │ .agents/agents/*.md   │  │
│  └──────▲───────┘  └──────┬───────┘  └───────────────────────┘  │
│         │                 │                                      │
│  ┌──────▼───────┐  ┌──────▼───────┐  ┌───────────────────────┐  │
│  │ WS Handler   │  │ ReActAgent   │  │ FileSessionEntryStore │  │
│  │ (broadcast)  │──│ instances    │──│ {type}/{sid}.jsonl    │  │
│  └──────────────┘  └──────────────┘  └───────────────────────┘  │
│                                                                  │
│  HTTP API: /api/v1/agent-types, /api/v1/agent-instances          │
└─────────────────────────────────────────────────────────────────┘
```

## Components

### 1. WS Router (new: `ws/router.rs`)

New WebSocket route: `/ws/agents/:agent_type/session/:session_id`

- Parses path segments to extract `agent_type` and `session_id`
- Validates `agent_type` exists in AgentLoader definitions
- Rejects unknown types with WS close code 4004
- Delegates to WS handler for message processing

```
GET /ws/agents/qa/session/abc-123
```

### 2. Instance Registry (new: `instance.rs`)

```rust
pub struct AgentInstance {
    pub agent_type: String,
    pub session_id: String,
    pub parent_session_id: Option<String>,
    pub session: Arc<Session>,
    pub status: InstanceStatus,
    pub created_at: DateTime<Utc>,
    pub ws_connections: HashSet<ConnectionId>,
}

pub enum InstanceStatus {
    Running,
    Completed,
    Failed,
}
```

Registry interface:

```rust
impl AgentInstanceRegistry {
    async fn get_or_create(&self, agent_type: &str, session_id: &str, parent_id: Option<&str>) -> Result<Arc<AgentInstance>>;
    async fn disconnect(&self, agent_type: &str, session_id: &str, conn_id: ConnectionId);
    async fn list_instances(&self) -> Vec<AgentInstanceSummary>;
    async fn destroy(&self, agent_type: &str, session_id: &str);
}
```

### 3. Instance Lifecycle

| Event | Behavior |
|---|---|
| First WS connect to `(type, sid)` | Create new session + ReActAgent instance |
| Subsequent WS connect to same `(type, sid)` | Add connection to existing instance |
| WS client sends message | Forward to agent, broadcast response to all connections |
| WS client disconnect | Remove connection; agent continues running |
| Session deleted | Destroy instance (lifecycle tied to session) |

### 4. Agent Instantiation Flow

1. Load `AgentDef` from AgentLoader by type
2. Create `FileSessionEntryStore` with agent type subdirectory
3. Create or resume `Session`
4. Build `ReActAgent` from definition (system prompt = content, tools, max_iterations, model override)
5. Run agent in background task with broadcast observer
6. Register instance in registry

### 5. FileSessionEntryStore Enhancement

**New constructor:**
```rust
impl FileSessionEntryStore {
    pub fn new<P: AsRef<Path>>(entry_dir: P) -> Self  // existing
    pub fn with_agent_type<P: AsRef<Path>>(entry_dir: P, agent_type: Option<String>) -> Self  // new
}
```

**Path resolution:**
- `agent_type: None` → `{entry_dir}/{session_id}.jsonl` (backward compatible)
- `agent_type: Some("qa")` → `{entry_dir}/qa/{session_id}.jsonl`
- Subdirectory auto-created on first `save()`
- `list_sessions()` scoped to current subdirectory

### 6. WS Message Protocol

Existing `WsMessage` envelope reused. New message types:

| message_type | Direction | Payload |
|---|---|---|
| `user_input` | Client→Agent | `{"content": "..."}` |
| `agent_event` | Agent→Client (broadcast) | `{"event": "thinking|tool_call|tool_result|answer", "data": {...}}` |
| `agent_complete` | Agent→Client (broadcast) | `{"result": "..."}` |
| `agent_error` | Agent→Client (broadcast) | `{"error": "..."}` |

### 7. REST API Additions

| Method | Path | Description |
|---|---|---|
| GET | `/api/v1/agent-types` | List discovered agent definitions (name, type, description, scope) |
| GET | `/api/v1/agent-instances` | List running instances (agent_type, session_id, status, connections, created_at) |
| DELETE | `/api/v1/agent-instances/:type/:session_id` | Destroy instance + delete session |

Existing endpoints unchanged.

### 8. Broadcast Mechanism

Agent output streamed to all connected WS clients via `broadcast::Sender<WsMessage>`. Each connection holds a `broadcast::Receiver`, independently reading from the same channel.

## Error Handling

| Scenario | Behavior |
|---|---|
| Unknown `agent_type` in WS path | WS close with code 4004, reason "agent type not found" |
| AgentLoader discover failure | Log warning, continue with empty definitions (no crash) |
| Agent instantiation failure | Return WS error message, do not register instance |
| WS broadcast channel full | Drop oldest message (bounded channel with overflow handling) |
| Agent panic/panic recovery | Instance removed from registry; next connection creates fresh instance |

## Dependencies

| Crate | Purpose |
|---|---|
| `vol-llm-agent` | AgentLoader, AgentDef, ReActAgent |
| `vol-session` | Session lifecycle (with agent_type store enhancement) |
| `vol-llm-core` | Message, AgentStreamEvent |
| `tokio` | Async runtime, broadcast channel |
| `axum` | HTTP + WebSocket server |

## Crate Structure

```
crates/vol-agent-manager/
├── src/
│   ├── main.rs              # Updated: add new routes
│   ├── lib.rs               # Updated: AppRouterState + loader
│   ├── instance.rs          # NEW: AgentInstance, AgentInstanceRegistry
│   └── ws/
│       ├── mod.rs           # Updated: export router
│       ├── server.rs        # Updated: add /ws/agents/:type/session/:id route
│       ├── router.rs        # NEW: path parsing, type validation
│       ├── handler.rs       # Updated: broadcast + agent instance handling
│       └── protocol.rs      # Updated: new message types
├── tests/
│   └── integration.rs       # Updated: WS agent routing tests
```

## Testing Strategy

- Unit tests for path parsing and type validation
- Unit tests for instance registry (create, connect, disconnect, destroy)
- Integration test: WS connect → agent instantiation → message exchange → disconnect
- Integration test: multiple WS clients to same instance receive broadcast
- Unit tests for FileSessionEntryStore with agent_type subdirectory
- Existing tests pass unchanged (backward compatible)

## Out of Scope (MVP)

- Agent-to-agent direct communication
- Persistent state beyond session JSONL
- Multi-instance HA
- Dashboard UI
- Role-based access control
- Rate limiting per agent instance
