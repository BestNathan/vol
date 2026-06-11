# Control Plane Behavior Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the behavior gaps left after the first agent-server control/data-plane implementation pass: notifications, endpoint roles, routed client methods, local-node registration, command handling, capability revision tracking, and run status.

**Architecture:** Keep the existing final boundary: `vol-llm-agent-channel` owns JSON-RPC protocol/transport abstractions, `vol-agent-server` owns concrete `DataPlaneServerCore` and `ControlPlaneServerCore`, and `vol-llm-runtime` remains execution-resource owner. This plan adds missing behavior without changing crate boundaries.

**Tech Stack:** Rust, Tokio, Axum WebSocket, Serde JSON, existing `vol-llm-agent-channel`, `vol-agent-server`, `vol-llm-runtime`.

---

## Current Baseline

The first implementation pass compiles and passes focused tests:

- `./scripts/check-agent-boundaries.sh`
- `cargo test -p vol-llm-agent-channel --tests`
- `cargo test -p vol-agent-server --tests`
- `cargo check -p vol-llm-agent-channel -p vol-agent-server`

Known gaps found by final review:

1. JSON-RPC notifications without `id` are rejected.
2. `/ws` and `/control/v1/ws` share the same control-plane core behavior; endpoint role allowlists are missing.
3. Control-plane `/ws` does not implement client-facing `agent.*`, `tool.*`, `mcp.*`, `skill.*`, `task.*`, `session.*` behavior.
4. `DataPlaneServerCore` does not handle `control.command`.
5. Combined mode builds both cores but does not register the local data plane with the local control plane.
6. Applying a capability snapshot does not update `NodeRecord.capability_revision`.
7. `control.run_status` has protocol types but no handler.
8. Config allows `client_ws_path == node_ws_path`, collapsing endpoint role separation.

---

## Target File Structure

### Modify in `vol-llm-agent-channel`

- `crates/vol-llm-agent-channel/src/transport/jsonrpc/codec.rs`
  - Support notification decode when `id` is absent.
  - Preserve responses/errors with ids.
- `crates/vol-llm-agent-channel/src/agent_server_protocol.rs`
  - Support optional message id for notifications or represent notification id consistently.

### Modify/Create in `vol-agent-server`

- `crates/vol-agent-server/src/control_plane/endpoint.rs`
  - New role wrapper around `ControlPlaneServerCore`.
- `crates/vol-agent-server/src/control_plane/core.rs`
  - Add role-aware serving or expose role-aware handle helper.
- `crates/vol-agent-server/src/control_plane/handlers/client.rs`
  - Client-facing handler for minimal catalog/routing methods.
- `crates/vol-agent-server/src/control_plane/handlers/run.rs`
  - `control.run_status` handler.
- `crates/vol-agent-server/src/control_plane/handlers/control.rs`
  - Update capability snapshot handling to sync `NodeRecord.capability_revision`.
- `crates/vol-agent-server/src/control_plane/registry.rs`
  - Add capability revision setter.
- `crates/vol-agent-server/src/control_plane/router.rs`
  - Keep route lookup and expose for client handler.
- `crates/vol-agent-server/src/data_plane/handlers/control.rs`
  - New data-plane handler for inbound `control.command`.
- `crates/vol-agent-server/src/data_plane/handlers/mod.rs`
  - Register control handler module.
- `crates/vol-agent-server/src/data_plane/core.rs`
  - Register data-plane control handler.
- `crates/vol-agent-server/src/data_plane/reporter.rs`
  - Local combined-mode registration helper.
- `crates/vol-agent-server/src/app.rs`
  - Mount role wrappers and start combined-mode local registration task.
- `crates/vol-agent-server/src/config.rs`
  - Reject equal control/client node websocket paths when control plane is enabled.

---

