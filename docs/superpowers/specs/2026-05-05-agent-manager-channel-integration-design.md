# Design Spec: vol-agent-manager Channel Integration

## Background

`vol-agent-manager` and `vol-llm-agent-channel` are currently independent crates with zero dependency relationship. `vol-agent-manager` implements its own WebSocket protocol, handler logic, and task tracking. `vol-llm-agent-channel` provides a channel abstraction with `Connection` trait, message protocols (`InboundMessage`/`OutboundMessage`), and dispatchers.

The two crates have redundant communication logic that needs to be consolidated. The architectural model follows K8s Master/Worker: manager is the centralized control plane, agents are workers, and channel is the communication protocol layer.

## Architecture

### Three-tier model

```
Frontend  ←→  vol-agent-manager (control plane)  ←→  vol-llm-agent-channel (protocol)  ←→  Agents
              (REST + SSE)
```

### Component positioning

| Component | Role | K8s Analogy |
|-----------|------|-------------|
| `vol-agent-manager` | Centralized control plane — agent CRUD, task CRUD, state tracking, health checks, metrics, gateway to frontend | Master (API Server + Scheduler + Controller) |
| `vol-llm-agent-channel` | Communication protocol layer — how agents talk to manager and to each other | Pod Network / kubelet protocol |
| Individual Agents | Workers that execute tasks and report status | Nodes / kubelets |

### Communication patterns

| Pattern | Direction | Transport | Description |
|---------|-----------|-----------|-------------|
| Control plane (agent → manager) | Agent → Manager | Manager WS endpoint via `Connection` | Registration, heartbeat, task results, events |
| Control plane (manager → agent) | Manager → Agent | Manager routes via `Connection` | Task dispatch, commands |
| Data plane (agent ↔ agent direct) | Agent → Agent | Direct channel connection | Business data exchange without manager involvement |
| Data plane (agent ↔ agent via manager) | Agent → Manager → Agent | Manager Router forwards | Manager-mediated agent communication |

## Design Decisions

### 1. Unified Message Type

Replace `InboundMessage`/`OutboundMessage` with a single `Message` enum in `vol-llm-agent-channel`. Direction is not encoded in the type name — each message carries `sender` and `receiver` fields that determine routing.

```rust
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    Submit {
        req_id: String,
        sender: String,
        receiver: String,
        input: String,
        metadata: Option<HashMap<String, serde_json::Value>>,
    },
    Cancel {
        req_id: String,
        sender: String,
        receiver: String,
    },
    Connected {
        sender: String,
        receiver: String,
    },
    Event {
        sender: String,
        receiver: String,
        event: serde_json::Value,
    },
    Result {
        req_id: String,
        sender: String,
        receiver: String,
        result: serde_json::Value,
    },
    Error {
        req_id: Option<String>,
        sender: String,
        receiver: String,
        message: String,
    },
}
```

The same message type can be both received and sent. An agent receiving `Message::Submit` executes the task; the same agent can send a `Message::Submit` to another agent. The `Connection` trait simplifies to `recv() -> Message` and `send(Message)`.

### 2. Protocol: Channel types exclusively

Delete `ws/protocol.rs` entirely from vol-agent-manager. All communication uses `vol-llm-agent-channel`'s unified `Message` type. Manager concepts (register, heartbeat, metric, task_result) are expressed through `Message::Submit` with metadata fields.

No adapter layer — manager adopts channel protocol directly.

### 3. Connection: Use the trait, not ConnectionHolder

The manager uses `Connection` trait directly for agent communication. `ConnectionHolder` is an `AgentPlugin` designed for `ReActAgent` lifecycle, which does not match the manager's external connection model.

The `Connection` trait is updated to use the unified `Message` type:

```rust
#[async_trait]
pub trait Connection: Send + Sync + 'static {
    fn protocol(&self) -> &str;
    async fn recv(&mut self) -> Option<Result<Message, ConnectionError>>;
    async fn send(&self, msg: Message) -> Result<(), ConnectionError>;
}
```

The manager's `ws/handler.rs` will:
- Receive raw axum `WebSocket` from the server
- Wrap it in a `WsConnection` implementing `Connection`
- Use `Connection::recv()` to read `Message`
- Use `Connection::send(msg)` to write `Message`

### 4. Routing: Keep ws/server.rs in manager

Routing configuration (`/ws` endpoint, query params, auth) remains the manager's responsibility. The server creates the `WsConnection` wrapper and delegates to the handler.

### 5. TaskDispatcher: Keep in manager

`TaskDispatcher` in manager tracks multi-agent task state at the management level. This is a control plane concern, distinct from `AgentDispatcher`'s single-agent request queue (data plane). No redundancy here.

### 6. Frontend gateway: Manager serves as gateway

Manager exposes REST API + SSE for frontend consumption. The SSE (`EventBus`) remains in manager's `events/` module. Frontend does not directly interact with channel — all agent data flows through manager.

## Module Changes

### vol-agent-manager

