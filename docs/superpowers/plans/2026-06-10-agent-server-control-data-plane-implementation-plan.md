# Agent Server Control/Data Plane Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor agent-server boundaries so `vol-llm-agent-channel` owns protocol/JSON-RPC abstractions, while `vol-agent-server` owns concrete `DataPlaneServerCore` and `ControlPlaneServerCore` with JSON-RPC over WebSocket control/data-plane communication.

**Architecture:** Implement in stages. First make channel transport generic and add control protocol types. Then move concrete data-plane server behavior from channel into `vol-agent-server`. Finally add a minimal in-memory control-plane core, node endpoint, capability indexing, and data-plane reporter loopback path.

**Tech Stack:** Rust, Tokio, Axum WebSocket, Serde JSON, existing `vol-llm-agent-channel`, `vol-agent-server`, `vol-llm-runtime`, `vol-llm-tool`, `vol-llm-mcp`, `vol-session`.

---

## Scope and Sequencing

This plan implements an MVP with these boundaries:

- `vol-llm-agent-channel` owns:
  - JSON-RPC codec/server/connection abstractions
  - `AgentServerMessage`, `Operation`, `Payload`
  - `ControlOperation`, `ControlPayload`, and control-plane payload models
  - `DomainHandler`, `HandlerRegistry`
  - `JsonRpcMessageService`
- `vol-agent-server` owns:
  - `DataPlaneServerCore`
  - `ControlPlaneServerCore`
  - `ControlPlaneState`, `NodeRegistry`, `CapabilityIndex`, `ControlRouter`, `EventBus`, `RunStore`
  - data-plane reporter/client/snapshot/command executor
  - role-based config and route composition
- `vol-llm-runtime` remains unaware of the control plane.

The implementation should be split into commits by task. Do not attempt to move every protocol type into a new `protocol/` tree in the first commit; first add `control.*` to the existing `agent_server_protocol.rs` and make the JSON-RPC service generic. A later cleanup task can move protocol files once behavior is covered by tests.

## Target File Structure

### `crates/vol-llm-agent-channel`

- Create: `crates/vol-llm-agent-channel/src/service.rs`
  - Defines `JsonRpcMessageService`.
- Modify: `crates/vol-llm-agent-channel/src/lib.rs`
  - Exports `JsonRpcMessageService`.
  - Stops exporting `AgentServerCore` after the server core is moved.
- Modify: `crates/vol-llm-agent-channel/src/transport/jsonrpc/server.rs`
  - Makes `JsonRpcServer` generic over `JsonRpcMessageService`.
  - Allows mounting at a configurable path.
- Modify: `crates/vol-llm-agent-channel/src/agent_server_protocol.rs`
  - Adds `ControlOperation`, `ControlPayload`, capability models, node models, command models, and payload mapping.
- Modify: `crates/vol-llm-agent-channel/src/operation_codec.rs`
  - Maps `control.*` JSON-RPC method strings to `Operation::Control` and decodes control payloads.
- Keep initially, then remove/move: `crates/vol-llm-agent-channel/src/server_core.rs`, `router.rs`, `dispatcher.rs`, and data-plane `domain/*` concrete handlers.

### `crates/vol-agent-server`

- Create: `crates/vol-agent-server/src/app.rs`
  - Builds configured role cores and starts background tasks.
- Create: `crates/vol-agent-server/src/routes.rs`
  - Mounts `/ws`, `/control/v1/ws`, `/health`, `/metrics`.
- Create: `crates/vol-agent-server/src/health.rs`
  - Plain health endpoint.
- Modify: `crates/vol-agent-server/src/main.rs`
  - Delegates startup to `app::run`.
- Modify: `crates/vol-agent-server/src/config.rs`
  - Adds `[server.roles]`, `[control_plane]`, `[data_plane]` config.
- Create: `crates/vol-agent-server/src/data_plane/core.rs`
  - New home of current `AgentServerCore` behavior as `DataPlaneServerCore`.
- Create: `crates/vol-agent-server/src/data_plane/builder.rs`
  - Builds `DataPlaneServerCore` from runtime config.
- Create/move: `crates/vol-agent-server/src/data_plane/router.rs`
  - Current local `AgentRouter`.
- Create/move: `crates/vol-agent-server/src/data_plane/dispatcher.rs`
  - Current `AgentDispatcher`.
- Create/move: `crates/vol-agent-server/src/data_plane/connection_holder.rs`
  - Current `ConnectionHolder` plugin.
- Create/move: `crates/vol-agent-server/src/data_plane/handlers/*.rs`
  - Current data-plane concrete handlers: agent, file, log, mcp, session, skill, system, task, tool.
- Create: `crates/vol-agent-server/src/data_plane/snapshot.rs`
  - Runtime capability snapshotter.
- Create: `crates/vol-agent-server/src/data_plane/command.rs`
  - Executes `ControlCommand` against local data-plane core.
- Create: `crates/vol-agent-server/src/data_plane/client.rs`
  - Data-plane JSON-RPC control-plane client skeleton.
- Create: `crates/vol-agent-server/src/data_plane/reporter.rs`
  - Heartbeat/snapshot reporter skeleton.
- Create: `crates/vol-agent-server/src/control_plane/core.rs`
  - `ControlPlaneServerCore`.
- Create: `crates/vol-agent-server/src/control_plane/state.rs`
  - `ControlPlaneState`.
- Create: `crates/vol-agent-server/src/control_plane/registry.rs`
  - `NodeRegistry` and `NodeSession`.
- Create: `crates/vol-agent-server/src/control_plane/capability.rs`
  - `CapabilityIndex`.
- Create: `crates/vol-agent-server/src/control_plane/lease.rs`
  - Lease scanner logic.
- Create: `crates/vol-agent-server/src/control_plane/router.rs`
  - MVP `ControlRouter`.
- Create: `crates/vol-agent-server/src/control_plane/event.rs`
  - Broadcast event bus.
- Create: `crates/vol-agent-server/src/control_plane/store.rs`
  - In-memory command/run records.
- Create: `crates/vol-agent-server/src/control_plane/handlers/control.rs`
  - Handles node-facing `control.*` methods.
- Create: `crates/vol-agent-server/src/control_plane/handlers/node.rs`
  - Handles client-facing node queries.
- Create: `crates/vol-agent-server/src/control_plane/handlers/capability.rs`
  - Handles client-facing capability queries.
- Create: `crates/vol-agent-server/src/control_plane/handlers/run.rs`
  - Handles client-facing run status/submit routing skeleton.

---

## Task 1: Add Generic JSON-RPC Service Abstraction

**Files:**
- Create: `crates/vol-llm-agent-channel/src/service.rs`
- Modify: `crates/vol-llm-agent-channel/src/lib.rs`
- Modify: `crates/vol-llm-agent-channel/src/server_core.rs`
- Modify: `crates/vol-llm-agent-channel/src/transport/jsonrpc/server.rs`
- Test: `crates/vol-llm-agent-channel/src/transport/jsonrpc/server.rs`

- [ ] **Step 1: Write failing service trait compile test**

Add this test to the bottom of `crates/vol-llm-agent-channel/src/transport/jsonrpc/server.rs`:

```rust
#[cfg(test)]
mod generic_service_tests {
    use std::sync::Arc;

    use async_trait::async_trait;

    use crate::connection::Connection;
    use crate::service::JsonRpcMessageService;
    use super::JsonRpcServer;

    struct MockService;

    #[async_trait]
    impl JsonRpcMessageService for MockService {
        async fn serve_connection(&self, _conn: Arc<dyn Connection>) {}
    }

    #[test]
    fn jsonrpc_server_accepts_generic_service_and_path() {
        let service = Arc::new(MockService);
        let _server = JsonRpcServer::new(service, "/custom/ws");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p vol-llm-agent-channel generic_service_tests::jsonrpc_server_accepts_generic_service_and_path
```

Expected: compile failure because `crate::service::JsonRpcMessageService` does not exist and `JsonRpcServer::new` does not accept `(service, path)`.

- [ ] **Step 3: Add `JsonRpcMessageService` trait**

Create `crates/vol-llm-agent-channel/src/service.rs`:

```rust
use std::sync::Arc;

use async_trait::async_trait;

use crate::connection::Connection;

/// Generic service abstraction consumed by JSON-RPC WebSocket transport.
///
/// Implementations own connection lifecycle behavior. Concrete services live
/// outside the transport layer, e.g. data-plane and control-plane server cores.
#[async_trait]
pub trait JsonRpcMessageService: Send + Sync + 'static {
    async fn serve_connection(&self, conn: Arc<dyn Connection>);
}
```

Modify `crates/vol-llm-agent-channel/src/lib.rs`:

```rust
pub mod service;
pub use service::JsonRpcMessageService;
```

Keep existing exports unchanged in this task.

- [ ] **Step 4: Make `JsonRpcServer` generic over service**

Replace `crates/vol-llm-agent-channel/src/transport/jsonrpc/server.rs` with this shape, preserving existing imports where needed:

```rust
//! JSON-RPC server providing a WebSocket endpoint.

use std::sync::Arc;

use axum::extract::ws::{WebSocket, WebSocketUpgrade};
use axum::routing::get;
use axum::Router;

use crate::connection::Connection;
use crate::service::JsonRpcMessageService;

use super::connection::JsonRpcConnection;

/// JSON-RPC server providing a WebSocket endpoint.
pub struct JsonRpcServer<S> {
    service: Arc<S>,
    path: &'static str,
}

impl<S> JsonRpcServer<S>
where
    S: JsonRpcMessageService,
{
    /// Create a new server wrapping the given service and mounting path.
    pub fn new(service: Arc<S>, path: &'static str) -> Self {
        Self { service, path }
    }

    /// Build an axum `Router` with the JSON-RPC WebSocket endpoint.
    pub fn into_axum_router(self) -> Router {
        let path = self.path;
        let server = Arc::new(self);

        Router::new().route(
            path,
            get(move |ws: WebSocketUpgrade| {
                let server = server.clone();
                async move { ws.on_upgrade(move |socket| handle_ws(socket, server)) }
            }),
        )
    }
}

async fn handle_ws<S>(socket: WebSocket, server: Arc<JsonRpcServer<S>>)
where
    S: JsonRpcMessageService,
{
    let conn: Arc<dyn Connection> = Arc::new(JsonRpcConnection::new(socket));
    server.service.serve_connection(conn).await;
}
```

If this breaks existing tests that call `JsonRpcServer::new(core)`, update those call sites to pass `"/ws"`.

- [ ] **Step 5: Implement service trait for current `AgentServerCore`**

In `crates/vol-llm-agent-channel/src/server_core.rs`, add:

```rust
use async_trait::async_trait;
use crate::connection::Connection;
use crate::service::JsonRpcMessageService;

#[async_trait]
impl JsonRpcMessageService for AgentServerCore {
    async fn serve_connection(&self, conn: Arc<dyn Connection>) {
        self.serve(conn).await;
    }
}
```

If `serve` currently takes a concrete generic connection, adjust it to accept `Arc<dyn Connection>` or add a wrapper overload:

```rust
pub async fn serve_dyn(&self, conn: Arc<dyn Connection>) {
    self.serve(conn).await;
}
```

Use the smallest change that compiles.

- [ ] **Step 6: Update server binary call site**

In `crates/vol-agent-server/src/main.rs`, change:

```rust
let server = JsonRpcServer::new(Arc::new(core));
```

to:

```rust
let server = JsonRpcServer::new(Arc::new(core), "/ws");
```

- [ ] **Step 7: Run tests**

Run:

```bash
cargo test -p vol-llm-agent-channel generic_service_tests::jsonrpc_server_accepts_generic_service_and_path
cargo test -p vol-llm-agent-channel transport::jsonrpc
cargo check -p vol-agent-server
```

Expected: all pass.

- [ ] **Step 8: Commit**

```bash
git add crates/vol-llm-agent-channel/src/service.rs \
  crates/vol-llm-agent-channel/src/lib.rs \
  crates/vol-llm-agent-channel/src/server_core.rs \
  crates/vol-llm-agent-channel/src/transport/jsonrpc/server.rs \
  crates/vol-agent-server/src/main.rs
git commit -m "refactor(channel): generalize jsonrpc server service"
```

---

## Task 2: Add `control.*` Protocol Types in Channel

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/agent_server_protocol.rs`
- Modify: `crates/vol-llm-agent-channel/src/operation_codec.rs`
- Test: `crates/vol-llm-agent-channel/src/transport/jsonrpc/codec.rs`

- [ ] **Step 1: Write failing JSON-RPC decode tests**

Add tests to `crates/vol-llm-agent-channel/src/transport/jsonrpc/codec.rs`:

```rust
#[test]
fn decode_control_register() {
    let msg = decode_jsonrpc_frame(
        r#"{"jsonrpc":"2.0","id":"reg-1","method":"control.register","params":{"node_id":"node-a","name":"Node A","version":"0.1.0"}}"#,
    )
    .unwrap();

    assert_eq!(msg.message_id, "reg-1");
    assert_eq!(msg.operation, Operation::Control(ControlOperation::Register));
    match msg.payload {
        Payload::Control(ControlPayload::Register(p)) => {
            assert_eq!(p.node_id, "node-a");
            assert_eq!(p.name, "Node A");
            assert_eq!(p.version, "0.1.0");
        }
        other => panic!("unexpected payload: {other:?}"),
    }
}

#[test]
fn decode_control_heartbeat_notification() {
    let msg = decode_jsonrpc_frame(
        r#"{"jsonrpc":"2.0","id":"hb-1","method":"control.heartbeat","params":{"node_id":"node-a","status":"online","load":{"running":1,"queued":2}}}"#,
    )
    .unwrap();

    assert_eq!(msg.operation, Operation::Control(ControlOperation::Heartbeat));
}
```

Also add imports in the test module:

```rust
use crate::agent_server_protocol::{ControlOperation, ControlPayload};
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p vol-llm-agent-channel decode_control_register decode_control_heartbeat_notification
```

Expected: compile failure because `ControlOperation`, `ControlPayload`, and `Payload::Control` do not exist.

- [ ] **Step 3: Add control operation and payload models**

In `crates/vol-llm-agent-channel/src/agent_server_protocol.rs`, add `Control` to `Operation`:

```rust
Control(ControlOperation),
```

Extend `method_name()`:

```rust
Operation::Control(ControlOperation::Register) => "control.register",
Operation::Control(ControlOperation::Heartbeat) => "control.heartbeat",
Operation::Control(ControlOperation::CapabilitySnapshot) => "control.capability_snapshot",
Operation::Control(ControlOperation::CapabilityDelta) => "control.capability_delta",
Operation::Control(ControlOperation::Event) => "control.event",
Operation::Control(ControlOperation::Command) => "control.command",
Operation::Control(ControlOperation::CommandAck) => "control.command_ack",
Operation::Control(ControlOperation::CommandResult) => "control.command_result",
Operation::Control(ControlOperation::NodeList) => "control.node_list",
Operation::Control(ControlOperation::NodeGet) => "control.node_get",
Operation::Control(ControlOperation::CapabilityList) => "control.capability_list",
Operation::Control(ControlOperation::RunStatus) => "control.run_status",
```

Add enum:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControlOperation {
    Register,
    Heartbeat,
    CapabilitySnapshot,
    CapabilityDelta,
    Event,
    Command,
    CommandAck,
    CommandResult,
    NodeList,
    NodeGet,
    CapabilityList,
    RunStatus,
}
```

Add `Payload::Control(ControlPayload)`.