## Task 1: Decode JSON-RPC Notifications Without `id`

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/transport/jsonrpc/codec.rs`
- Modify: `crates/vol-llm-agent-channel/src/agent_server_protocol.rs` only if required by message id representation

- [ ] **Step 1: Add failing notification decode test**

Add to `crates/vol-llm-agent-channel/src/transport/jsonrpc/codec.rs` tests:

```rust
#[test]
fn decode_control_heartbeat_without_id_as_notification() {
    let msg = decode_jsonrpc_frame(
        r#"{"jsonrpc":"2.0","method":"control.heartbeat","params":{"node_id":"node-a","status":"online","load":{"running":1,"queued":0}}}"#,
    )
    .unwrap();

    assert_eq!(msg.kind, MessageKind::Event);
    assert_eq!(msg.operation, Operation::Control(ControlOperation::Heartbeat));
    assert!(msg.message_id.starts_with("notification:"));
}
```

- [ ] **Step 2: Run test and verify it fails**

Run:

```bash
cargo test -p vol-llm-agent-channel decode_control_heartbeat_without_id_as_notification
```

Expected: fails with `missing id`.

- [ ] **Step 3: Implement notification decode**

In `decode_jsonrpc_frame`, change id handling:

```rust
let has_id = envelope.id.is_some();
let message_id = match envelope.id {
    Some(serde_json::Value::Number(n)) => n.to_string(),
    Some(serde_json::Value::String(s)) => s,
    Some(_) => return Err(ConnectionError::ParseError("unsupported id type".into())),
    None => format!("notification:{}", uuid::Uuid::new_v4()),
};
```

Set kind based on id:

```rust
kind: if has_id { MessageKind::Command } else { MessageKind::Event },
```

If `uuid` was removed from channel dependencies, avoid adding it back by using a deterministic fallback:

```rust
None => format!("notification:{}", method),
```

This is acceptable because notifications do not get JSON-RPC responses.

- [ ] **Step 4: Run tests**

Run:

```bash
cargo test -p vol-llm-agent-channel decode_control_heartbeat_without_id_as_notification
cargo test -p vol-llm-agent-channel transport::jsonrpc::codec::tests
cargo fmt --check
```

Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-agent-channel/src/transport/jsonrpc/codec.rs \
  crates/vol-llm-agent-channel/src/agent_server_protocol.rs
git commit -m "fix(channel): decode jsonrpc notifications"
```

---

## Task 2: Add Control Endpoint Role Allowlists

**Files:**
- Create: `crates/vol-agent-server/src/control_plane/endpoint.rs`
- Modify: `crates/vol-agent-server/src/control_plane/mod.rs`
- Modify: `crates/vol-agent-server/src/app.rs`
- Modify: `crates/vol-agent-server/src/config.rs`

- [ ] **Step 1: Add role allowlist tests**

Create `crates/vol-agent-server/src/control_plane/endpoint.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_agent_channel::agent_server_protocol::{AgentOperation, ControlOperation, Operation};

    #[test]
    fn client_endpoint_allows_agent_submit_and_denies_register() {
        assert!(ControlConnectionRole::Client.allows(&Operation::Agent(AgentOperation::Submit)));
        assert!(!ControlConnectionRole::Client.allows(&Operation::Control(ControlOperation::Register)));
    }

    #[test]
    fn node_endpoint_allows_register_and_denies_agent_submit() {
        assert!(ControlConnectionRole::DataPlaneNode.allows(&Operation::Control(ControlOperation::Register)));
        assert!(!ControlConnectionRole::DataPlaneNode.allows(&Operation::Agent(AgentOperation::Submit)));
    }
}
```

- [ ] **Step 2: Run tests and verify failure**

Run:

```bash
cargo test -p vol-agent-server control_plane::endpoint
```

Expected: compile failure until module exists.

- [ ] **Step 3: Implement role enum and allowlist**

Implement in `endpoint.rs`:

