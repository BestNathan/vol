# Design: Data-Plane Remote Registration and Sandbox Fault Tolerance

## Background

Previous deployment testing revealed two gaps:

1. **Sandbox init crash**: `SandboxRegistry::load()` propagates any per-sandbox error as fatal. When an SSH sandbox cannot find `known_hosts`, the entire agent-server crashes instead of logging a warning and continuing with the remaining sandboxes.

2. **Remote data-plane registration not implemented**: The `control_url` field exists in config but standalone data-plane startup (`app.rs`) only serves its own WebSocket — it never connects to a remote control-plane. Combined mode (`control_plane=true, data_plane=true`) registers in-process, but standalone data-plane nodes must connect to a control-plane's `/control/v1/ws` node WebSocket.

## Goals

1. Make individual sandbox initialization failures non-fatal: log a warning, skip that sandbox, continue with the rest (including the built-in `local` sandbox).
2. Implement remote data-plane → control-plane connection so standalone data-plane instances register with a control-plane, send heartbeats and capability snapshots, and receive agent run requests from the control-plane.

## Non-Goals

1. Do not implement service discovery — `control_url` is explicitly configured.
2. Do not add authentication to the WebSocket connection (auth_token field exists and can be used later).
3. Do not change combined mode behavior.
4. Do not implement connection pooling or multiple control-plane endpoints.
5. Do not extract the connection logic into a separate module (`data_plane::connector`) — keep it inline in `app.rs` for minimal scope. Refactor later if needed.

## Architecture

### Sandbox Fault Tolerance

**File**: `crates/vol-llm-sandbox/src/registry.rs`

`SandboxRegistry::load()` iterates over `*.toml` files. Currently any error constructing or starting a sandbox is propagated with `?`. Change the per-file loop to:

```text
for each *.toml file:
  parse config → on error: warn + continue
  construct sandbox → on error: warn + continue
  start sandbox → on error: warn + continue
  insert into registry
```

The built-in `local` sandbox is always present regardless of per-file errors.

**File**: `crates/vol-llm-runtime/src/lib.rs`

No change needed — the `?` on `SandboxRegistry::load()` stays because `load()` itself can still fail on directory I/O errors. Per-sandbox errors are handled inside `load()`.

### Remote Data-Plane → Control-Plane Connection

**File**: `crates/vol-agent-server/src/app.rs`

When `control_plane_enabled == false && data_plane_enabled == true` AND
`config.data_plane.control_url` is `Some`, spawn a tokio task that:

```text
loop:
  connect via tokio_tungstenite to control_url
  build JSON-RPC control.register message:
    { node_id, name, version }
  send register, receive RegisterAck
  spawn heartbeat timer (every heartbeat_secs):
    send control.heartbeat { node_id, load }
  build capability snapshot from DataPlaneServerCore:
    agents, tools, skills, mcp_servers
  send control.capability_snapshot { ... }
  wait for disconnect or error
  on disconnect: sleep with exponential backoff (1s → 60s max), reconnect
```

Key details:
- `node_id`: from `config.data_plane.node_id` or default `"dp-<short-hostname>"`
- `name`: from `config.data_plane.name` or `"data-plane"`
- `version`: `env!("CARGO_PKG_VERSION")`
- `capability_snapshot`: collected from `DataPlaneServerCore` after agent discovery
- Reconnect: exponential backoff 1s → 60s, jitter ±25%

**Dependencies**: `tokio-tungstenite` already in workspace deps.

## Data Flow

```text
┌──────────────────┐                    ┌──────────────────┐
│   Control-Plane  │                    │    Data-Plane    │
│   (port 3001)    │                    │   (port 3002)    │
│                  │                    │                  │
│  /ws          ◄──┼─ client conns      │                  │
│  /control/v1/ws ◄┼── node conns ◄─────┼─ tokio connect   │
│                  │                    │                  │
│  1. recv register│◄───────────────────│  1. connect      │
│  2. send ack     │───────────────────►│  2. send register│
│                  │                    │  3. send snapshot│
│                  │                    │  4. heartbeat    │
│  3. route agent  │───────────────────►│  5. run agent    │
│     run request  │                    │     in sandbox   │
│                  │                    │                  │
└──────────────────┘                    └──────────────────┘
```

## Error Handling

- Sandbox per-file parse/start errors → `tracing::warn!`, skip file, continue. `SandboxRegistry` always contains `local`.
- WebSocket connect failure → exponential backoff, retry indefinitely.
- Register/Heartbeat send failure → disconnect, reconnect.
- `control_url` missing → standalone data-plane starts without remote registration (backward compatible).
- Concurrent control-plane connections → only one active connection per data-plane instance; reconnect replaces old.

## Testing

1. **Sandbox**: Add a test with one valid TOML + one invalid TOML → registry contains valid sandbox + local, invalid is skipped.
2. **Registration**: Unit test that `ControlHandler` handling `Register` returns `RegisterAck`.
3. **Integration**: Manual deploy of data-plane with `control_url` → verify control-plane logs show node registration, capability snapshot received.

## Open Questions

None for this implementation.
