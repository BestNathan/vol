# Design Spec: Agent Manager Control Plane

## Background

Existing ReActAgent system runs on individual hosts with no centralized management. A control plane service is needed to manage agents across multiple hosts, monitor their status and metrics, and provide reporting and task dispatch capabilities.

## Architecture

Control plane runs as a standalone service (`vol-agent-manager` crate). Agents (data plane) connect via WebSocket, register themselves, and maintain persistent connections. The control plane provides REST APIs for management and Prometheus `/metrics` endpoint for observability.

```
┌─────────────────────────────────────────────────────┐
│                   vol-agent-manager                  │
│                                                     │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────┐ │
│  │ WebSocket    │  │ Agent State  │  │ Prometheus │ │
│  │ Server (axum)│◄─┤ Manager      │  │ /metrics   │ │
│  └──────▲───────┘  └──────┬───────┘  └────────────┘ │
│         │                 │                           │
│  ┌──────▼───────┐  ┌──────▼───────┐  ┌────────────┐ │
│  │ Command      │  │ Health       │  │ SSE Event  │ │
│  │ Dispatcher   │  │ Checker      │  │ Stream     │ │
│  └──────────────┘  └──────────────┘  └────────────┘ │
│                                                     │
│  HTTP API: /api/v1/agents/* /api/v1/tasks/*         │
└─────────────────────────────────────────────────────┘
         ▲                   ▲
         │ WebSocket         │ WebSocket
         ▼                   ▼
  ┌─────────────┐      ┌─────────────┐
  │ Agent Node  │      │ Agent Node  │
  │ (data plane)│      │ (data plane)│
  └─────────────┘      └─────────────┘
```

## Components

### 1. WebSocket Server

axum-based WebSocket server on configurable port (default 8080). Handles:
- Connection upgrade with optional token auth (`?token=xxx`)
- Message routing to appropriate handlers
- Connection lifecycle (open, close, error)

### 2. Message Protocol

JSON over WebSocket with unified envelope:

```json
// Agent → Control (reporting)
{
  "message_type": "register" | "heartbeat" | "metric" | "event" | "task_result",
  "agent_id": "agent-node-01",
  "timestamp": "2026-05-02T12:00:00Z",
  "payload": {}
}

// Control → Agent (commands)
{
  "message_type": "task" | "health_check" | "config_update",
  "task_id": "task-abc123",
  "target_agent_id": "agent-node-01",
  "payload": {}
}
```

Payload schemas:

| message_type | Direction | Payload fields |
|---|---|---|
| `register` | Agent→Ctrl | `name`, `type`, `version`, `capabilities[]`, `host_info` |
| `register_ack` | Ctrl→Agent | `agent_id`, `status` |
| `heartbeat` | Agent→Ctrl | `status` (Idle/Busy), `load` (optional) |
| `metric` | Agent→Ctrl | `samples[]` (each: `name`, `value`, `labels`, `timestamp`) |
| `event` | Agent→Ctrl | `run_id`, `event_name`, `severity`, `data` |
| `task` | Ctrl→Agent | `task_type`, `parameters`, `timeout_seconds` |
| `task_result` | Agent→Ctrl | `status` (Completed/Failed), `result`, `error`, `duration_ms` |
| `health_check` | Ctrl→Agent | empty |
| `config_update` | Ctrl→Agent | `config_key`, `config_value` |

### 3. Agent State Manager

Thread-safe state store (RwLock<AgentId, AgentState>):

```rust
struct AgentState {
    agent_id: String,
    name: String,
    r#type: String,
    version: String,
    capabilities: Vec<String>,
    host_info: HostInfo,
    status: AgentStatus,
    connected_at: DateTime<Utc>,
    last_heartbeat: DateTime<Utc>,
}

enum AgentStatus {
    Connected,     // WebSocket connected, registered
    Idle,          // No active task
    Busy,          // Running a task
    Disconnected,  // WebSocket closed unexpectedly
    Dead,          // Heartbeat timeout
}

struct HostInfo {
    hostname: String,
    os: String,
    arch: String,
    ip: String,
}
```

### 4. Prometheus Metrics Collector

Using `prometheus` crate, exposes via `/metrics` endpoint:

| Metric | Type | Labels |
|---|---|---|
| `agent_connections_current` | Gauge | none |
| `agent_registered_total` | Gauge | none |
| `agent_heartbeat_latency_seconds` | Histogram | `agent_id`, `type` |
| `agent_messages_total` | Counter | `message_type`, `agent_id`, `type` |
| `agent_task_duration_seconds` | Histogram | `task_type`, `agent_id`, `status` |
| `agent_status_count` | Gauge | `status` |
| `agent_metric_samples_total` | Counter | `agent_id` |