```rust
use std::sync::Arc;

use vol_llm_agent_channel::agent_server_protocol::{
    AgentOperation, ControlOperation, McpOperation, Operation, SessionOperation, SkillOperation,
    TaskOperation, ToolOperation,
};
use vol_llm_agent_channel::{Connection, JsonRpcMessageService};

use crate::control_plane::core::ControlPlaneServerCore;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlConnectionRole {
    Client,
    DataPlaneNode,
}

impl ControlConnectionRole {
    pub fn allows(&self, operation: &Operation) -> bool {
        match self {
            ControlConnectionRole::Client => matches!(
                operation,
                Operation::Agent(AgentOperation::List)
                    | Operation::Agent(AgentOperation::Status)
                    | Operation::Agent(AgentOperation::Submit)
                    | Operation::Agent(AgentOperation::Cancel)
                    | Operation::Tool(ToolOperation::List)
                    | Operation::Tool(ToolOperation::Call)
                    | Operation::Mcp(McpOperation::ListServers)
                    | Operation::Mcp(McpOperation::ListTools)
                    | Operation::Mcp(McpOperation::CallTool)
                    | Operation::Mcp(McpOperation::ServerStatus)
                    | Operation::Skill(SkillOperation::List)
                    | Operation::Task(TaskOperation::List)
                    | Operation::Task(TaskOperation::Get)
                    | Operation::Session(SessionOperation::List)
                    | Operation::Session(SessionOperation::Entries)
                    | Operation::Control(ControlOperation::NodeList)
                    | Operation::Control(ControlOperation::NodeGet)
                    | Operation::Control(ControlOperation::CapabilityList)
                    | Operation::Control(ControlOperation::RunStatus)
            ),
            ControlConnectionRole::DataPlaneNode => matches!(
                operation,
                Operation::Control(ControlOperation::Register)
                    | Operation::Control(ControlOperation::Heartbeat)
                    | Operation::Control(ControlOperation::CapabilitySnapshot)
                    | Operation::Control(ControlOperation::CapabilityDelta)
                    | Operation::Control(ControlOperation::Event)
                    | Operation::Control(ControlOperation::CommandResult)
            ),
        }
    }
}

pub struct ControlPlaneEndpoint {
    core: Arc<ControlPlaneServerCore>,
    role: ControlConnectionRole,
}

impl ControlPlaneEndpoint {
    pub fn new(core: Arc<ControlPlaneServerCore>, role: ControlConnectionRole) -> Self {
        Self { core, role }
    }
}

#[async_trait::async_trait]
impl JsonRpcMessageService for ControlPlaneEndpoint {
    async fn serve_connection(&self, conn: Arc<dyn Connection>) {
        self.core.serve_connection_with_role(self.role, conn).await;
    }
}
```

- [ ] **Step 4: Add role-aware core serving**

In `control_plane/core.rs`, add:

```rust
pub async fn serve_connection_with_role(
    &self,
    role: crate::control_plane::endpoint::ControlConnectionRole,
    conn: Arc<dyn Connection>,
) {
    while let Some(next) = conn.recv().await {
        match next {
            Ok(message) => {
                if !role.allows(&message.operation) {
                    let err = AgentServerMessage::new_error(
                        message.message_id,
                        message.operation,
                        ErrorPayload {
                            code: "method_not_allowed_for_role".to_string(),
                            message: "method is not allowed on this endpoint".to_string(),
                            detail: None,
                            terminal: false,
                        },
                    );
                    let _ = conn.send(err).await;
                    continue;
                }
                // reuse existing handle/send logic
            }
            Err(err) => { tracing::warn!("control-plane connection error: {err}"); break; }
        }
    }
}
```

Then implement `JsonRpcMessageService for ControlPlaneServerCore` by delegating to client role:

```rust
self.serve_connection_with_role(ControlConnectionRole::Client, conn).await;
```

- [ ] **Step 5: Mount role wrappers in app.rs**

In `app.rs`, replace direct `JsonRpcServer::new(control.clone(), client_ws_path)` with:

```rust
let client_endpoint = Arc::new(ControlPlaneEndpoint::new(
    control.clone(),
    ControlConnectionRole::Client,
));
app = app.merge(JsonRpcServer::new(client_endpoint, client_ws_path).into_axum_router());
```

For node path:

```rust
let node_endpoint = Arc::new(ControlPlaneEndpoint::new(
    control,
    ControlConnectionRole::DataPlaneNode,
));
app = app.merge(JsonRpcServer::new(node_endpoint, node_ws_path).into_axum_router());
```

- [ ] **Step 6: Reject equal client/node paths**

In `config.rs validate()`, add:

```rust
if self.server.roles.control_plane
    && self.control_plane.client_ws_path == self.control_plane.node_ws_path
{
    return Err("control_plane.client_ws_path and node_ws_path must be different".to_string());
}
```

Add a config test for this exact error.

- [ ] **Step 7: Run tests**

Run:

```bash
cargo test -p vol-agent-server control_plane::endpoint
cargo test -p vol-agent-server config::tests
cargo check -p vol-agent-server
cargo fmt --check
```

Expected: pass.

- [ ] **Step 8: Commit**

```bash
git add crates/vol-agent-server/src/control_plane/endpoint.rs \
  crates/vol-agent-server/src/control_plane/core.rs \
  crates/vol-agent-server/src/control_plane/mod.rs \
  crates/vol-agent-server/src/app.rs \
  crates/vol-agent-server/src/config.rs
git commit -m "feat(server): enforce control endpoint roles"
```

---

## Task 3: Add Minimal Client-Facing Control-Plane Handlers

**Files:**
- Create: `crates/vol-agent-server/src/control_plane/handlers/client.rs`
- Modify: `crates/vol-agent-server/src/control_plane/handlers/mod.rs`
- Modify: `crates/vol-agent-server/src/control_plane/core.rs`

