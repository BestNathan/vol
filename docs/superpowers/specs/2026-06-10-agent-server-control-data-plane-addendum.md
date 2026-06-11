# Design Spec: Agent Server Control/Data Plane Addendum

**Date:** 2026-06-10
**Status:** Draft
**Source:** `docs/superpowers/architectures/2026-06-10-agent-server-control-data-plane.md`

## Summary

This addendum refines the agent-server control/data-plane architecture. The main architecture sets the final crate boundary: `vol-llm-agent-channel` owns protocol/transport/abstractions, `vol-agent-server` owns concrete `DataPlaneServerCore` and `ControlPlaneServerCore`, and `vol-llm-runtime` owns execution resources.

This document adds details that should guide the implementation plan: endpoint role allowlists, command/run semantics, capability revision consistency, node session lifecycle, migration constraints, and boundary verification tests.

## 1. Endpoint Role and Method Allowlist

`ControlPlaneServerCore` serves two JSON-RPC WebSocket endpoints with the same wire protocol but different roles:

| Endpoint | Role | Purpose |
|----------|------|---------|
| `/ws` | `Client` | UI/CLI/client-facing control-plane API |
| `/control/v1/ws` | `DataPlaneNode` | Data-plane node registration, reports, command execution |

Allowed methods should be explicit.

| Method | Client `/ws` | Node `/control/v1/ws` |
|--------|--------------|------------------------|
| `agent.list` / `agent.status` | yes | no |
| `agent.submit` / `agent.cancel` | yes | no |
| `tool.list` / `tool.call` | yes | no |
| `mcp.*` catalog/call methods | yes | no |
| `skill.list` | yes | no |
| `task.*` / `session.*` | yes, routed or local by policy | no |
| `control.node_list` | yes | no |
| `control.capability_list` | yes | no |
| `control.run_status` | yes | no |
| `control.register` | no | yes |
| `control.heartbeat` | no | yes |
| `control.capability_snapshot` | no | yes |
| `control.capability_delta` | no | yes |
| `control.event` | no | yes |
| `control.command_result` | no | yes |
| `control.command` | no | inbound from control plane to node only |

If a method is received on the wrong endpoint, return a protocol/domain error such as `method_not_allowed_for_role`.

## 2. Command and Run Semantics

`control.command` represents one control-plane instruction sent to one data-plane node. It is not the same as an agent run.

A command may create a run:

```text
control.command(command_id) -> run_id
```

Examples:

- `SubmitAgent` creates a `run_id`.
- `CallTool` may not create a `run_id`.
- `RefreshCapabilities` does not create a `run_id`.
- `HealthCheck` does not create a `run_id`.

### Recommended `control.command` flow

1. Control plane sends JSON-RPC request `control.command`.
2. Data plane validates and accepts/rejects quickly.
3. JSON-RPC response means accepted/rejected, not full agent completion.
4. Long-running lifecycle events are sent through `control.event` notifications.
5. Terminal command status is sent through `control.command_result`.

This avoids holding JSON-RPC requests open for long agent runs.

### Records

```rust
pub struct CommandRecord {
    pub command_id: String,
    pub node_id: String,
    pub operation_kind: String,
    pub status: CommandStatus,
    pub created_at_ms: u64,
    pub accepted_at_ms: Option<u64>,
    pub completed_at_ms: Option<u64>,
    pub run_id: Option<String>,
}

pub struct RunRecord {
    pub run_id: String,
    pub command_id: Option<String>,
    pub node_id: String,
    pub agent_id: String,
    pub status: RunStatus,
}
```

`CommandStore` and `RunStore` should be separate concepts even if both are in-memory in the MVP.

## 3. Capability Snapshot Consistency

Capability reporting should use monotonic node-local revisions.

```rust
pub struct CapabilitySnapshot {
    pub node_id: String,
    pub revision: u64,
    pub generated_at_ms: u64,
    pub agents: Vec<AgentCapability>,
    pub tools: Vec<ToolCapability>,
    pub mcp_servers: Vec<McpServerCapability>,
    pub skills: Vec<SkillCapability>,
}
```

Rules:

1. Each node owns a monotonic `revision` counter.
2. Control plane stores current revision per node.
3. If `revision <= current_revision`, ignore the snapshot as stale.
4. Full snapshot uses replace semantics for that node.
5. Missing capabilities in a newer full snapshot are removed for that node.
6. Delta snapshots must include `base_revision`.
7. If `base_revision != current_revision`, reject delta and request full snapshot.

MVP may send full snapshots only, but the protocol should reserve safe delta semantics:

```rust
pub struct CapabilityDelta {
    pub node_id: String,
    pub base_revision: u64,
    pub revision: u64,
    pub added: CapabilityPatch,
    pub removed: CapabilityPatch,
    pub updated: CapabilityPatch,
}
```

## 4. Node Record vs Node Session

Separate durable-ish node state from live connection state.

```rust
pub struct NodeRecord {
    pub node_id: String,
    pub name: String,
    pub version: String,
    pub status: NodeStatus,
    pub last_seen_at_ms: u64,
    pub capability_revision: u64,
    pub load: NodeLoad,
}

pub struct NodeSession {
    pub node_id: String,
    pub generation: u64,
    pub conn: Arc<dyn Connection>,
    pub connected_at_ms: u64,
}
```

