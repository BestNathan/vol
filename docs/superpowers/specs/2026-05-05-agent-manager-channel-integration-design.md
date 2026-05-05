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

### 1. Protocol: Channel types exclusively

Delete `ws/protocol.rs` entirely from vol-agent-manager. All communication uses `vol-llm-agent-channel`'s protocol types:

- `InboundMessage` (Submit/Cancel) — the only incoming message type
- `OutboundMessage` (Connected/Event/Result/Error) — the only outgoing message type
- Manager concepts (register, heartbeat, metric, task_result) are expressed through `InboundMessage::Submit` with metadata fields

No adapter layer — manager adopts channel protocol directly.

### 2. Connection: Use the trait, not ConnectionHolder

The manager uses `Connection` trait directly for agent communication. `ConnectionHolder` is an `AgentPlugin` designed for `ReActAgent` lifecycle, which does not match the manager's external connection model.

The manager's `ws/handler.rs` will:
- Receive raw axum `WebSocket` from the server
- Wrap it in a `WsConnection` (from vol-llm-agent-channel)
- Use `Connection::recv()` to read `InboundMessage`
- Use `Connection::send_event()` / `Connection::send_result()` to write `OutboundMessage`

### 3. Routing: Keep ws/server.rs in manager

Routing configuration (`/ws` endpoint, query params, auth) remains the manager's responsibility. The server creates the `WsConnection` wrapper and delegates to the handler.

### 4. TaskDispatcher: Keep in manager

`TaskDispatcher` in manager tracks multi-agent task state at the management level. This is a control plane concern, distinct from `AgentDispatcher`'s single-agent request queue (data plane). No redundancy here.

### 5. Frontend gateway: Manager serves as gateway

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

No changes expected. The channel crate's API remains stable.

## Data Flow

### Agent registration and control plane

```
Agent                          Manager                           Channel
  │                              │                                 │
  │── WS connect ───────────────►│                                 │
  │                              │── wrap in WsConnection ────────►│
  │── InboundMessage::Submit ───►│                                 │
  │   (metadata: type=register)  │                                 │
  │                              │── parse, register agent ───────►│
  │                              │── update state manager          │
  │                              │── emit SSE event                │
  │                              │                                 │
  │◄── OutboundMessage::Connected│                                 │
  │                              │                                 │
  │── InboundMessage::Submit ───►│                                 │
  │   (metadata: type=heartbeat) │                                 │
  │                              │── update heartbeat              │
  │                              │── update metrics                │
  │                              │                                 │
  │── InboundMessage::Submit ───►│                                 │
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
  │                              │── InboundMessage::Submit ──────►│
  │                              │   (task details)                │
  │                              │                                 │
  │◄── SSE event: task_dispatched│                                 │
  │                              │                                 │
  │                              │◄── OutboundMessage::Result ────│
  │                              │                                 │
  │◄── SSE event: task_completed │                                 │
```

### Agent-to-agent communication (direct)

```
Agent A                        Channel                         Agent B
  │                              │                                 │
  │── Connection::send ─────────►│── InboundMessage::Submit ──────►│
  │                              │                                 │
  │                              │◄── OutboundMessage::Result ────│
  │◄── Connection::recv ────────│                                 │
```

### Agent-to-agent communication (via manager)

```
Agent A                        Manager                           Agent B
  │                              │                                 │
  │── InboundMessage::Submit ───►│                                 │
  │   (target: agent_b)          │                                 │
  │                              │── Router lookup agent_b conn ──►│
  │                              │── forward Submit ──────────────►│
  │                              │                                 │
  │                              │◄── OutboundMessage::Result ────│
  │◄── OutboundMessage::Result ──│                                 │
```

## Error Handling

- Connection failures: manager marks agent as `Disconnected` in state, emits SSE event
- Protocol errors (invalid InboundMessage): manager sends `OutboundMessage::Error`, logs warning
- Task timeouts: `TaskDispatcher` marks task as `Timeout`, emits SSE event
- Agent crashes: health checker detects stale heartbeat, marks agent unhealthy

## Testing

- Unit tests: handler parsing of InboundMessage variants, state manager updates
- Integration tests: end-to-end agent registration via WebSocket, task dispatch flow
- No changes to vol-llm-agent-channel's existing tests

## Open Questions

None.