- [ ] **Step 1: Add failing client `agent.list` test**

Create tests in `handlers/client.rs`:

```rust
#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use vol_llm_agent_channel::agent_server_protocol::{
        AgentOperation, AgentServerMessage, MessageKind, Operation, Payload,
    };
    use vol_llm_agent_channel::DomainHandler;

    use crate::control_plane::handlers::client::ClientHandler;
    use crate::control_plane::state::ControlPlaneState;

    #[tokio::test]
    async fn agent_list_returns_empty_list_from_control_plane() {
        let state = Arc::new(ControlPlaneState::new());
        let handler = ClientHandler::new(state);
        let msg = AgentServerMessage {
            protocol: "agent-server/1".to_string(),
            message_id: "1".to_string(),
            sender: "client".to_string(),
            receiver: "control".to_string(),
            kind: MessageKind::Command,
            operation: Operation::Agent(AgentOperation::List),
            payload: Payload::Agent(vol_llm_agent_channel::agent_server_protocol::AgentPayload::List),
            meta: Default::default(),
        };

        let replies = handler.handle(msg).await.unwrap();
        assert_eq!(replies.len(), 1);
        let json = replies[0].payload.data_json();
        assert!(json.get("agents").is_some());
    }
}
```

- [ ] **Step 2: Run test and verify failure**

Run:

```bash
cargo test -p vol-agent-server agent_list_returns_empty_list_from_control_plane
```

Expected: compile failure until handler exists.

- [ ] **Step 3: Implement client handler**

Create `handlers/client.rs`:

```rust
use std::sync::Arc;

use async_trait::async_trait;
use vol_llm_agent_channel::agent_server_protocol::{
    AgentOperation, AgentPayload, AgentServerMessage, MessageKind, Operation, Payload,
    ProtocolError,
};
use vol_llm_agent_channel::DomainHandler;

use crate::control_plane::core::make_result;
use crate::control_plane::state::ControlPlaneState;

pub struct ClientHandler {
    state: Arc<ControlPlaneState>,
}

impl ClientHandler {
    pub fn new(state: Arc<ControlPlaneState>) -> Self { Self { state } }
}

#[async_trait]
impl DomainHandler for ClientHandler {
    fn name(&self) -> &str { "control-client" }

    fn operations(&self) -> Vec<Operation> {
        vec![
            Operation::Agent(AgentOperation::List),
            Operation::Agent(AgentOperation::Status),
            Operation::Agent(AgentOperation::Submit),
        ]
    }

    async fn handle(&self, message: AgentServerMessage) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        match message.operation.clone() {
            Operation::Agent(AgentOperation::List) => {
                let snapshots = self.state.capabilities.list(None);
                let agents: Vec<serde_json::Value> = snapshots
                    .into_iter()
                    .flat_map(|snapshot| {
                        let node_id = snapshot.node_id;
                        snapshot.agents.into_iter().map(move |agent| {
                            serde_json::json!({
                                "node_id": node_id,
                                "id": agent.agent_id,
                                "name": agent.name,
                                "description": agent.description,
                                "status": agent.status,
                            })
                        })
                    })
                    .collect();
                let payload = Payload::Agent(AgentPayload::ListResult { agents });
                Ok(vec![AgentServerMessage {
                    sender: "control".to_string(),
                    receiver: message.sender,
                    ..AgentServerMessage::new_result(
                        message.message_id,
                        Operation::Agent(AgentOperation::List),
                        payload,
                    )
                }])
            }
            Operation::Agent(AgentOperation::Status) => Ok(vec![AgentServerMessage {
                sender: "control".to_string(),
                receiver: message.sender,
                ..AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Agent(AgentOperation::Status),
                    Payload::Agent(AgentPayload::StatusResult { status: serde_json::json!({"status":"control_plane"}) }),
                )
            }]),
            Operation::Agent(AgentOperation::Submit) => Err(ProtocolError::PayloadDecodeFailedOwned(
                "agent.submit routing is not implemented in this behavior-completion slice".to_string(),
            )),
            _ => Err(ProtocolError::PayloadDecodeFailedOwned("unsupported client operation".to_string())),
        }
    }
}
```

If `AgentPayload::ListResult` or `StatusResult` field names differ, use the current definitions from `agent_server_protocol.rs` and adapt exactly.

- [ ] **Step 4: Register client handler in ControlPlaneServerCore**

In `handlers/mod.rs` add:

```rust
pub mod client;
```