### 5. Health Checker

Periodic background task scanning agent heartbeats:

- Check interval: configurable, default 15s
- Heartbeat timeout: configurable, default 90s (no heartbeat for 90s → Dead)
- On timeout: set status to Dead, emit event to SSE stream
- On reconnect: restore status to Connected, update `connected_at`

### 6. Command Dispatcher

Dispatches tasks to agents and tracks results:

```rust
struct Task {
    id: String,
    agent_id: String,
    task_type: String,
    parameters: serde_json::Value,
    timeout: Duration,
    status: TaskStatus,
    result: Option<serde_json::Value>,
    created_at: DateTime<Utc>,
    dispatched_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
}

enum TaskStatus {
    Pending, Dispatched, Running, Completed, Failed, Timeout,
}
```

### 7. SSE Event Stream

Server-Sent Events endpoint (`/api/v1/events`) for real-time event subscription. Clients can filter by `agent_id` or event type. Emits:
- Agent registered/disconnected/dead
- Task dispatched/completed/failed/timeout
- Custom events from agents

### 8. HTTP API Server

| Method | Path | Description |
|---|---|---|
| GET | `/api/v1/agents` | List all agents |
| GET | `/api/v1/agents/:id` | Get specific agent details |
| POST | `/api/v1/agents/:id/tasks` | Dispatch task to agent |
| GET | `/api/v1/tasks/:id` | Get task status and result |
| GET | `/api/v1/tasks` | List all tasks (filter by status, agent_id) |
| GET | `/api/v1/events` | SSE event stream |
| GET | `/metrics` | Prometheus metrics endpoint |
| GET | `/health` | Control plane health check |

## Error Handling

| Scenario | Behavior |
|---|---|
| Agent WebSocket disconnect | Mark Disconnected, retain state for 5 min for reconnect recovery |
| Heartbeat timeout | Mark Dead, emit event, do not purge (allow reconnect recovery) |
| Task timeout | Mark task Timeout, agent connection preserved |
| Invalid message format | Return error message to agent, do not disconnect |
| Control plane restart | Agents reconnect and re-register automatically |
| Duplicate agent_id on reconnect | Update existing record, reset connection state |

## Security

- Token-based authentication for WebSocket connections (`ws://host:port/ws?token=xxx`)
- Optional TLS (wss://)
- Token validated during register phase; invalid token → close connection
- REST API can be secured with same token (Bearer auth)

## Crate Structure

```
crates/vol-agent-manager/
├── Cargo.toml
├── src/
│   ├── main.rs              # Binary entrypoint, config loading
│   ├── config.rs            # Configuration (port, token, timeouts)
│   ├── lib.rs               # Public exports
│   ├── state/
│   │   ├── mod.rs
│   │   ├── manager.rs       # AgentStateManager
│   │   └── models.rs        # AgentState, AgentStatus, HostInfo
│   ├── ws/
│   │   ├── mod.rs
│   │   ├── server.rs        # WebSocket server setup
│   │   ├── handler.rs       # WebSocket message handler
│   │   └── protocol.rs      # Message types and serialization
│   ├── api/
│   │   ├── mod.rs
│   │   ├── routes.rs        # HTTP API routes
│   │   └── handlers.rs      # Route handlers
│   ├── task/
│   │   ├── mod.rs
│   │   └── dispatcher.rs    # CommandDispatcher, Task
│   ├── health/
│   │   ├── mod.rs
│   │   └── checker.rs       # HealthChecker
│   ├── metrics/
│   │   ├── mod.rs
│   │   └── collector.rs     # Prometheus metrics
│   └── events/
│       ├── mod.rs
│       └── sse.rs           # SSE event stream
└── tests/
    └── integration.rs       # Integration tests
```

## Dependencies

| Crate | Purpose |
|---|---|
| `axum` | HTTP + WebSocket server |
| `tokio` | Async runtime |
| `serde`, `serde_json` | JSON serialization |
| `prometheus` | Metrics collection |
| `tracing` | Structured logging |
| `chrono` | Time handling |
| `uuid` | Task/agent ID generation |
| `tower-http` | CORS, compression middleware |

## Out of Scope (MVP)

- Multi-instance HA (no Redis/KV state sync)
- Agent-to-agent communication
- Persistent state storage (state is in-memory)
- Dashboard UI
- Role-based access control

## Testing Strategy

- Unit tests for protocol parsing and state transitions
- Integration tests using WebSocket client connections
- Mock agent simulator for load testing
- Prometheus metrics verification via `/metrics` endpoint parsing