`NodeRecord` can later be persisted. `NodeSession` cannot.

Reconnect behavior:

1. Same `node_id` reconnects.
2. Auth identity must match.
3. Increment session generation.
4. Replace active session handle.
5. Close old session.
6. Require a fresh full capability snapshot.
7. Mark in-flight commands from the old generation as `Unknown`, `NodeReconnected`, or wait for dedup result if node-side cache survives.

## 5. Combined Mode Lifecycle

In combined mode, one `vol-agent-server` process starts both cores.

Route ownership:

| Mode | `/ws` owner | `/control/v1/ws` owner |
|------|-------------|------------------------|
| standalone data-plane | `DataPlaneServerCore` | none |
| standalone control-plane | `ControlPlaneServerCore` client endpoint | `ControlPlaneServerCore` node endpoint |
| combined | `ControlPlaneServerCore` client endpoint | `ControlPlaneServerCore` node endpoint |

Control-plane role has priority over `/ws`. If users want old standalone behavior, they disable the control-plane role.

Combined mode should initially use loopback JSON-RPC registration from the local data plane to `/control/v1/ws`. This verifies the same path as remote nodes.

Startup order:

1. Build `ControlPlaneServerCore`.
2. Build `DataPlaneServerCore`.
3. Bind routes.
4. Start lease scanner.
5. Start local `DataPlaneReporter` with reconnect/backoff.

Shutdown order:

1. Stop accepting new connections.
2. Stop reporter heartbeat.
3. Mark local node disconnecting.
4. Drain in-flight commands with timeout.
5. Stop runtime and background tasks.

## 6. Runtime Capability Source Facade

Avoid scattering runtime lock reads throughout reporter code. Add a data-plane-local facade:

```rust
#[async_trait::async_trait]
pub trait RuntimeCapabilitySource {
    async fn snapshot_capabilities(&self) -> CapabilitySnapshot;
    async fn current_load(&self) -> NodeLoad;
}
```

This trait belongs in `vol-agent-server::data_plane`, not channel. It hides `AgentRuntime` internals from `DataPlaneReporter` and makes snapshot tests easier.

## 7. Capability Policy Hints

Capabilities should include enough metadata for future routing and approval decisions.

For tools and MCP tools, include at least:

```rust
pub sensitivity: ToolSensitivity,
pub requires_approval: bool,
pub annotations: HashMap<String, String>,
```

MVP can index these fields without enforcing full policy. The important part is not to route sensitive tools blindly based only on name.

## 8. Subscription Model

MVP should preserve existing run-event behavior through `agent.subscribe` / `agent.unsubscribe` where possible.

Future control-plane-wide subscriptions can use:

```text
control.subscribe
control.unsubscribe
```

Potential topics:

```rust
pub enum SubscriptionTopic {
    NodeEvents,
    RunEvents { run_id: Option<String> },
    CapabilityEvents,
    CommandEvents { command_id: Option<String> },
}
```

Do not add broad subscription APIs in MVP unless the UI needs them.

## 9. Error Code Ownership

Protocol-level errors belong in channel:

```text
parse_error
invalid_request
method_not_found
invalid_params
internal_error
```

Domain-level error codes should also be defined in channel protocol, while server fills detail:

```text
method_not_allowed_for_role
node_not_registered
node_unavailable
capability_not_found
command_timeout
command_rejected
agent_not_found
tool_not_found
mcp_server_unavailable
permission_required
```

Example:

```json
{
  "code": "node_unavailable",
  "message": "node is dead",
  "detail": {
    "node_id": "node-a",
    "last_seen_at_ms": 123
  }
}
```

## 10. Migration Constraint

Moving current `AgentServerCore` out of channel is a workspace-internal breaking change. Do not try to preserve compatibility by re-exporting server types from channel, because channel must not depend on server.

Recommended migration:

1. Add `JsonRpcMessageService` in channel.
2. Make current `AgentServerCore` implement it in-place.
3. Make `JsonRpcServer` generic over `JsonRpcMessageService`.
4. Move `AgentServerCore` and concrete data-plane handlers to `vol-agent-server::data_plane`.
5. Update all workspace references in the same phase.
6. Add boundary tests to prevent channel depending on server.

## 11. Boundary Verification Tests

Add tests/checks for dependency direction:

```text
vol-llm-agent-channel must not depend on vol-agent-server
vol-llm-runtime must not depend on vol-agent-server
vol-agent-server may depend on both channel and runtime
```

Also add mode-routing tests:

| control_plane | data_plane | Expected |
|---------------|------------|----------|
| false | true | `/ws -> DataPlaneServerCore` |
| true | false | `/ws -> ControlPlaneServerCore`, `/control/v1/ws -> node endpoint` |
| true | true | `/ws -> ControlPlaneServerCore`, local node registers |
| false | false | config error |

## Open Decisions

1. Whether `control.command_result` is always a notification or sometimes folded into synchronous `control.command` result for short operations.
2. Whether combined mode should support an in-process registration shortcut in addition to loopback JSON-RPC.
3. Whether `control.subscribe` is needed in MVP or can wait until UI asks for node/capability event streams.