In `core.rs`, register:

```rust
handler_registry.register(Arc::new(ClientHandler::new(state.clone())))?;
```

- [ ] **Step 5: Run tests**

Run:

```bash
cargo test -p vol-agent-server agent_list_returns_empty_list_from_control_plane
cargo check -p vol-agent-server
cargo fmt --check
```

Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add crates/vol-agent-server/src/control_plane/handlers/client.rs \
  crates/vol-agent-server/src/control_plane/handlers/mod.rs \
  crates/vol-agent-server/src/control_plane/core.rs
git commit -m "feat(server): add minimal control client handler"
```

---

## Task 4: Register Data-Plane `control.command` Handler

**Files:**
- Create: `crates/vol-agent-server/src/data_plane/handlers/control.rs`
- Modify: `crates/vol-agent-server/src/data_plane/handlers/mod.rs`
- Modify: `crates/vol-agent-server/src/data_plane/core.rs`

- [ ] **Step 1: Add failing command handler test**

Create `handlers/control.rs` with test:

```rust
#[cfg(test)]
mod tests {
    use vol_llm_agent_channel::agent_server_protocol::{
        AgentServerMessage, ControlCommand, ControlCommandOperation, ControlOperation,
        ControlPayload, MessageKind, Operation, Payload,
    };
    use vol_llm_agent_channel::DomainHandler;

    use super::DataPlaneControlHandler;

    #[tokio::test]
    async fn control_command_health_check_returns_ack() {
        let handler = DataPlaneControlHandler::new();
        let msg = AgentServerMessage {
            protocol: "agent-server/1".to_string(),
            message_id: "cmd-1".to_string(),
            sender: "control".to_string(),
            receiver: "node".to_string(),
            kind: MessageKind::Command,
            operation: Operation::Control(ControlOperation::Command),
            payload: Payload::Control(ControlPayload::Command(ControlCommand {
                command_id: "cmd-1".to_string(),
                node_id: "node-a".to_string(),
                operation: ControlCommandOperation::HealthCheck,
                deadline_ms: None,
            })),
            meta: Default::default(),
        };

        let replies = handler.handle(msg).await.unwrap();
        assert_eq!(replies.len(), 1);
        let json = replies[0].payload.data_json();
        assert_eq!(json["accepted"], true);
        assert_eq!(json["command_id"], "cmd-1");
    }
}
```

- [ ] **Step 2: Run test and verify failure**

Run:

```bash
cargo test -p vol-agent-server control_command_health_check_returns_ack
```

Expected: compile failure until handler exists.

- [ ] **Step 3: Implement handler**

Create `handlers/control.rs`:

```rust
use async_trait::async_trait;
use vol_llm_agent_channel::agent_server_protocol::{
    AgentServerMessage, ControlOperation, ControlPayload, MessageKind, Operation, Payload,
    ProtocolError,
};
use vol_llm_agent_channel::DomainHandler;

use crate::data_plane::command::accept_control_command;

pub struct DataPlaneControlHandler;

impl DataPlaneControlHandler {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl DomainHandler for DataPlaneControlHandler {
    fn name(&self) -> &str { "data-plane-control" }

    fn operations(&self) -> Vec<Operation> {
        vec![Operation::Control(ControlOperation::Command)]
    }

    async fn handle(&self, message: AgentServerMessage) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        match message.payload.clone() {
            Payload::Control(ControlPayload::Command(command)) => {
                let ack = accept_control_command(&command).await;
                Ok(vec![AgentServerMessage {
                    sender: "node".to_string(),
                    receiver: message.sender,
                    ..AgentServerMessage::new_result(
                        message.message_id,
                        Operation::Control(ControlOperation::Command),
                        Payload::Control(ControlPayload::CommandAck(ack)),
                    )
                }])
            }
            _ => Err(ProtocolError::PayloadDecodeFailedOwned("expected control.command payload".to_string())),
        }
    }
}
```

- [ ] **Step 4: Register in data-plane core**

In `data_plane/handlers/mod.rs` add:

```rust
pub mod control;
```

In `data_plane/core.rs`, import and register `DataPlaneControlHandler`:

```rust
use crate::data_plane::handlers::control::DataPlaneControlHandler;
```

Where handlers are registered:

```rust
handler_registry.register(Arc::new(DataPlaneControlHandler::new()))?;
```

- [ ] **Step 5: Run tests**

Run:

```bash
cargo test -p vol-agent-server control_command_health_check_returns_ack
cargo check -p vol-agent-server
cargo fmt --check
```

Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add crates/vol-agent-server/src/data_plane/handlers/control.rs \
  crates/vol-agent-server/src/data_plane/handlers/mod.rs \
  crates/vol-agent-server/src/data_plane/core.rs
git commit -m "feat(server): handle data plane control commands"
```