**Files to delete:**
- `src/ws/protocol.rs` — all message types removed

**Files to modify:**
- `Cargo.toml` — add `vol-llm-agent-channel` dependency
- `src/ws/mod.rs` — update exports, remove protocol module
- `src/ws/handler.rs` — rewrite to use `Connection` trait and channel protocol types
- `src/ws/server.rs` — minor changes to wire up `WsConnection`
- `src/lib.rs` / `src/main.rs` — update imports

**Files unchanged:**
- `src/state/` — agent state management
- `src/metrics/` — metrics collection
- `src/health/` — health checking
- `src/events/` — SSE event bus for frontend
- `src/task/` — task dispatcher (control plane)
- `src/config.rs` — configuration

### vol-llm-agent-channel

**Files to modify:**
- `src/protocol.rs` — replace `InboundMessage`/`OutboundMessage` with unified `Message` enum, add `sender`/`receiver` fields to all variants
- `src/connection.rs` — update `Connection` trait: `recv() -> Message`, `send(Message)`, remove `send_event()`/`send_result()`
- `src/dispatcher.rs` — update to use unified `Message`
- `src/router.rs` — update to use unified `Message`
- `src/transport/ws.rs` — update WebSocket transport to handle unified `Message`
- `src/transport/memory.rs` — update memory transport to handle unified `Message`
- `src/lib.rs` — update public exports

## Data Flow

### Agent registration and control plane

```
Agent                          Manager                           Channel
  │                              │                                 │
  │── WS connect ───────────────►│                                 │
  │                              │── wrap in WsConnection ────────►│
  │── Message::Submit ──────────►│                                 │
  │   (sender=agent,             │                                 │
  │    metadata: type=register)  │                                 │
  │                              │── parse, register agent ───────►│
  │                              │── update state manager          │
  │                              │── emit SSE event                │
  │                              │                                 │
  │◄── Message::Connected ──────│                                 │
  │   (sender=manager,           │                                 │
  │    receiver=agent)           │                                 │
  │                              │                                 │
  │── Message::Submit ──────────►│                                 │
  │   (metadata: type=heartbeat) │                                 │
  │                              │── update heartbeat              │
  │                              │── update metrics                │
  │                              │                                 │
  │── Message::Submit ──────────►│                                 │
  │   (metadata: type=task_result)                                │
  │                              │── complete task in dispatcher   │
  │                              │── emit SSE event                │
  │                              │                                 │
```

### Task dispatch (manager → agent)

```
Frontend                       Manager                           Agent
  │                              │                                 │
  │── POST /tasks ──────────────►│                                 │
  │                              │── create task (TaskDispatcher)  │
  │                              │── Message::Submit ─────────────►│
  │                              │   (sender=manager,              │
  │    receiver=agent,           │                                 │
  │    task details)             │                                 │
  │                              │                                 │
  │◄── SSE event: task_dispatched│                                 │
  │                              │                                 │
  │                              │◄── Message::Result ────────────│
  │                              │   (sender=agent,               │
  │    receiver=manager)         │                                 │
  │◄── SSE event: task_completed │                                 │
```

### Agent-to-agent communication (direct)

```
Agent A                        Channel                         Agent B
  │                              │                                 │
  │── Connection::send ─────────►│── Message::Submit ─────────────►│
  │   (sender=agent_a,           │    (sender=agent_a,             │
  │    receiver=agent_b)         │     receiver=agent_b)           │
  │                              │                                 │
  │                              │◄── Message::Result ────────────│
  │                              │    (sender=agent_b,             │
  │     receiver=agent_a)       │                                 │
  │◄── Connection::recv ────────│                                 │
```

### Agent-to-agent communication (via manager)

```
Agent A                        Manager                           Agent B
  │                              │                                 │
  │── Message::Submit ──────────►│                                 │
  │   (sender=agent_a,           │                                 │
  │    receiver=agent_b)         │                                 │
  │                              │── Router lookup agent_b conn ──►│
  │                              │── forward Message::Submit ─────►│
  │                              │    (sender=agent_a,             │
  │     receiver=agent_b)        │                                 │
  │                              │                                 │
  │                              │◄── Message::Result ────────────│
  │                              │    (sender=agent_b,             │
  │     receiver=agent_a)        │                                 │
  │◄── Message::Result ──────────│                                 │
  │   (sender=agent_b,           │                                 │
  │    receiver=agent_a)         │                                 │
```

## Error Handling

- Connection failures: manager marks agent as `Disconnected` in state, emits SSE event
- Protocol errors (invalid Message): manager sends `Message::Error`, logs warning
- Task timeouts: `TaskDispatcher` marks task as `Timeout`, emits SSE event
- Agent crashes: health checker detects stale heartbeat, marks agent unhealthy

## Testing

- Unit tests: handler parsing of Message variants, state manager updates
- Integration tests: end-to-end agent registration via WebSocket, task dispatch flow
- Channel tests: update existing tests to use unified Message type

## Open Questions

None.