Add minimal models:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ControlPayload {
    Register(NodeRegistration),
    RegisterAck(RegisterAck),
    Heartbeat(NodeHeartbeat),
    CapabilitySnapshot(CapabilitySnapshot),
    CapabilityDelta(CapabilityDelta),
    Event(DataPlaneEvent),
    Command(ControlCommand),
    CommandAck(CommandAck),
    CommandResult(CommandResult),
    NodeList(NodeListRequest),
    NodeListResult(NodeListResult),
    NodeGet(NodeGetRequest),
    NodeGetResult(NodeGetResult),
    CapabilityList(CapabilityListRequest),
    CapabilityListResult(CapabilityListResult),
    RunStatus(RunStatusRequest),
    RunStatusResult(RunStatusResult),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeRegistration {
    pub node_id: String,
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegisterAck {
    pub node_id: String,
    pub accepted: bool,
    pub generation: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeHeartbeat {
    pub node_id: String,
    pub status: String,
    #[serde(default)]
    pub load: NodeLoad,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NodeLoad {
    pub running: u64,
    pub queued: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilitySnapshot {
    pub node_id: String,
    pub revision: u64,
    #[serde(default)]
    pub generated_at_ms: Option<u64>,
    #[serde(default)]
    pub agents: Vec<AgentCapability>,
    #[serde(default)]
    pub tools: Vec<ToolCapability>,
    #[serde(default)]
    pub mcp_servers: Vec<McpServerCapability>,
    #[serde(default)]
    pub skills: Vec<SkillCapability>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityDelta {
    pub node_id: String,
    pub base_revision: u64,
    pub revision: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentCapability {
    pub agent_id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCapability {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub sensitivity: Option<String>,
    #[serde(default)]
    pub requires_approval: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpServerCapability {
    pub name: String,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkillCapability {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataPlaneEvent {
    pub node_id: String,
    pub event_type: String,
    #[serde(default)]
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ControlCommand {
    pub command_id: String,
    pub node_id: String,
    pub operation: ControlCommandOperation,
    #[serde(default)]
    pub deadline_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", content = "payload")]
pub enum ControlCommandOperation {
    SubmitAgent { target: Option<String>, input: AgentInput },
    CancelRun { run_id: String },
    CallTool { name: String, args: serde_json::Value },
    CallMcpTool { server: String, name: String, args: serde_json::Value },
    RefreshCapabilities,
    HealthCheck,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandAck {
    pub command_id: String,
    pub accepted: bool,
    #[serde(default)]
    pub run_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandResult {
    pub command_id: String,
    pub status: String,
    #[serde(default)]
    pub result: serde_json::Value,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NodeListRequest {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeListResult {
    pub nodes: Vec<NodeRecord>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeGetRequest {
    pub node_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeGetResult {
    pub node: Option<NodeRecord>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeRecord {
    pub node_id: String,
    pub name: String,
    pub version: String,
    pub status: String,
    #[serde(default)]
    pub last_seen_at_ms: Option<u64>,
    #[serde(default)]
    pub capability_revision: u64,
    #[serde(default)]
    pub load: NodeLoad,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CapabilityListRequest {
    #[serde(default)]
    pub node_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityListResult {
    pub snapshots: Vec<CapabilitySnapshot>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunStatusRequest {
    pub run_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunStatusResult {
    pub run_id: String,
    pub status: String,
    #[serde(default)]
    pub node_id: Option<String>,
}
```

- [ ] **Step 4: Add control payload decoding**

In `Payload::from_operation`, add match arms for `Operation::Control(_)`. Start with these exact arms:

```rust
Operation::Control(ControlOperation::Register) => serde_json::from_value(value)
    .map(ControlPayload::Register)
    .map(Payload::Control)
    .map_err(|_| ProtocolError::PayloadDecodeFailed("control.register")),
Operation::Control(ControlOperation::Heartbeat) => serde_json::from_value(value)
    .map(ControlPayload::Heartbeat)
    .map(Payload::Control)
    .map_err(|_| ProtocolError::PayloadDecodeFailed("control.heartbeat")),
Operation::Control(ControlOperation::CapabilitySnapshot) => serde_json::from_value(value)
    .map(ControlPayload::CapabilitySnapshot)
    .map(Payload::Control)
    .map_err(|_| ProtocolError::PayloadDecodeFailed("control.capability_snapshot")),
Operation::Control(ControlOperation::CapabilityDelta) => serde_json::from_value(value)
    .map(ControlPayload::CapabilityDelta)
    .map(Payload::Control)
    .map_err(|_| ProtocolError::PayloadDecodeFailed("control.capability_delta")),
Operation::Control(ControlOperation::Event) => serde_json::from_value(value)
    .map(ControlPayload::Event)
    .map(Payload::Control)
    .map_err(|_| ProtocolError::PayloadDecodeFailed("control.event")),
Operation::Control(ControlOperation::Command) => serde_json::from_value(value)
    .map(ControlPayload::Command)
    .map(Payload::Control)
    .map_err(|_| ProtocolError::PayloadDecodeFailed("control.command")),
Operation::Control(ControlOperation::CommandAck) => serde_json::from_value(value)
    .map(ControlPayload::CommandAck)
    .map(Payload::Control)
    .map_err(|_| ProtocolError::PayloadDecodeFailed("control.command_ack")),
Operation::Control(ControlOperation::CommandResult) => serde_json::from_value(value)
    .map(ControlPayload::CommandResult)
    .map(Payload::Control)
    .map_err(|_| ProtocolError::PayloadDecodeFailed("control.command_result")),
Operation::Control(ControlOperation::NodeList) => serde_json::from_value(value)
    .map(ControlPayload::NodeList)
    .map(Payload::Control)
    .map_err(|_| ProtocolError::PayloadDecodeFailed("control.node_list")),
Operation::Control(ControlOperation::NodeGet) => serde_json::from_value(value)
    .map(ControlPayload::NodeGet)
    .map(Payload::Control)
    .map_err(|_| ProtocolError::PayloadDecodeFailed("control.node_get")),
Operation::Control(ControlOperation::CapabilityList) => serde_json::from_value(value)
    .map(ControlPayload::CapabilityList)
    .map(Payload::Control)
    .map_err(|_| ProtocolError::PayloadDecodeFailed("control.capability_list")),
Operation::Control(ControlOperation::RunStatus) => serde_json::from_value(value)
    .map(ControlPayload::RunStatus)
    .map(Payload::Control)
    .map_err(|_| ProtocolError::PayloadDecodeFailed("control.run_status")),
```

- [ ] **Step 5: Add method mapping**

In `crates/vol-llm-agent-channel/src/operation_codec.rs`, add mappings:

```rust
"control.register" => Operation::Control(ControlOperation::Register),
"control.heartbeat" => Operation::Control(ControlOperation::Heartbeat),
"control.capability_snapshot" => Operation::Control(ControlOperation::CapabilitySnapshot),
"control.capability_delta" => Operation::Control(ControlOperation::CapabilityDelta),
"control.event" => Operation::Control(ControlOperation::Event),
"control.command" => Operation::Control(ControlOperation::Command),
"control.command_ack" => Operation::Control(ControlOperation::CommandAck),
"control.command_result" => Operation::Control(ControlOperation::CommandResult),
"control.node_list" => Operation::Control(ControlOperation::NodeList),
"control.node_get" => Operation::Control(ControlOperation::NodeGet),
"control.capability_list" => Operation::Control(ControlOperation::CapabilityList),
"control.run_status" => Operation::Control(ControlOperation::RunStatus),
```

Add import for `ControlOperation`.

- [ ] **Step 6: Run tests**

Run:

```bash
cargo test -p vol-llm-agent-channel decode_control_register decode_control_heartbeat_notification
cargo test -p vol-llm-agent-channel operation_codec
```

Expected: pass.

- [ ] **Step 7: Commit**

```bash
git add crates/vol-llm-agent-channel/src/agent_server_protocol.rs \
  crates/vol-llm-agent-channel/src/operation_codec.rs \
  crates/vol-llm-agent-channel/src/transport/jsonrpc/codec.rs
git commit -m "feat(channel): add control plane jsonrpc protocol"
```

---

## Task 3: Add Server Role Config and Route Composition Skeleton

**Files:**
- Modify: `crates/vol-agent-server/src/config.rs`
- Create: `crates/vol-agent-server/src/app.rs`
- Create: `crates/vol-agent-server/src/routes.rs`
- Create: `crates/vol-agent-server/src/health.rs`
- Modify: `crates/vol-agent-server/src/main.rs`
- Test: `crates/vol-agent-server/src/config.rs`

- [ ] **Step 1: Write failing config tests**

Add tests to `crates/vol-agent-server/src/config.rs`:

```rust
#[test]
fn test_parse_roles_config() {
    let toml_str = r#"
        [server.roles]
        control_plane = true
        data_plane = false

        [control_plane]
        client_ws_path = "/ws"
        node_ws_path = "/control/v1/ws"
        lease_timeout_secs = 90
        lease_scan_secs = 15
    "#;

    let config: ServerConfig = toml::from_str(toml_str).unwrap();
    assert!(config.server.roles.control_plane);
    assert!(!config.server.roles.data_plane);
    assert_eq!(config.control_plane.client_ws_path, "/ws");
    assert_eq!(config.control_plane.node_ws_path, "/control/v1/ws");
}

#[test]
fn test_reject_both_roles_disabled() {
    let toml_str = r#"
        [server.roles]
        control_plane = false
        data_plane = false
    "#;

    let config: ServerConfig = toml::from_str(toml_str).unwrap();
    let err = config.validate().unwrap_err();
    assert!(err.contains("at least one server role must be enabled"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p vol-agent-server test_parse_roles_config test_reject_both_roles_disabled
```

Expected: compile failure because role config fields do not exist.

- [ ] **Step 3: Extend config**

Modify `ServerSection`:

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct ServerSection {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub roles: ServerRoles,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerRoles {
    #[serde(default)]
    pub control_plane: bool,
    #[serde(default = "default_true")]
    pub data_plane: bool,
}
```

Add sections to `ServerConfig`:

```rust
#[serde(default)]
pub control_plane: ControlPlaneSection,
#[serde(default)]
pub data_plane: DataPlaneSection,
```

Add defaults:

```rust
fn default_true() -> bool { true }
fn default_client_ws_path() -> String { "/ws".to_string() }
fn default_node_ws_path() -> String { "/control/v1/ws".to_string() }
fn default_lease_timeout_secs() -> u64 { 90 }
fn default_lease_scan_secs() -> u64 { 15 }
fn default_heartbeat_secs() -> u64 { 15 }

#[derive(Debug, Clone, Deserialize)]
pub struct ControlPlaneSection {
    #[serde(default)]
    pub auth_token: Option<String>,
    #[serde(default = "default_client_ws_path")]
    pub client_ws_path: String,
    #[serde(default = "default_node_ws_path")]
    pub node_ws_path: String,
    #[serde(default = "default_lease_timeout_secs")]
    pub lease_timeout_secs: u64,
    #[serde(default = "default_lease_scan_secs")]
    pub lease_scan_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DataPlaneSection {
    #[serde(default)]
    pub node_id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub control_url: Option<String>,
    #[serde(default)]
    pub auth_token: Option<String>,
    #[serde(default = "default_heartbeat_secs")]
    pub heartbeat_secs: u64,
    #[serde(default = "default_true")]
    pub snapshot_on_connect: bool,
}
```

Add `Default` impls for new sections.

In `validate()` add:

```rust
if !self.server.roles.control_plane && !self.server.roles.data_plane {
    return Err("at least one server role must be enabled".to_string());
}
if self.server.roles.data_plane && self.server.roles.control_plane {
    if self.data_plane.control_url.is_none() {
        // Combined mode may use loopback default later; keep this valid for now.
    }
}
```

In `expand_tilde()`, no changes are required for new URL fields.

- [ ] **Step 4: Create app/routes/health skeleton**

Create `crates/vol-agent-server/src/health.rs`:

```rust
use axum::Json;
use serde::Serialize;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
}

pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}
```

Create `crates/vol-agent-server/src/routes.rs`:

```rust
use axum::routing::get;
use axum::Router;

use crate::health;

pub fn base_router() -> Router {
    Router::new().route("/health", get(health::health))
}
```

Create `crates/vol-agent-server/src/app.rs`:

```rust
use crate::config::ServerConfig;

pub async fn run(config: ServerConfig) -> Result<(), String> {
    let app = crate::routes::base_router();
    let addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| format!("failed to bind {addr}: {e}"))?;
    tracing::info!("agent server started on {}", addr);
    axum::serve(listener, app)
        .await
        .map_err(|e| format!("server error: {e}"))
}
```

Do not wire app into `main.rs` yet in this task if it would remove current behavior. This skeleton is used by later tasks.

- [ ] **Step 5: Run tests**

Run:

```bash
cargo test -p vol-agent-server test_parse_roles_config test_reject_both_roles_disabled
cargo check -p vol-agent-server
```

Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add crates/vol-agent-server/src/config.rs \
  crates/vol-agent-server/src/app.rs \
  crates/vol-agent-server/src/routes.rs \
  crates/vol-agent-server/src/health.rs
git commit -m "feat(server): add role configuration skeleton"
```

---

## Task 4: Move Data-Plane Core Into `vol-agent-server`

**Files:**
- Create: `crates/vol-agent-server/src/data_plane/mod.rs`
- Create: `crates/vol-agent-server/src/data_plane/core.rs`
- Create/move: `crates/vol-agent-server/src/data_plane/{builder.rs,router.rs,dispatcher.rs,connection_holder.rs}`
- Create/move: `crates/vol-agent-server/src/data_plane/handlers/*.rs`
- Modify: `crates/vol-agent-server/src/main.rs`
- Modify: `crates/vol-agent-server/Cargo.toml`
- Modify: `crates/vol-llm-agent-channel/src/lib.rs`

- [ ] **Step 1: Identify move set**

Move these files from channel into server with module path updates:

```text
crates/vol-llm-agent-channel/src/server_core.rs -> crates/vol-agent-server/src/data_plane/core.rs
crates/vol-llm-agent-channel/src/router.rs -> crates/vol-agent-server/src/data_plane/router.rs
crates/vol-llm-agent-channel/src/dispatcher.rs -> crates/vol-agent-server/src/data_plane/dispatcher.rs
crates/vol-llm-agent-channel/src/connection.rs -> split: keep Connection trait in channel; move ConnectionHolder into crates/vol-agent-server/src/data_plane/connection_holder.rs
crates/vol-llm-agent-channel/src/domain/agent.rs -> crates/vol-agent-server/src/data_plane/handlers/agent.rs
crates/vol-llm-agent-channel/src/domain/file.rs -> crates/vol-agent-server/src/data_plane/handlers/file.rs
crates/vol-llm-agent-channel/src/domain/log.rs -> crates/vol-agent-server/src/data_plane/handlers/log.rs
crates/vol-llm-agent-channel/src/domain/mcp.rs -> crates/vol-agent-server/src/data_plane/handlers/mcp.rs
crates/vol-llm-agent-channel/src/domain/session.rs -> crates/vol-agent-server/src/data_plane/handlers/session.rs
crates/vol-llm-agent-channel/src/domain/skill.rs -> crates/vol-agent-server/src/data_plane/handlers/skill.rs
crates/vol-llm-agent-channel/src/domain/system.rs -> crates/vol-agent-server/src/data_plane/handlers/system.rs
crates/vol-llm-agent-channel/src/domain/task.rs -> crates/vol-agent-server/src/data_plane/handlers/task.rs
crates/vol-llm-agent-channel/src/domain/tool.rs -> crates/vol-agent-server/src/data_plane/handlers/tool.rs
```

- [ ] **Step 2: Create modules**

Create `crates/vol-agent-server/src/data_plane/mod.rs`:

```rust
pub mod builder;
pub mod command;
pub mod connection_holder;
pub mod core;
pub mod dispatcher;
pub mod handlers;
pub mod router;
pub mod snapshot;

pub use core::DataPlaneServerCore;
```

Create `crates/vol-agent-server/src/data_plane/handlers/mod.rs`:

```rust
pub mod agent;
pub mod file;
pub mod log;
pub mod mcp;
pub mod session;
pub mod skill;
pub mod system;
pub mod task;
pub mod tool;
```

Add to `crates/vol-agent-server/src/main.rs` top-level modules:

```rust
mod data_plane;
```

- [ ] **Step 3: Rename core type**

In the moved `core.rs`, rename:

```rust
pub struct AgentServerCore
```

to:

```rust
pub struct DataPlaneServerCore
```

Rename builder types:

```rust
AgentServerCoreBuilder -> DataPlaneServerCoreBuilder
```

Keep method names familiar with the existing argument shape:

```rust
impl DataPlaneServerCore {
    pub fn builder(
        working_dir: impl Into<std::path::PathBuf>,
        store_dir: impl Into<std::path::PathBuf>,
    ) -> DataPlaneServerCoreBuilder {
        DataPlaneServerCoreBuilder::new(working_dir.into(), store_dir.into())
    }

    pub async fn discover_agents(&self) -> Result<(), String> {
        let registrations = self.runtime.discover_agents().await?;
        for (agent_id, agent) in registrations {
            self.register_discovered_agent(agent_id, agent).await?;
        }
        Ok(())
    }
}
```

- [ ] **Step 4: Update imports**

In moved files, change channel crate imports from crate-relative protocol imports to explicit channel-crate imports. For example, change:

```rust
use crate::agent_server_protocol::{AgentServerMessage, Operation, Payload, ProtocolError};
```

to:

```rust
use vol_llm_agent_channel::agent_server_protocol::{AgentServerMessage, Operation, Payload, ProtocolError};
```

Similarly:

```rust
use vol_llm_agent_channel::{Connection, DomainHandler, HandlerRegistry, JsonRpcMessageService};
```

For local moved types:

```rust
use crate::data_plane::router::AgentRouter;
use crate::data_plane::dispatcher::AgentDispatcher;
use crate::data_plane::connection_holder::ConnectionHolder;
```

- [ ] **Step 5: Implement `JsonRpcMessageService` for `DataPlaneServerCore`**

In `data_plane/core.rs`:

```rust
#[async_trait::async_trait]
impl vol_llm_agent_channel::JsonRpcMessageService for DataPlaneServerCore {
    async fn serve_connection(&self, conn: std::sync::Arc<dyn vol_llm_agent_channel::Connection>) {
        self.serve(conn).await;
    }
}
```

If the moved `serve` method still takes a generic type, change it to:

```rust
pub async fn serve(&self, conn: std::sync::Arc<dyn vol_llm_agent_channel::Connection>) {
    // existing receive -> handle -> send loop
}
```

- [ ] **Step 6: Remove concrete exports from channel**

In `crates/vol-llm-agent-channel/src/lib.rs`, stop exporting moved concrete execution types:

```rust
// remove after move
// pub mod dispatcher;
// pub mod router;
// pub mod server_core;
// pub use connection::{Connection, ConnectionHolder};
// pub use dispatcher::AgentDispatcher;
// pub use router::AgentRouter;
// pub use server_core::AgentServerCore;
```

Keep:

```rust
pub use connection::Connection;
pub use domain::handler::DomainHandler;
pub use domain::registry::HandlerRegistry;
pub use service::JsonRpcMessageService;
```

If `ConnectionHolder` still lives in `connection.rs`, split it before removing export.

- [ ] **Step 7: Update `vol-agent-server` dependencies**

In `crates/vol-agent-server/Cargo.toml`, add dependencies that moved data-plane files need and were previously only in channel. Include exact workspace crate dependencies already used by channel core, such as:

```toml
vol-llm-agent = { path = "../vol-llm-agent" }
vol-llm-core = { path = "../vol-llm-core" }
vol-llm-tool = { path = "../vol-llm-tool" }
vol-llm-mcp = { path = "../vol-llm-mcp" }
vol-llm-skill = { path = "../vol-llm-skill" }
vol-llm-sandbox = { path = "../vol-llm-sandbox" }
vol-session = { path = "../vol-session" }
```

Only add crates that compiler errors show are needed.

- [ ] **Step 8: Update main to use `DataPlaneServerCore`**

In `crates/vol-agent-server/src/main.rs`, replace:

```rust
use vol_llm_agent_channel::{AgentServerCore, JsonRpcServer};
```

with:

```rust
use vol_llm_agent_channel::JsonRpcServer;
use crate::data_plane::DataPlaneServerCore;
```

Change builder call:

```rust
let core = DataPlaneServerCore::builder(&config.runtime.working_dir, &config.runtime.store_dir)
```

Change startup log from `AgentServerCore` to `DataPlaneServerCore`.

- [ ] **Step 9: Run compile checks**

Run:

```bash
cargo check -p vol-llm-agent-channel
cargo check -p vol-agent-server
```

Expected: both pass.

- [ ] **Step 10: Run data-plane behavior tests**

Run:

```bash
cargo test -p vol-llm-agent-channel
cargo test -p vol-agent-server
```

Expected: pass. If tests that belong to moved files now live in server, move the tests with the files and update imports.

- [ ] **Step 11: Commit**

```bash
git add crates/vol-llm-agent-channel crates/vol-agent-server
git commit -m "refactor(server): move data plane core into agent server"
```

---

## Task 5: Add Control-Plane State and Registry

**Files:**
- Create: `crates/vol-agent-server/src/control_plane/mod.rs`
- Create: `crates/vol-agent-server/src/control_plane/state.rs`
- Create: `crates/vol-agent-server/src/control_plane/registry.rs`
- Create: `crates/vol-agent-server/src/control_plane/capability.rs`
- Create: `crates/vol-agent-server/src/control_plane/store.rs`
- Create: `crates/vol-agent-server/src/control_plane/event.rs`
- Modify: `crates/vol-agent-server/src/main.rs` or `lib.rs` module list

- [ ] **Step 1: Write failing registry tests**

Create `crates/vol-agent-server/src/control_plane/registry.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_agent_channel::agent_server_protocol::{NodeLoad, NodeRegistration};

    #[test]
    fn register_creates_node_record_and_session_generation() {
        let registry = NodeRegistry::new();
        let reg = NodeRegistration {
            node_id: "node-a".to_string(),
            name: "Node A".to_string(),
            version: "0.1.0".to_string(),
        };

        let ack = registry.register(reg, "auth-a".to_string(), 1000).unwrap();
        assert_eq!(ack.node_id, "node-a");
        assert_eq!(ack.generation, 1);

        let node = registry.get("node-a").unwrap();
        assert_eq!(node.node_id, "node-a");
        assert_eq!(node.status, "online");
    }

    #[test]
    fn heartbeat_updates_last_seen_and_load() {
        let registry = NodeRegistry::new();
        registry.register(NodeRegistration {
            node_id: "node-a".to_string(),
            name: "Node A".to_string(),
            version: "0.1.0".to_string(),
        }, "auth-a".to_string(), 1000).unwrap();

        registry.heartbeat("node-a", NodeLoad { running: 2, queued: 3 }, 2000).unwrap();

        let node = registry.get("node-a").unwrap();
        assert_eq!(node.last_seen_at_ms, Some(2000));
        assert_eq!(node.load.running, 2);
        assert_eq!(node.load.queued, 3);
    }
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
cargo test -p vol-agent-server control_plane::registry
```

Expected: compile failure because module/types do not exist.

- [ ] **Step 3: Implement registry**

Create `crates/vol-agent-server/src/control_plane/mod.rs`:

```rust
pub mod capability;
pub mod event;
pub mod registry;
pub mod state;
pub mod store;
```

Add `mod control_plane;` to `main.rs` or `lib.rs` depending on crate structure.

Implement `registry.rs`:

```rust
use std::collections::HashMap;
use std::sync::RwLock;

use vol_llm_agent_channel::agent_server_protocol::{NodeLoad, NodeRecord, NodeRegistration, RegisterAck};

#[derive(Debug, Clone)]
struct NodeAuth {
    identity: String,
    generation: u64,
}

pub struct NodeRegistry {
    nodes: RwLock<HashMap<String, NodeRecord>>,
    auth: RwLock<HashMap<String, NodeAuth>>,
}

impl NodeRegistry {
    pub fn new() -> Self {
        Self {
            nodes: RwLock::new(HashMap::new()),
            auth: RwLock::new(HashMap::new()),
        }
    }

    pub fn register(
        &self,
        reg: NodeRegistration,
        auth_identity: String,
        now_ms: u64,
    ) -> Result<RegisterAck, String> {
        let mut auth = self.auth.write().unwrap();
        let generation = match auth.get(&reg.node_id) {
            Some(existing) if existing.identity != auth_identity => {
                return Err("node_id already registered with different auth identity".to_string());
            }
            Some(existing) => existing.generation + 1,
            None => 1,
        };
        auth.insert(reg.node_id.clone(), NodeAuth { identity: auth_identity, generation });

        let mut nodes = self.nodes.write().unwrap();
        nodes.insert(reg.node_id.clone(), NodeRecord {
            node_id: reg.node_id.clone(),
            name: reg.name,
            version: reg.version,
            status: "online".to_string(),
            last_seen_at_ms: Some(now_ms),
            capability_revision: 0,
            load: NodeLoad::default(),
        });

        Ok(RegisterAck { node_id: reg.node_id, accepted: true, generation })
    }

    pub fn heartbeat(&self, node_id: &str, load: NodeLoad, now_ms: u64) -> Result<(), String> {
        let mut nodes = self.nodes.write().unwrap();
        let node = nodes
            .get_mut(node_id)
            .ok_or_else(|| "node_not_registered".to_string())?;
        node.status = "online".to_string();
        node.last_seen_at_ms = Some(now_ms);
        node.load = load;
        Ok(())
    }

    pub fn get(&self, node_id: &str) -> Option<NodeRecord> {
        self.nodes.read().unwrap().get(node_id).cloned()
    }

    pub fn list(&self) -> Vec<NodeRecord> {
        self.nodes.read().unwrap().values().cloned().collect()
    }
}

impl Default for NodeRegistry {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 4: Implement capability index**

Create `capability.rs`:

```rust
use std::collections::HashMap;
use std::sync::RwLock;

use vol_llm_agent_channel::agent_server_protocol::CapabilitySnapshot;

pub struct CapabilityIndex {
    snapshots: RwLock<HashMap<String, CapabilitySnapshot>>,
}

impl CapabilityIndex {
    pub fn new() -> Self {
        Self { snapshots: RwLock::new(HashMap::new()) }
    }

    pub fn apply_snapshot(&self, snapshot: CapabilitySnapshot) -> Result<(), String> {
        let mut snapshots = self.snapshots.write().unwrap();
        if let Some(existing) = snapshots.get(&snapshot.node_id) {
            if snapshot.revision <= existing.revision {
                return Err("stale_capability_snapshot".to_string());
            }
        }
        snapshots.insert(snapshot.node_id.clone(), snapshot);
        Ok(())
    }

    pub fn list(&self, node_id: Option<&str>) -> Vec<CapabilitySnapshot> {
        let snapshots = self.snapshots.read().unwrap();
        match node_id {
            Some(id) => snapshots.get(id).cloned().into_iter().collect(),
            None => snapshots.values().cloned().collect(),
        }
    }
}

impl Default for CapabilityIndex {
    fn default() -> Self { Self::new() }
}
```

- [ ] **Step 5: Implement state/store/event skeletons**

Create `state.rs`:

```rust
use std::sync::Arc;

use super::capability::CapabilityIndex;
use super::event::EventBus;
use super::registry::NodeRegistry;
use super::store::{CommandStore, RunStore};

#[derive(Clone)]
pub struct ControlPlaneState {
    pub nodes: Arc<NodeRegistry>,
    pub capabilities: Arc<CapabilityIndex>,
    pub events: EventBus,
    pub commands: Arc<CommandStore>,
    pub runs: Arc<RunStore>,
}

impl ControlPlaneState {
    pub fn new() -> Self {
        Self {
            nodes: Arc::new(NodeRegistry::new()),
            capabilities: Arc::new(CapabilityIndex::new()),
            events: EventBus::new(),
            commands: Arc::new(CommandStore::new()),
            runs: Arc::new(RunStore::new()),
        }
    }
}

impl Default for ControlPlaneState {
    fn default() -> Self { Self::new() }
}
```

Create `event.rs`:

```rust
use tokio::sync::broadcast;

#[derive(Debug, Clone)]
pub struct ControlPlaneEvent {
    pub event_type: String,
    pub node_id: Option<String>,
}

#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<ControlPlaneEvent>,
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(256);
        Self { tx }
    }

    pub fn publish(&self, event: ControlPlaneEvent) {
        let _ = self.tx.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ControlPlaneEvent> {
        self.tx.subscribe()
    }
}
```

Create `store.rs`:

```rust
use std::collections::HashMap;
use std::sync::RwLock;

#[derive(Debug, Clone)]
pub struct CommandRecord {
    pub command_id: String,
    pub node_id: String,
    pub operation_kind: String,
    pub status: String,
    pub run_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RunRecord {
    pub run_id: String,
    pub command_id: Option<String>,
    pub node_id: String,
    pub agent_id: String,
    pub status: String,
}

pub struct CommandStore {
    records: RwLock<HashMap<String, CommandRecord>>,
}

impl CommandStore {
    pub fn new() -> Self { Self { records: RwLock::new(HashMap::new()) } }
    pub fn insert(&self, record: CommandRecord) { self.records.write().unwrap().insert(record.command_id.clone(), record); }
    pub fn get(&self, command_id: &str) -> Option<CommandRecord> { self.records.read().unwrap().get(command_id).cloned() }
}

pub struct RunStore {
    records: RwLock<HashMap<String, RunRecord>>,
}

impl RunStore {
    pub fn new() -> Self { Self { records: RwLock::new(HashMap::new()) } }
    pub fn insert(&self, record: RunRecord) { self.records.write().unwrap().insert(record.run_id.clone(), record); }
    pub fn get(&self, run_id: &str) -> Option<RunRecord> { self.records.read().unwrap().get(run_id).cloned() }
}
```

- [ ] **Step 6: Run tests**

Run:

```bash
cargo test -p vol-agent-server control_plane::registry
cargo check -p vol-agent-server
```

Expected: pass.

- [ ] **Step 7: Commit**

```bash
git add crates/vol-agent-server/src/control_plane crates/vol-agent-server/src/main.rs
git commit -m "feat(server): add control plane state registry"
```

---

## Task 6: Add Control-Plane Core and Handlers

**Files:**
- Create: `crates/vol-agent-server/src/control_plane/core.rs`
- Create: `crates/vol-agent-server/src/control_plane/handlers/mod.rs`
- Create: `crates/vol-agent-server/src/control_plane/handlers/control.rs`
- Create: `crates/vol-agent-server/src/control_plane/handlers/node.rs`
- Create: `crates/vol-agent-server/src/control_plane/handlers/capability.rs`

- [ ] **Step 1: Write failing handler tests**

In `control_plane/handlers/control.rs`, add tests that construct the handler and send `control.register`:

```rust
#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use vol_llm_agent_channel::agent_server_protocol::{
        AgentServerMessage, ControlOperation, ControlPayload, MessageKind, Operation, Payload,
        NodeRegistration,
    };
    use vol_llm_agent_channel::DomainHandler;

    use crate::control_plane::state::ControlPlaneState;
    use super::ControlHandler;

    #[tokio::test]
    async fn control_register_creates_node() {
        let state = Arc::new(ControlPlaneState::new());
        let handler = ControlHandler::new(state.clone());
        let msg = AgentServerMessage {
            protocol: "agent-server/1".to_string(),
            message_id: "1".to_string(),
            sender: "node-a".to_string(),
            receiver: "control".to_string(),
            kind: MessageKind::Command,
            operation: Operation::Control(ControlOperation::Register),
            payload: Payload::Control(ControlPayload::Register(NodeRegistration {
                node_id: "node-a".to_string(),
                name: "Node A".to_string(),
                version: "0.1.0".to_string(),
            })),
            meta: Default::default(),
        };

        let replies = handler.handle(msg).await.unwrap();
        assert_eq!(replies.len(), 1);
        assert!(state.nodes.get("node-a").is_some());
    }
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
cargo test -p vol-agent-server control_register_creates_node
```

Expected: compile failure because handler/core modules do not exist.

- [ ] **Step 3: Implement control handler**

Create `handlers/mod.rs`:

```rust
pub mod capability;
pub mod control;
pub mod node;
```

Create `handlers/control.rs`:

```rust
use std::sync::Arc;

use async_trait::async_trait;
use vol_llm_agent_channel::agent_server_protocol::{
    AgentServerMessage, ControlOperation, ControlPayload, MessageKind, Operation, Payload,
    ProtocolError,
};
use vol_llm_agent_channel::DomainHandler;

use crate::control_plane::state::ControlPlaneState;

pub struct ControlHandler {
    state: Arc<ControlPlaneState>,
}

impl ControlHandler {
    pub fn new(state: Arc<ControlPlaneState>) -> Self {
        Self { state }
    }
}

#[async_trait]
impl DomainHandler for ControlHandler {
    fn name(&self) -> &str { "control" }

    fn operations(&self) -> Vec<Operation> {
        vec![
            Operation::Control(ControlOperation::Register),
            Operation::Control(ControlOperation::Heartbeat),
            Operation::Control(ControlOperation::CapabilitySnapshot),
            Operation::Control(ControlOperation::CapabilityDelta),
            Operation::Control(ControlOperation::Event),
            Operation::Control(ControlOperation::CommandResult),
        ]
    }

    async fn handle(&self, message: AgentServerMessage) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        match (message.operation.clone(), message.payload.clone()) {
            (Operation::Control(ControlOperation::Register), Payload::Control(ControlPayload::Register(reg))) => {
                let ack = self.state.nodes.register(reg, message.sender.clone(), now_ms())
                    .map_err(ProtocolError::PayloadDecodeFailedOwned)?;
                Ok(vec![AgentServerMessage {
                    protocol: "agent-server/1".to_string(),
                    message_id: message.message_id,
                    sender: "control".to_string(),
                    receiver: message.sender,
                    kind: MessageKind::Result,
                    operation: Operation::Control(ControlOperation::Register),
                    payload: Payload::Control(ControlPayload::RegisterAck(ack)),
                    meta: Default::default(),
                }])
            }
            (Operation::Control(ControlOperation::Heartbeat), Payload::Control(ControlPayload::Heartbeat(hb))) => {
                self.state.nodes.heartbeat(&hb.node_id, hb.load, now_ms())
                    .map_err(ProtocolError::PayloadDecodeFailedOwned)?;
                Ok(vec![])
            }
            (Operation::Control(ControlOperation::CapabilitySnapshot), Payload::Control(ControlPayload::CapabilitySnapshot(snapshot))) => {
                self.state.capabilities.apply_snapshot(snapshot)
                    .map_err(ProtocolError::PayloadDecodeFailedOwned)?;
                Ok(vec![])
            }
            _ => Err(ProtocolError::PayloadDecodeFailedOwned("unsupported control operation".to_string())),
        }
    }
}

fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}
```

- [ ] **Step 4: Implement client-facing node/capability handlers**

Create `handlers/node.rs`:

```rust
use std::sync::Arc;

use async_trait::async_trait;
use vol_llm_agent_channel::agent_server_protocol::{
    AgentServerMessage, ControlOperation, ControlPayload, MessageKind, Operation, Payload,
    ProtocolError,
};
use vol_llm_agent_channel::DomainHandler;

use crate::control_plane::state::ControlPlaneState;

pub struct NodeHandler { state: Arc<ControlPlaneState> }
impl NodeHandler { pub fn new(state: Arc<ControlPlaneState>) -> Self { Self { state } } }

#[async_trait]
impl DomainHandler for NodeHandler {
    fn name(&self) -> &str { "node" }
    fn operations(&self) -> Vec<Operation> {
        vec![Operation::Control(ControlOperation::NodeList), Operation::Control(ControlOperation::NodeGet)]
    }
    async fn handle(&self, message: AgentServerMessage) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        match message.operation.clone() {
            Operation::Control(ControlOperation::NodeList) => {
                let result = ControlPayload::NodeListResult(vol_llm_agent_channel::agent_server_protocol::NodeListResult {
                    nodes: self.state.nodes.list(),
                });
                Ok(vec![AgentServerMessage {
                    protocol: "agent-server/1".to_string(),
                    message_id: message.message_id,
                    sender: "control".to_string(),
                    receiver: message.sender,
                    kind: MessageKind::Result,
                    operation: Operation::Control(ControlOperation::NodeList),
                    payload: Payload::Control(result),
                    meta: Default::default(),
                }])
            }
            _ => Err(ProtocolError::PayloadDecodeFailedOwned("unsupported node operation".to_string())),
        }
    }
}
```

Create `handlers/capability.rs` similarly for `ControlOperation::CapabilityList`:

```rust
use std::sync::Arc;

use async_trait::async_trait;
use vol_llm_agent_channel::agent_server_protocol::{
    AgentServerMessage, CapabilityListResult, ControlOperation, ControlPayload, MessageKind,
    Operation, Payload, ProtocolError,
};
use vol_llm_agent_channel::DomainHandler;

use crate::control_plane::state::ControlPlaneState;

pub struct CapabilityHandler { state: Arc<ControlPlaneState> }
impl CapabilityHandler { pub fn new(state: Arc<ControlPlaneState>) -> Self { Self { state } } }

#[async_trait]
impl DomainHandler for CapabilityHandler {
    fn name(&self) -> &str { "capability" }
    fn operations(&self) -> Vec<Operation> { vec![Operation::Control(ControlOperation::CapabilityList)] }
    async fn handle(&self, message: AgentServerMessage) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        let result = CapabilityListResult { snapshots: self.state.capabilities.list(None) };
        Ok(vec![AgentServerMessage {
            protocol: "agent-server/1".to_string(),
            message_id: message.message_id,
            sender: "control".to_string(),
            receiver: message.sender,
            kind: MessageKind::Result,
            operation: Operation::Control(ControlOperation::CapabilityList),
            payload: Payload::Control(ControlPayload::CapabilityListResult(result)),
            meta: Default::default(),
        }])
    }
}
```

- [ ] **Step 5: Implement `ControlPlaneServerCore`**

Create `control_plane/core.rs`:

```rust
use std::sync::Arc;

use vol_llm_agent_channel::{Connection, HandlerRegistry, JsonRpcMessageService, ProtocolError};
use vol_llm_agent_channel::agent_server_protocol::AgentServerMessage;

use crate::control_plane::handlers::capability::CapabilityHandler;
use crate::control_plane::handlers::control::ControlHandler;
use crate::control_plane::handlers::node::NodeHandler;
use crate::control_plane::state::ControlPlaneState;

pub struct ControlPlaneServerCore {
    pub state: Arc<ControlPlaneState>,
    handler_registry: HandlerRegistry,
}

impl ControlPlaneServerCore {
    pub fn new(state: Arc<ControlPlaneState>) -> Result<Self, String> {
        let mut handler_registry = HandlerRegistry::new();
        handler_registry.register(Arc::new(ControlHandler::new(state.clone())))?;
        handler_registry.register(Arc::new(NodeHandler::new(state.clone())))?;
        handler_registry.register(Arc::new(CapabilityHandler::new(state.clone())))?;
        Ok(Self { state, handler_registry })
    }

    pub async fn handle(&self, message: AgentServerMessage) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        self.handler_registry.dispatch(message).await
    }
}

#[async_trait::async_trait]
impl JsonRpcMessageService for ControlPlaneServerCore {
    async fn serve_connection(&self, conn: Arc<dyn Connection>) {
        while let Some(next) = conn.recv().await {
            match next {
                Ok(message) => {
                    match self.handle(message).await {
                        Ok(replies) => {
                            for reply in replies {
                                let _ = conn.send(reply).await;
                            }
                        }
                        Err(err) => {
                            tracing::warn!("control-plane handler error: {err}");
                        }
                    }
                }
                Err(err) => {
                    tracing::warn!("control-plane connection error: {err}");
                    break;
                }
            }
        }
    }
}
```

- [ ] **Step 6: Run tests**

Run:

```bash
cargo test -p vol-agent-server control_register_creates_node
cargo check -p vol-agent-server
```

Expected: pass.

- [ ] **Step 7: Commit**

```bash
git add crates/vol-agent-server/src/control_plane
git commit -m "feat(server): add control plane core handlers"
```

---

## Task 7: Compose Routes by Role

**Files:**
- Modify: `crates/vol-agent-server/src/app.rs`
- Modify: `crates/vol-agent-server/src/routes.rs`
- Modify: `crates/vol-agent-server/src/main.rs`

- [ ] **Step 1: Write route selection tests**

In `routes.rs`, add a pure function and tests:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WsOwner {
    DataPlane,
    ControlPlane,
}

pub fn ws_owner(control_plane: bool, data_plane: bool) -> Result<WsOwner, String> {
    match (control_plane, data_plane) {
        (false, true) => Ok(WsOwner::DataPlane),
        (true, false) | (true, true) => Ok(WsOwner::ControlPlane),
        (false, false) => Err("at least one server role must be enabled".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_plane_owns_ws_only_when_control_plane_disabled() {
        assert_eq!(ws_owner(false, true).unwrap(), WsOwner::DataPlane);
    }

    #[test]
    fn control_plane_owns_ws_when_enabled() {
        assert_eq!(ws_owner(true, false).unwrap(), WsOwner::ControlPlane);
        assert_eq!(ws_owner(true, true).unwrap(), WsOwner::ControlPlane);
    }

    #[test]
    fn both_roles_disabled_is_error() {
        assert!(ws_owner(false, false).is_err());
    }
}
```

- [ ] **Step 2: Run tests**

Run:

```bash
cargo test -p vol-agent-server routes::tests
```

Expected: pass after adding function/tests.

- [ ] **Step 3: Implement role composition in `app.rs`**

Replace skeleton `app.rs` with:

```rust
use std::sync::Arc;

use axum::Router;
use vol_llm_agent_channel::JsonRpcServer;

use crate::config::ServerConfig;
use crate::control_plane::core::ControlPlaneServerCore;
use crate::control_plane::state::ControlPlaneState;
use crate::data_plane::DataPlaneServerCore;

pub async fn run(mut config: ServerConfig) -> Result<(), String> {
    config.expand_tilde();

    let mut app = crate::routes::base_router();

    let control_core = if config.server.roles.control_plane {
        Some(Arc::new(ControlPlaneServerCore::new(Arc::new(ControlPlaneState::new()))?))
    } else {
        None
    };

    let data_core = if config.server.roles.data_plane {
        let core = DataPlaneServerCore::builder(&config.runtime.working_dir, &config.runtime.store_dir)
            .with_task_store_config(config.runtime.task_store.clone())
            .with_session_store_config(config.runtime.session_store.clone())
            .build()
            .await?;
        core.discover_agents().await?;
        Some(Arc::new(core))
    } else {
        None
    };

    if let Some(control) = control_core.clone() {
        app = app.merge(JsonRpcServer::new(control.clone(), "/ws").into_axum_router());
        app = app.merge(JsonRpcServer::new(control, "/control/v1/ws").into_axum_router());
    } else if let Some(data) = data_core {
        app = app.merge(JsonRpcServer::new(data, "/ws").into_axum_router());
    }

    let addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| format!("failed to bind {addr}: {e}"))?;
    tracing::info!("agent server started on {}", addr);
    axum::serve(listener, app)
        .await
        .map_err(|e| format!("server error: {e}"))
}
```

- [ ] **Step 4: Simplify `main.rs`**

Replace code after config/tracing setup with:

```rust
if let Err(err) = app::run(config).await {
    tracing::error!("Server error: {}", err);
    std::process::exit(1);
}
```

Add module declarations:

```rust
mod app;
mod control_plane;
mod data_plane;
mod health;
mod routes;
```

- [ ] **Step 5: Run checks**

Run:

```bash
cargo test -p vol-agent-server routes::tests
cargo check -p vol-agent-server
```

Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add crates/vol-agent-server/src/app.rs \
  crates/vol-agent-server/src/routes.rs \
  crates/vol-agent-server/src/main.rs
git commit -m "feat(server): compose jsonrpc routes by role"
```

---

## Task 8: Add Data-Plane Snapshot and Command Skeleton

**Files:**
- Create: `crates/vol-agent-server/src/data_plane/snapshot.rs`
- Create: `crates/vol-agent-server/src/data_plane/command.rs`
- Modify: `crates/vol-agent-server/src/data_plane/mod.rs`

- [ ] **Step 1: Write snapshot tests with fake source**

In `snapshot.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    struct FakeSource;

    #[async_trait::async_trait]
    impl RuntimeCapabilitySource for FakeSource {
        async fn snapshot_capabilities(&self) -> CapabilitySnapshot {
            CapabilitySnapshot {
                node_id: "node-a".to_string(),
                revision: 1,
                generated_at_ms: Some(1000),
                agents: vec![AgentCapability {
                    agent_id: "coding".to_string(),
                    name: "coding".to_string(),
                    description: Some("Coding agent".to_string()),
                    status: Some("idle".to_string()),
                }],
                tools: vec![],
                mcp_servers: vec![],
                skills: vec![],
            }
        }

        async fn current_load(&self) -> NodeLoad {
            NodeLoad { running: 0, queued: 0 }
        }
    }

    #[tokio::test]
    async fn fake_source_returns_snapshot() {
        let snapshot = FakeSource.snapshot_capabilities().await;
        assert_eq!(snapshot.node_id, "node-a");
        assert_eq!(snapshot.revision, 1);
        assert_eq!(snapshot.agents[0].agent_id, "coding");
    }
}
```

- [ ] **Step 2: Run test to verify failure**

Run:

```bash
cargo test -p vol-agent-server fake_source_returns_snapshot
```

Expected: compile failure until trait/imports exist.

- [ ] **Step 3: Implement `RuntimeCapabilitySource`**

Create `snapshot.rs`:

```rust
use vol_llm_agent_channel::agent_server_protocol::{
    AgentCapability, CapabilitySnapshot, NodeLoad,
};

#[async_trait::async_trait]
pub trait RuntimeCapabilitySource {
    async fn snapshot_capabilities(&self) -> CapabilitySnapshot;
    async fn current_load(&self) -> NodeLoad;
}

pub struct StaticCapabilitySource {
    pub node_id: String,
}

#[async_trait::async_trait]
impl RuntimeCapabilitySource for StaticCapabilitySource {
    async fn snapshot_capabilities(&self) -> CapabilitySnapshot {
        CapabilitySnapshot {
            node_id: self.node_id.clone(),
            revision: 1,
            generated_at_ms: None,
            agents: Vec::<AgentCapability>::new(),
            tools: vec![],
            mcp_servers: vec![],
            skills: vec![],
        }
    }

    async fn current_load(&self) -> NodeLoad {
        NodeLoad { running: 0, queued: 0 }
    }
}
```

Later tasks can replace `StaticCapabilitySource` with a real `DataPlaneServerCore` implementation.

- [ ] **Step 4: Implement command skeleton**

Create `command.rs`:

```rust
use vol_llm_agent_channel::agent_server_protocol::{CommandAck, ControlCommand, ControlCommandOperation};

pub async fn accept_control_command(command: &ControlCommand) -> CommandAck {
    let run_id = match &command.operation {
        ControlCommandOperation::SubmitAgent { .. } => Some(format!("run_{}", command.command_id)),
        _ => None,
    };

    CommandAck {
        command_id: command.command_id.clone(),
        accepted: true,
        run_id,
    }
}
```

- [ ] **Step 5: Run tests**

Run:

```bash
cargo test -p vol-agent-server fake_source_returns_snapshot
cargo check -p vol-agent-server
```

Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add crates/vol-agent-server/src/data_plane/snapshot.rs \
  crates/vol-agent-server/src/data_plane/command.rs \
  crates/vol-agent-server/src/data_plane/mod.rs
git commit -m "feat(server): add data plane reporting primitives"
```

---

## Task 9: Add Control Router MVP

**Files:**
- Create: `crates/vol-agent-server/src/control_plane/router.rs`
- Modify: `crates/vol-agent-server/src/control_plane/mod.rs`

- [ ] **Step 1: Write router tests**

Create `router.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::control_plane::capability::CapabilityIndex;
    use crate::control_plane::registry::NodeRegistry;
    use vol_llm_agent_channel::agent_server_protocol::{
        AgentCapability, CapabilitySnapshot, NodeRegistration,
    };

    #[test]
    fn route_agent_prefers_node_with_agent_capability() {
        let nodes = NodeRegistry::new();
        nodes.register(NodeRegistration {
            node_id: "node-a".to_string(),
            name: "Node A".to_string(),
            version: "0.1.0".to_string(),
        }, "auth-a".to_string(), 1000).unwrap();

        let capabilities = CapabilityIndex::new();
        capabilities.apply_snapshot(CapabilitySnapshot {
            node_id: "node-a".to_string(),
            revision: 1,
            generated_at_ms: Some(1000),
            agents: vec![AgentCapability {
                agent_id: "coding".to_string(),
                name: "coding".to_string(),
                description: None,
                status: Some("idle".to_string()),
            }],
            tools: vec![],
            mcp_servers: vec![],
            skills: vec![],
        }).unwrap();

        let router = ControlRouter::new(&nodes, &capabilities);
        assert_eq!(router.route_agent(Some("coding")).unwrap(), "node-a");
    }
}
```

- [ ] **Step 2: Run test to verify failure**

Run:

```bash
cargo test -p vol-agent-server route_agent_prefers_node_with_agent_capability
```

Expected: compile failure until `ControlRouter` exists.

- [ ] **Step 3: Implement router**

Implement `router.rs`:

```rust
use crate::control_plane::capability::CapabilityIndex;
use crate::control_plane::registry::NodeRegistry;

pub struct ControlRouter<'a> {
    nodes: &'a NodeRegistry,
    capabilities: &'a CapabilityIndex,
}

impl<'a> ControlRouter<'a> {
    pub fn new(nodes: &'a NodeRegistry, capabilities: &'a CapabilityIndex) -> Self {
        Self { nodes, capabilities }
    }

    pub fn route_agent(&self, target: Option<&str>) -> Result<String, String> {
        let snapshots = self.capabilities.list(None);
        for snapshot in snapshots {
            if self.nodes.get(&snapshot.node_id).map(|n| n.status == "online").unwrap_or(false) {
                if let Some(target) = target {
                    if snapshot.agents.iter().any(|a| a.agent_id == target || a.name == target) {
                        return Ok(snapshot.node_id);
                    }
                } else if !snapshot.agents.is_empty() {
                    return Ok(snapshot.node_id);
                }
            }
        }
        Err("capability_not_found".to_string())
    }
}
```

Add `pub mod router;` to `control_plane/mod.rs`.

- [ ] **Step 4: Run tests**

Run:

```bash
cargo test -p vol-agent-server route_agent_prefers_node_with_agent_capability
```

Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-agent-server/src/control_plane/router.rs \
  crates/vol-agent-server/src/control_plane/mod.rs
git commit -m "feat(server): add control plane routing MVP"
```

---

## Task 10: Boundary and Mode Verification Tests

**Files:**
- Create: `crates/vol-agent-server/tests/role_modes.rs`
- Create: `scripts/check-agent-boundaries.sh`
- Modify: `crates/vol-agent-server/src/lib.rs` only when integration tests need library exports from the binary crate

- [ ] **Step 1: Add boundary script**

Create `scripts/check-agent-boundaries.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

if cargo tree -p vol-llm-agent-channel | grep -q 'vol-agent-server'; then
  echo 'ERROR: vol-llm-agent-channel must not depend on vol-agent-server' >&2
  exit 1
fi

if cargo tree -p vol-llm-runtime | grep -q 'vol-agent-server'; then
  echo 'ERROR: vol-llm-runtime must not depend on vol-agent-server' >&2
  exit 1
fi

echo 'agent boundary checks passed'
```

Make executable:

```bash
chmod +x scripts/check-agent-boundaries.sh
```

- [ ] **Step 2: Add role mode tests**

Create `crates/vol-agent-server/tests/role_modes.rs`:

```rust
use vol_agent_server::config::ServerConfig;
use vol_agent_server::routes::{ws_owner, WsOwner};

#[test]
fn standalone_data_plane_routes_ws_to_data_plane() {
    assert_eq!(ws_owner(false, true).unwrap(), WsOwner::DataPlane);
}

#[test]
fn control_plane_routes_ws_to_control_plane() {
    assert_eq!(ws_owner(true, false).unwrap(), WsOwner::ControlPlane);
    assert_eq!(ws_owner(true, true).unwrap(), WsOwner::ControlPlane);
}

#[test]
fn both_roles_disabled_rejected_by_config() {
    let toml_str = r#"
        [server.roles]
        control_plane = false
        data_plane = false
    "#;
    let config: ServerConfig = toml::from_str(toml_str).unwrap();
    assert!(config.validate().is_err());
}
```

If `vol-agent-server` is binary-only and tests cannot import modules, add `src/lib.rs` with:

```rust
pub mod config;
pub mod routes;
```

Then keep `main.rs` using those modules from the library.

- [ ] **Step 3: Run boundary and mode tests**

Run:

```bash
./scripts/check-agent-boundaries.sh
cargo test -p vol-agent-server --test role_modes
```

Expected:

```text
agent boundary checks passed
```

and all role mode tests pass.

- [ ] **Step 4: Run wider checks**

Run:

```bash
cargo check -p vol-llm-agent-channel
cargo check -p vol-agent-server
cargo test -p vol-llm-agent-channel
cargo test -p vol-agent-server
```

Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add scripts/check-agent-boundaries.sh \
  crates/vol-agent-server/tests/role_modes.rs \
  crates/vol-agent-server/src/lib.rs \
  crates/vol-agent-server/src/routes.rs \
  crates/vol-agent-server/src/config.rs
git commit -m "test(server): verify agent server boundaries and role modes"
```

---

## Task 11: Documentation and Wiki Update

**Files:**
- Modify: `docs/superpowers/architectures/2026-06-10-agent-server-control-data-plane.md`
- Modify: `docs/superpowers/specs/2026-06-10-agent-server-control-data-plane-addendum.md`
- Update: `docs/wiki/*` through `wiki-ingest`

- [ ] **Step 1: Update architecture docs with implementation results**

After implementation, update the architecture/spec docs to replace future-tense claims with implemented paths. Add exact module paths:

```markdown
Implemented module paths:

- `vol_llm_agent_channel::service::JsonRpcMessageService`
- `vol_llm_agent_channel::transport::jsonrpc::JsonRpcServer<S>`
- `vol_agent_server::data_plane::DataPlaneServerCore`
- `vol_agent_server::control_plane::ControlPlaneServerCore`
```

- [ ] **Step 2: Run wiki ingest**

Use skill `wiki-ingest` with a summary of the code changes and docs changed.

- [ ] **Step 3: Upload changed superpowers docs to Feishu/Lark**

For the addendum spec, update the existing Feishu document:

```bash
lark-cli docs +update \
  --api-version v2 \
  --doc "https://my.feishu.cn/docx/Rk11ddyFJoC6q2x8HOjcrwuQn4c" \
  --command overwrite \
  --doc-format markdown \
  --content @docs/superpowers/specs/2026-06-10-agent-server-control-data-plane-addendum.md \
  --as user
```

For the architecture doc, update the existing Feishu document:

```bash
lark-cli docs +update \
  --api-version v2 \
  --doc "https://my.feishu.cn/docx/K0mGdhW5UoKL9IxVBwHcQmsxn9c" \
  --command overwrite \
  --doc-format markdown \
  --content @docs/superpowers/architectures/2026-06-10-agent-server-control-data-plane.md \
  --as user
```

- [ ] **Step 4: Final verification**

Run:

```bash
./scripts/check-agent-boundaries.sh
cargo check -p vol-llm-agent-channel
cargo check -p vol-agent-server
cargo test -p vol-llm-agent-channel
cargo test -p vol-agent-server
git status --short
```

Expected: boundary check passes, cargo checks/tests pass, and git status only shows intended docs/code changes.

- [ ] **Step 5: Commit**

```bash
git add docs scripts crates
git commit -m "docs(wiki): ingest agent server control plane implementation"
```

---

## Self-Review Checklist

Spec coverage:

- Endpoint role allowlists: Task 6 and Task 10.
- `control.command` vs run semantics: Task 2 protocol models, Task 8 command skeleton, Task 9 router MVP.
- Capability snapshot revisions: Task 2 models and Task 5 capability index.
- Node record/session split: Task 5 registry. Live session send handles are left for the next implementation slice after the MVP registry and control command skeleton compile.
- Combined mode lifecycle: Task 3 config and Task 7 route composition.
- Runtime capability facade: Task 8.
- Capability policy hints: Task 2 `ToolCapability` fields.
- Error code ownership: Task 2 protocol and Task 6 handler errors. A richer enum can be added after MVP compiles.
- Migration constraints: Task 1, Task 4, Task 10.
- Boundary tests: Task 10.

Known intentional MVP limits:

- `ControlPlaneClient` and real node command delivery are skeleton-level in this plan. The next plan should implement the full client send/receive loop once the core boundary migration compiles.
- `control.subscribe` is not implemented in this plan because the addendum marks it as a future option unless UI needs it.
- Persistent control-plane storage is not implemented; state remains in-memory.