---

## Task 5: Sync Capability Revision to Node Records

**Files:**
- Modify: `crates/vol-agent-server/src/control_plane/registry.rs`
- Modify: `crates/vol-agent-server/src/control_plane/handlers/control.rs`

- [ ] **Step 1: Add failing revision sync test**

In `handlers/control.rs` tests, add:

```rust
#[tokio::test]
async fn capability_snapshot_updates_node_capability_revision() {
    let state = Arc::new(ControlPlaneState::new());
    state.nodes.register(NodeRegistration {
        node_id: "node-a".to_string(),
        name: "Node A".to_string(),
        version: "0.1.0".to_string(),
    }, "node-a".to_string(), 1000).unwrap();

    let handler = ControlHandler::new(state.clone());
    let msg = AgentServerMessage {
        protocol: "agent-server/1".to_string(),
        message_id: "snap-1".to_string(),
        sender: "node-a".to_string(),
        receiver: "control".to_string(),
        kind: MessageKind::Command,
        operation: Operation::Control(ControlOperation::CapabilitySnapshot),
        payload: Payload::Control(ControlPayload::CapabilitySnapshot(CapabilitySnapshot {
            node_id: "node-a".to_string(),
            revision: 7,
            generated_at_ms: Some(1000),
            agents: vec![],
            tools: vec![],
            mcp_servers: vec![],
            skills: vec![],
        })),
        meta: Default::default(),
    };

    handler.handle(msg).await.unwrap();
    let node = state.nodes.get("node-a").unwrap();
    assert_eq!(node.capability_revision, 7);
}
```

Add imports as needed.

- [ ] **Step 2: Run test and verify failure**

Run:

```bash
cargo test -p vol-agent-server capability_snapshot_updates_node_capability_revision
```

Expected: fails because revision remains 0.

- [ ] **Step 3: Add registry setter**

In `registry.rs`:

```rust
pub fn update_capability_revision(&self, node_id: &str, revision: u64) -> Result<(), String> {
    let mut nodes = self.nodes.write().expect("NodeRegistry nodes lock poisoned");
    let node = nodes
        .get_mut(node_id)
        .ok_or_else(|| "node_not_registered".to_string())?;
    node.capability_revision = revision;
    Ok(())
}
```

- [ ] **Step 4: Update control handler snapshot path**

In `ControlHandler` snapshot arm, after applying snapshot, update registry:

```rust
let node_id = snapshot.node_id.clone();
let revision = snapshot.revision;
self.state.capabilities.apply_snapshot(snapshot)
    .map_err(ProtocolError::PayloadDecodeFailedOwned)?;
self.state.nodes.update_capability_revision(&node_id, revision)
    .map_err(ProtocolError::PayloadDecodeFailedOwned)?;
Ok(vec![])
```

- [ ] **Step 5: Run tests**

Run:

```bash
cargo test -p vol-agent-server capability_snapshot_updates_node_capability_revision
cargo test -p vol-agent-server control_plane
cargo fmt --check
```

Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add crates/vol-agent-server/src/control_plane/registry.rs \
  crates/vol-agent-server/src/control_plane/handlers/control.rs
git commit -m "fix(server): sync node capability revision"
```

---

## Task 6: Add `control.run_status` Handler

**Files:**
- Create: `crates/vol-agent-server/src/control_plane/handlers/run.rs`
- Modify: `crates/vol-agent-server/src/control_plane/handlers/mod.rs`
- Modify: `crates/vol-agent-server/src/control_plane/core.rs`

- [ ] **Step 1: Add failing run status test**

Create `handlers/run.rs` with test:

```rust
#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use vol_llm_agent_channel::agent_server_protocol::{
        AgentServerMessage, ControlOperation, ControlPayload, MessageKind, Operation, Payload,
        RunStatusRequest,
    };
    use vol_llm_agent_channel::DomainHandler;

    use crate::control_plane::handlers::run::RunHandler;
    use crate::control_plane::state::ControlPlaneState;
    use crate::control_plane::store::RunRecord;

    #[tokio::test]
    async fn run_status_returns_stored_run() {
        let state = Arc::new(ControlPlaneState::new());
        state.runs.insert(RunRecord {
            run_id: "run-1".to_string(),
            command_id: Some("cmd-1".to_string()),
            node_id: "node-a".to_string(),
            agent_id: "coding".to_string(),
            status: "running".to_string(),
        });

        let handler = RunHandler::new(state);
        let msg = AgentServerMessage {
            protocol: "agent-server/1".to_string(),
            message_id: "1".to_string(),
            sender: "client".to_string(),
            receiver: "control".to_string(),
            kind: MessageKind::Command,
            operation: Operation::Control(ControlOperation::RunStatus),
            payload: Payload::Control(ControlPayload::RunStatus(RunStatusRequest {
                run_id: "run-1".to_string(),
            })),
            meta: Default::default(),
        };

        let replies = handler.handle(msg).await.unwrap();
        let json = replies[0].payload.data_json();
        assert_eq!(json["run_id"], "run-1");
        assert_eq!(json["status"], "running");
        assert_eq!(json["node_id"], "node-a");
    }
}
```

- [ ] **Step 2: Run test and verify failure**

Run:

```bash
cargo test -p vol-agent-server run_status_returns_stored_run
```

Expected: compile failure until handler exists.

- [ ] **Step 3: Implement `RunHandler`**

Create `handlers/run.rs`:

```rust
use std::sync::Arc;

use async_trait::async_trait;
use vol_llm_agent_channel::agent_server_protocol::{
    AgentServerMessage, ControlOperation, ControlPayload, Operation, Payload, ProtocolError,
    RunStatusResult,
};
use vol_llm_agent_channel::DomainHandler;

use crate::control_plane::core::make_result;
use crate::control_plane::state::ControlPlaneState;

pub struct RunHandler { state: Arc<ControlPlaneState> }
impl RunHandler { pub fn new(state: Arc<ControlPlaneState>) -> Self { Self { state } } }

#[async_trait]
impl DomainHandler for RunHandler {
    fn name(&self) -> &str { "run" }

    fn operations(&self) -> Vec<Operation> {
        vec![Operation::Control(ControlOperation::RunStatus)]
    }

    async fn handle(&self, message: AgentServerMessage) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        match message.payload.clone() {
            Payload::Control(ControlPayload::RunStatus(req)) => {
                let run = self.state.runs.get(&req.run_id);
                let result = match run {
                    Some(run) => RunStatusResult {
                        run_id: run.run_id,
                        status: run.status,
                        node_id: Some(run.node_id),
                    },
                    None => RunStatusResult {
                        run_id: req.run_id,
                        status: "not_found".to_string(),
                        node_id: None,
                    },
                };
                Ok(vec![make_result(
                    message,
                    ControlOperation::RunStatus,
                    ControlPayload::RunStatusResult(result),
                )])
            }
            _ => Err(ProtocolError::PayloadDecodeFailedOwned("expected control.run_status payload".to_string())),
        }
    }
}
```

- [ ] **Step 4: Register handler**

In `handlers/mod.rs`:

```rust
pub mod run;
```

In `ControlPlaneServerCore::new`:

```rust
handler_registry.register(Arc::new(RunHandler::new(state.clone())))?;
```

- [ ] **Step 5: Run tests**

Run:

```bash
cargo test -p vol-agent-server run_status_returns_stored_run
cargo check -p vol-agent-server
cargo fmt --check
```

Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add crates/vol-agent-server/src/control_plane/handlers/run.rs \
  crates/vol-agent-server/src/control_plane/handlers/mod.rs \
  crates/vol-agent-server/src/control_plane/core.rs
git commit -m "feat(server): add run status handler"
```

---

## Task 7: Minimal Combined-Mode Local Node Registration

**Files:**
- Create: `crates/vol-agent-server/src/data_plane/reporter.rs`
- Modify: `crates/vol-agent-server/src/data_plane/mod.rs`
- Modify: `crates/vol-agent-server/src/app.rs`

- [ ] **Step 1: Add local registration helper test**

Create `data_plane/reporter.rs` with test:

```rust
#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::control_plane::state::ControlPlaneState;
    use super::register_local_data_plane;

    #[test]
    fn register_local_data_plane_creates_node_record() {
        let state = Arc::new(ControlPlaneState::new());
        register_local_data_plane(
            state.clone(),
            "local-node".to_string(),
            "Local Node".to_string(),
            "test-version".to_string(),
        ).unwrap();

        let node = state.nodes.get("local-node").unwrap();
        assert_eq!(node.node_id, "local-node");
        assert_eq!(node.name, "Local Node");
        assert_eq!(node.status, "online");
    }
}
```

- [ ] **Step 2: Run test and verify failure**

Run:

```bash
cargo test -p vol-agent-server register_local_data_plane_creates_node_record
```

Expected: compile failure until helper exists.

- [ ] **Step 3: Implement helper**

Create `reporter.rs`:

```rust
use std::sync::Arc;

use vol_llm_agent_channel::agent_server_protocol::NodeRegistration;

use crate::control_plane::state::ControlPlaneState;

pub fn register_local_data_plane(
    state: Arc<ControlPlaneState>,
    node_id: String,
    name: String,
    version: String,
) -> Result<(), String> {
    state.nodes.register(
        NodeRegistration { node_id: node_id.clone(), name, version },
        "local".to_string(),
        now_ms(),
    )?;
    Ok(())
}

fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}
```

- [ ] **Step 4: Call helper in combined mode**

In `data_plane/mod.rs` add:

```rust
pub mod reporter;
```

In `app.rs`, after both cores are built and before mounting routes:

```rust
if control_plane_enabled && data_plane_enabled {
    if let Some(control) = control_core.as_ref() {
        let node_id = config
            .data_plane
            .node_id
            .clone()
            .unwrap_or_else(|| "local-data-plane".to_string());
        let name = config
            .data_plane
            .name
            .clone()
            .unwrap_or_else(|| node_id.clone());
        crate::data_plane::reporter::register_local_data_plane(
            control.state.clone(),
            node_id,
            name,
            env!("CARGO_PKG_VERSION").to_string(),
        )?;
    }
}
```

This MVP uses in-process registration. A future task can replace it with loopback JSON-RPC.

- [ ] **Step 5: Run tests**

Run:

```bash
cargo test -p vol-agent-server register_local_data_plane_creates_node_record
cargo check -p vol-agent-server
cargo fmt --check
```

Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add crates/vol-agent-server/src/data_plane/reporter.rs \
  crates/vol-agent-server/src/data_plane/mod.rs \
  crates/vol-agent-server/src/app.rs
git commit -m "feat(server): register local data plane in combined mode"
```

---

## Task 8: Documentation and Verification

**Files:**
- Modify: `docs/superpowers/plans/2026-06-10-control-plane-behavior-completion-plan.md`
- Update wiki through `wiki-ingest`

- [ ] **Step 1: Run final verification**

Run:

```bash
./scripts/check-agent-boundaries.sh
cargo test -p vol-llm-agent-channel --tests
cargo test -p vol-agent-server --tests
cargo check -p vol-llm-agent-channel -p vol-agent-server
cargo fmt --all --check --manifest-path /Users/admin/Documents/learn/vol-agent/Cargo.toml
```

Expected: boundary check passes; targeted channel/server tests pass; targeted check passes; formatting passes.

- [ ] **Step 2: Run final code review**

Dispatch final reviewer with this prompt summary:

```text
Review control-plane behavior completion: JSON-RPC notifications, endpoint role allowlists, minimal client handler, data-plane control.command handler, capability revision sync, run_status, combined-mode local registration. Check behavior against architecture/addendum and no crate-boundary regressions.
```

- [ ] **Step 3: Update docs/wiki**

Use `wiki-ingest` with summary of completed behavior.

- [ ] **Step 4: Upload plan/doc updates to Lark**

Upload this plan under the plans wiki node if not already uploaded:

```bash
lark-cli docs +create \
  --api-version v2 \
  --doc-format markdown \
  --content @docs/superpowers/plans/2026-06-10-control-plane-behavior-completion-plan.md \
  --wiki-node TEkkw1W6niuBxQkcvswchOo5nhb \
  --as user
```

- [ ] **Step 5: Commit**

```bash
git add crates docs scripts
git commit -m "feat(server): complete control plane behavior"
```

---

## Self-Review Checklist

Spec coverage:

- JSON-RPC notification decode: Task 1.
- Endpoint role allowlists: Task 2.
- Minimal client-facing handlers: Task 3.
- Data-plane `control.command`: Task 4.
- Capability revision sync: Task 5.
- `control.run_status`: Task 6.
- Combined-mode local registration: Task 7.
- Verification/docs: Task 8.

Known intentional MVP limits:

- `agent.submit` routing in `ClientHandler` may return a not-implemented domain error until a later richer command-delivery task wires actual node-session send handles.
- Combined mode uses in-process local registration rather than loopback JSON-RPC to keep this follow-up plan small and deterministic.
- Persistent control-plane storage remains out of scope.
