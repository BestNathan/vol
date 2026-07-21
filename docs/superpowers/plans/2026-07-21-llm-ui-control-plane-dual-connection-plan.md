# Control-Plane Dual-Connection UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Adapt vol-llm-ui to use dual WebSocket connections — one to CP for management queries (agent.list, node.list, task.list), another to DP for agent interaction (submit, subscribe, sessions, tools, skills, MCP).

**Architecture:** CP `agent.list` gains `ws_url` per agent; UI creates `DpConnection` lazily per node when user selects an agent. CP `task.list` aggregates from all nodes. New ingress for `agent-server-dp` and `agent-server-ansible`. New `NodesPanel` component, modified `AgentsPanel` (node grouping), modified `InputArea` (DP routing), dual-status `StatusBar`.

**Tech Stack:** Rust (Dioxus WASM, vol-llm-agent-protocol), K8s manifests (ArgoCD), TOML config

---

## Phase 1: Control-Plane — agent.list ws_url injection

### Task 1.1: Add node_ingress config to ServerConfig

**Files:**
- Modify: `crates/vol-agent-server/src/config.rs`

- [ ] **Step 1: Add `node_ingress` field to `ControlPlaneSection`**

```rust
// After the lease_scan_secs field in ControlPlaneSection:
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
    #[serde(default)]
    pub node_ingress: std::collections::HashMap<String, String>,
}
```

- [ ] **Step 2: Update `Default for ControlPlaneSection` impl**

```rust
impl Default for ControlPlaneSection {
    fn default() -> Self {
        Self {
            auth_token: None,
            client_ws_path: default_client_ws_path(),
            node_ws_path: default_node_ws_path(),
            lease_timeout_secs: default_lease_timeout_secs(),
            lease_scan_secs: default_lease_scan_secs(),
            node_ingress: std::collections::HashMap::new(),
        }
    }
}
```

- [ ] **Step 3: Add config parse test**

In `mod tests` in `config.rs`, add:

```rust
#[test]
fn test_parse_node_ingress_config() {
    let toml_str = r#"
[server.roles]
control_plane = true
data_plane = false

[control_plane.node_ingress]
"dp-1" = "wss://dp.vol.bestnathan.top/ws"
"dingtalk" = "wss://dingtalk.vol.bestnathan.top/ws"
"#;

    let config: ServerConfig = toml::from_str(toml_str).unwrap();
    let ingress = &config.control_plane.node_ingress;
    assert_eq!(ingress.get("dp-1").unwrap(), "wss://dp.vol.bestnathan.top/ws");
    assert_eq!(ingress.get("dingtalk").unwrap(), "wss://dingtalk.vol.bestnathan.top/ws");
}
```

- [ ] **Step 4: Run test**

```bash
cargo test -p vol-agent-server -- config::tests::test_parse_node_ingress_config
```

- [ ] **Step 5: Commit**

```bash
git add crates/vol-agent-server/src/config.rs
git commit -m "feat(cp): add node_ingress mapping to control-plane config"
```

### Task 1.2: Wire node_ingress into ControlPlaneState

**Files:**
- Modify: `crates/vol-agent-server/src/control_plane/state.rs`

- [ ] **Step 1: Add `node_ingress` field to `ControlPlaneState`**

```rust
use std::collections::HashMap;

pub struct ControlPlaneState {
    pub nodes: NodeRegistry,
    pub capabilities: CapabilityIndex,
    pub node_connections: std::sync::RwLock<HashMap<String, Arc<dyn Connection>>>,
    pub event_bus: EventBus,
    pub node_ingress: HashMap<String, String>,
    // ... any existing fields
}
```

- [ ] **Step 2: Update `ControlPlaneState::new()` to accept and store `node_ingress`**

```rust
impl ControlPlaneState {
    pub fn new_with_ingress(node_ingress: HashMap<String, String>) -> Self {
        Self {
            nodes: NodeRegistry::new(),
            capabilities: CapabilityIndex::new(),
            node_connections: std::sync::RwLock::new(HashMap::new()),
            event_bus: EventBus::new(),
            node_ingress,
        }
    }
    
    // Keep existing ::new() for tests
    pub fn new() -> Self {
        Self::new_with_ingress(HashMap::new())
    }
}
```

- [ ] **Step 3: Run existing tests**

```bash
cargo test -p vol-agent-server -- control_plane
```

- [ ] **Step 4: Commit**

```bash
git add crates/vol-agent-server/src/control_plane/state.rs
git commit -m "feat(cp): add node_ingress to ControlPlaneState"
```

### Task 1.3: Build ws_url into agent.list response

**Files:**
- Modify: `crates/vol-agent-server/src/control_plane/handlers/client.rs`

- [ ] **Step 1: Inject ws_url into each agent entry in agent.list**

In the `ClientHandler::handle` method, modify the `AgentOperation::List` arm:

```rust
Operation::Agent(AgentOperation::List) => {
    let snapshots = self.state.capabilities.list(None);
    let agents: Vec<serde_json::Value> = snapshots
        .into_iter()
        .flat_map(|snapshot| {
            let node_id = snapshot.node_id.clone();
            let ws_url = self.state
                .node_ingress
                .get(&node_id)
                .cloned()
                .unwrap_or_default();
            snapshot.agents.into_iter().map(move |agent| {
                let mut entry = serde_json::json!({
                    "id": agent.agent_id,
                    "name": agent.name,
                    "description": agent.description,
                    "status": agent.status,
                    "node_id": node_id.clone(),
                    "ws_url": ws_url.clone(),
                });
                // Remove null fields
                if agent.description.is_none() {
                    entry.as_object_mut().unwrap().remove("description");
                }
                if agent.status.is_none() {
                    entry.as_object_mut().unwrap().remove("status");
                }
                entry
            })
        })
        .collect();
    let payload = Payload::Agent(AgentPayload::ListResult { agents });
    let mut reply = AgentServerMessage::new_result(
        message.message_id,
        Operation::Agent(AgentOperation::List),
        payload,
    );
    reply.sender = "control".to_string();
    reply.receiver = message.sender;
    Ok(vec![reply])
}
```

- [ ] **Step 2: Update agent_list tests to verify ws_url**

In the existing `agent_list_returns_agents_from_capability_snapshots` test, add `node_ingress` to the state and verify `ws_url`:

```rust
#[tokio::test]
async fn agent_list_includes_ws_url_from_node_ingress() {
    use std::collections::HashMap;
    use vol_llm_agent_protocol::agent_server_protocol::{AgentCapability, CapabilitySnapshot};

    let mut ingress = HashMap::new();
    ingress.insert(
        "node-a".to_string(),
        "wss://dp.vol.bestnathan.top/ws".to_string(),
    );
    let state = Arc::new(ControlPlaneState::new_with_ingress(ingress));
    state
        .capabilities
        .apply_snapshot(CapabilitySnapshot {
            node_id: "node-a".to_string(),
            revision: 1,
            generated_at_ms: Some(1000),
            agents: vec![AgentCapability {
                agent_id: "coding".to_string(),
                name: "Coding Agent".to_string(),
                description: Some("A coding agent".to_string()),
                status: Some("idle".to_string()),
            }],
            tools: vec![],
            mcp_servers: vec![],
            skills: vec![],
        })
        .unwrap();

    let handler = ClientHandler::new(state);
    let msg = AgentServerMessage {
        protocol: "agent-server/1".to_string(),
        message_id: "1".to_string(),
        sender: "client".to_string(),
        receiver: "control".to_string(),
        kind: MessageKind::Command,
        operation: Operation::Agent(AgentOperation::List),
        payload: Payload::Agent(AgentPayload::ListResult { agents: vec![] }),
        meta: Default::default(),
    };

    let replies = handler.handle(msg).await.unwrap();
    let json = replies[0].payload.data_json();
    let agents = json["agents"].as_array().unwrap();
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0]["ws_url"], "wss://dp.vol.bestnathan.top/ws");
    assert_eq!(agents[0]["node_id"], "node-a");
}
```

- [ ] **Step 3: Run tests**

```bash
cargo test -p vol-agent-server -- control_plane::handlers::client
```

- [ ] **Step 4: Commit**

```bash
git add crates/vol-agent-server/src/control_plane/handlers/client.rs
git commit -m "feat(cp): inject ws_url into agent.list response"
```

### Task 1.4: Wire node_ingress from config into app.rs

**Files:**
- Modify: `crates/vol-agent-server/src/app.rs`
- Modify: `crates/vol-agent-server/src/control_plane/core.rs`

- [ ] **Step 1: Pass `node_ingress` when creating `ControlPlaneState`**

In `app.rs`, find where `ControlPlaneState::new()` is called and change to:

```rust
let cp_state = Arc::new(ControlPlaneState::new_with_ingress(
    config.control_plane.node_ingress.clone(),
));
```

- [ ] **Step 2: Verify build**

```bash
cargo check -p vol-agent-server
```

- [ ] **Step 3: Commit**

```bash
git add crates/vol-agent-server/src/app.rs crates/vol-agent-server/src/control_plane/core.rs
git commit -m "feat(cp): wire node_ingress from config to ControlPlaneState"
```

---

## Phase 2: Control-Plane — task.list aggregation

### Task 2.1: Add task.list/task.get to node protocol

**Files:**
- Modify: `crates/vol-llm-agent-protocol/src/agent_server_protocol.rs`

- [ ] **Step 1: Add TaskList/TaskGet to ControlOperation enum**

In `ControlOperation`, after `RunStatusResult`:

```rust
    TaskList(TaskListRequest),
    TaskListResult(TaskListResult),
    TaskGet(TaskGetRequest),
    TaskGetResult(TaskGetResult),
```

- [ ] **Step 2: Add request/response payload types in ControlPayload**

```rust
    TaskList(TaskListRequest),
    TaskListResult(TaskListResult),
    TaskGet(TaskGetRequest),
    TaskGetResult(TaskGetResult),
```

- [ ] **Step 3: Add type definitions**

After `CapabilityListResult`:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskListRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskListResult {
    pub tasks: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskGetRequest {
    pub task_id: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskGetResult {
    pub task: serde_json::Value,
}
```

- [ ] **Step 4: Update all tests that match on ControlPayload/ControlOperation**

Search and update exhaustive match arms.

- [ ] **Step 5: Verify build**

```bash
cargo check -p vol-llm-agent-protocol -p vol-agent-server
```

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-agent-protocol/src/agent_server_protocol.rs
git commit -m "feat(protocol): add TaskList/TaskGet to control protocol"
```

### Task 2.2: Add task.list/task.get to ClientHandler (MVP)

**Files:**
- Modify: `crates/vol-agent-server/src/control_plane/handlers/client.rs`

**Design note:** Full cross-node task aggregation requires async request-response relay through node WS connections. For MVP, CP returns an empty task list with a note. The UI still gets per-node tasks via the DP connection when the user interacts with an agent. Full aggregation is a follow-up.

- [ ] **Step 1: Add TaskOperation handlers to ClientHandler::operations**

```rust
fn operations(&self) -> Vec<Operation> {
    vec![
        Operation::Agent(AgentOperation::List),
        Operation::Agent(AgentOperation::Status),
        Operation::Agent(AgentOperation::Submit),
        Operation::Task(TaskOperation::List),
        Operation::Task(TaskOperation::Get),
    ]
}
```

- [ ] **Step 2: Handle with empty aggregation (MVP)**

```rust
Operation::Task(TaskOperation::List) => {
    // MVP: return empty. Full cross-node aggregation is follow-up.
    let tasks: Vec<serde_json::Value> = vec![];
    let payload = Payload::Data(serde_json::json!({ "tasks": tasks }));
    let mut reply = AgentServerMessage::new_result(
        message.message_id,
        Operation::Task(TaskOperation::List),
        payload,
    );
    reply.sender = "control".to_string();
    reply.receiver = message.sender;
    Ok(vec![reply])
}
Operation::Task(TaskOperation::Get) => {
    // MVP: task not found (needs cross-node routing)
    Err(ProtocolError::PayloadDecodeFailedOwned(
        "task.get: cross-node routing not yet implemented".to_string(),
    ))
}
```

- [ ] **Step 3: Add test**

```rust
#[tokio::test]
async fn task_list_returns_empty_list() {
    let state = Arc::new(ControlPlaneState::new());
    let handler = ClientHandler::new(state);
    let msg = AgentServerMessage {
        protocol: "agent-server/1".to_string(),
        message_id: "1".to_string(),
        sender: "client".to_string(),
        receiver: "control".to_string(),
        kind: MessageKind::Command,
        operation: Operation::Task(TaskOperation::List),
        payload: Payload::Data(serde_json::json!({})),
        meta: Default::default(),
    };
    let replies = handler.handle(msg).await.unwrap();
    assert_eq!(replies.len(), 1);
    let json = replies[0].payload.data_json();
    assert!(json.get("tasks").is_some());
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p vol-agent-server -- control_plane::handlers::client
```

- [ ] **Step 5: Commit**

```bash
git add crates/vol-agent-server/src/control_plane/handlers/client.rs
git commit -m "feat(cp): add task.list/task.get stub to ClientHandler"
```

---

## Phase 3: Data-plane ingress manifests

### Task 3.1: Add ingress for agent-server-dp

**Files:**
- Create: `deploy/argocd/manifests/workloads/agent-server-dp/ingress.yaml`

```yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: agent-server-dp
  namespace: vol-agent-system
  labels:
    app.kubernetes.io/name: agent-server-dp
    app.kubernetes.io/part-of: vol-agent
  annotations:
    higress.io/domain: dp.vol.bestnathan.top
    nginx.ingress.kubernetes.io/ssl-redirect: "false"
    nginx.ingress.kubernetes.io/proxy-read-timeout: "86400"
    nginx.ingress.kubernetes.io/proxy-send-timeout: "86400"
spec:
  ingressClassName: higress
  rules:
    - host: dp.vol.bestnathan.top
      http:
        paths:
          - path: /
            pathType: Prefix
            backend:
              service:
                name: agent-server-dp
                port:
                  number: 3002
```

- [ ] **Step 1: Write the file**
- [ ] **Step 2: Validate with kubectl dry-run**

```bash
kubectl --dry-run=client -f deploy/argocd/manifests/workloads/agent-server-dp/ingress.yaml apply --dry-run=client
```

- [ ] **Step 3: Commit**

```bash
git add deploy/argocd/manifests/workloads/agent-server-dp/ingress.yaml
git commit -m "feat(k8s): add ingress for agent-server-dp at dp.vol.bestnathan.top"
```

### Task 3.2: Add ingress for agent-server-ansible

**Files:**
- Create: `deploy/argocd/manifests/workloads/agent-server-ansible/ingress.yaml`

```yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: agent-server-ansible
  namespace: vol-agent-system
  labels:
    app.kubernetes.io/name: agent-server-ansible
    app.kubernetes.io/part-of: vol-agent
  annotations:
    higress.io/domain: ansible.vol.bestnathan.top
    nginx.ingress.kubernetes.io/ssl-redirect: "false"
    nginx.ingress.kubernetes.io/proxy-read-timeout: "86400"
    nginx.ingress.kubernetes.io/proxy-send-timeout: "86400"
spec:
  ingressClassName: higress
  rules:
    - host: ansible.vol.bestnathan.top
      http:
        paths:
          - path: /
            pathType: Prefix
            backend:
              service:
                name: agent-server-ansible
                port:
                  number: 3003
```

- [ ] **Step 1: Write the file**
- [ ] **Step 2: Validate**

```bash
kubectl --dry-run=client -f deploy/argocd/manifests/workloads/agent-server-ansible/ingress.yaml apply --dry-run=client
```

- [ ] **Step 3: Commit**

```bash
git add deploy/argocd/manifests/workloads/agent-server-ansible/ingress.yaml
git commit -m "feat(k8s): add ingress for agent-server-ansible at ansible.vol.bestnathan.top"
```

### Task 3.3: Update CP ConfigMap with node_ingress

**Files:**
- Modify: `deploy/argocd/manifests/workloads/agent-server/configmap.yaml`

- [ ] **Step 1: Add `[control_plane.node_ingress]` section to agent-server.toml**

After the `[control_plane]` section, add:

```toml
[control_plane.node_ingress]
"dp-1" = "wss://dp.vol.bestnathan.top/ws"
"dingtalk" = "wss://dingtalk.vol.bestnathan.top/ws"
"ansible" = "wss://ansible.vol.bestnathan.top/ws"
```

- [ ] **Step 2: Commit**

```bash
git add deploy/argocd/manifests/workloads/agent-server/configmap.yaml
git commit -m "feat(k8s): add node_ingress mapping to CP ConfigMap"
```

---

## Phase 4: UI — Dual connection state model

### Task 4.1: Add node_ws_url field to AgentListEntry

**Files:**
- Modify: `crates/vol-llm-ui/src/web/client.rs`

- [ ] **Step 1: Add ws_url and node_id fields to AgentListEntry**

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentListEntry {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub description: String,
    pub scope: String,
    #[serde(default)]
    pub node_id: Option<String>,
    #[serde(default)]
    pub ws_url: Option<String>,
}
```

- [ ] **Step 2: Build and verify**

```bash
cargo check -p vol-llm-ui --features web
```

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-ui/src/web/client.rs
git commit -m "feat(ui): add node_id and ws_url to AgentListEntry"
```

### Task 4.2: Add DpConnection type and AppState changes

**Files:**
- Create: `crates/vol-llm-ui/src/web/dp_connection.rs`
- Modify: `crates/vol-llm-ui/src/web/components/app.rs`
- Modify: `crates/vol-llm-ui/src/web/mod.rs`

- [ ] **Step 1: Create `dp_connection.rs`**

```rust
//! Per-node data-plane WebSocket connection.
//!
//! Created lazily when the user selects an agent on a node. Reused across
//! agents on the same node. Each connection has its own event stream and
//! auto-subscribes on open.

use crate::web::client::{ConnectionState, JsonRpcClient};
use std::collections::HashMap;

/// A data-plane connection for one node.
pub struct DpConnection {
    pub client: JsonRpcClient,
    pub node_id: String,
    pub ws_url: String,
    /// Agent IDs known to be on this node.
    pub agent_ids: Vec<String>,
}

/// Manages a pool of per-node data-plane connections.
#[derive(Default)]
pub struct DpConnectionPool {
    /// Active connections keyed by node_id.
    connections: HashMap<String, DpConnection>,
}

impl DpConnectionPool {
    pub fn new() -> Self {
        Self {
            connections: HashMap::new(),
        }
    }

    /// Get or create a DP connection for a node.
    /// Returns Some if ws_url is present, None otherwise.
    pub fn get_or_create(
        &mut self,
        node_id: &str,
        ws_url: &str,
        agent_ids: Vec<String>,
    ) -> Option<&DpConnection> {
        if self.connections.contains_key(node_id) {
            return self.connections.get(node_id);
        }
        let client = JsonRpcClient::new(ws_url);
        let conn = DpConnection {
            client,
            node_id: node_id.to_string(),
            ws_url: ws_url.to_string(),
            agent_ids,
        };
        self.connections.insert(node_id.to_string(), conn);
        self.connections.get(node_id)
    }

    /// Get an existing connection by node_id.
    pub fn get(&self, node_id: &str) -> Option<&DpConnection> {
        self.connections.get(node_id)
    }

    /// Check if a connection exists for a node.
    pub fn contains(&self, node_id: &str) -> bool {
        self.connections.contains_key(node_id)
    }
}
```

- [ ] **Step 2: Register module in `web/mod.rs`**

```rust
pub mod dp_connection;
```

- [ ] **Step 3: Add dp_connections to AppState in `app.rs`**

```rust
use crate::web::dp_connection::DpConnectionPool;

pub struct AppState {
    pub event_bus: EventBus,
    pub rpc_client: JsonRpcClient,       // CP connection (keep for backward compat)
    pub cp_client: JsonRpcClient,         // explicit CP connection
    pub dp_pool: DpConnectionPool,
    pub active_node_id: Signal<Option<String>>,
    pub active_tab: Signal<ActiveTab>,
}
```

- [ ] **Step 4: Initialize dp_pool in App component**

In `App::render`, add `dp_pool` initialization.

- [ ] **Step 5: Verify build**

```bash
cargo check -p vol-llm-ui --features web
```

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-ui/src/web/dp_connection.rs crates/vol-llm-ui/src/web/mod.rs crates/vol-llm-ui/src/web/components/app.rs
git commit -m "feat(ui): add DpConnectionPool for per-node DP connections"
```

---

## Phase 5: UI — NodesPanel component

### Task 5.1: Add node.list/types in client.rs

**Files:**
- Modify: `crates/vol-llm-ui/src/web/client.rs`

- [ ] **Step 1: Add NodeListEntry type**

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeListEntry {
    pub node_id: String,
    pub name: String,
    pub version: String,
    pub status: String,
    pub agent_count: Option<usize>,
    pub load: Option<NodeLoadInfo>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeLoadInfo {
    pub cpu: Option<f64>,
    pub memory_mb: Option<u64>,
}
```

- [ ] **Step 2: Add node_list method to JsonRpcClient**

```rust
pub fn node_list(&self, cb: impl FnOnce(Result<Vec<NodeListEntry>, String>) + 'static) {
    let id = self.alloc_id();
    let msg = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "control.node_list",
        "params": {},
        "id": id,
    });
    let json = match serde_json::to_string(&msg) {
        Ok(j) => j,
        Err(e) => { cb(Err(e.to_string())); return; }
    };
    if let Err(e) = self.send_raw(&json) {
        cb(Err(format!("send failed: {e:?}"))); return;
    }
    let cb: Box<dyn FnOnce(serde_json::Value)> = Box::new(move |result| {
        match result.get("nodes").and_then(|v| v.as_array()) {
            Some(nodes) => {
                let parsed: Vec<NodeListEntry> = nodes
                    .iter()
                    .filter_map(|n| serde_json::from_value(n.clone()).ok())
                    .collect();
                cb(Ok(parsed));
            }
            None => cb(Err("no nodes in response".to_string())),
        }
    });
    self.inner.pending.borrow_mut().insert(id, cb);
}
```

- [ ] **Step 3: Build**

```bash
cargo check -p vol-llm-ui --features web
```

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-ui/src/web/client.rs
git commit -m "feat(ui): add node_list RPC and NodeListEntry type"
```

### Task 5.2: Create NodesPanel component

**Files:**
- Create: `crates/vol-llm-ui/src/web/components/nodes_panel.rs`
- Modify: `crates/vol-llm-ui/src/web/components/mod.rs`

- [ ] **Step 1: Create `nodes_panel.rs`**

```rust
//! Nodes panel — shows registered data-plane nodes and their status.

use dioxus::prelude::*;
use crate::web::components::app::AppState;

#[component]
pub fn NodesPanel() -> Element {
    let app: AppState = use_context();
    let nodes = use_signal(Vec::new);
    let error = use_signal(|| None::<String>);

    use_effect(move || {
        let cp = app.cp_client.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let (tx, rx) = futures_channel::oneshot::channel();
            cp.node_list(move |result| {
                let _ = tx.send(result);
            });
            match rx.await {
                Ok(Ok(n)) => nodes.set(n),
                Ok(Err(e)) => error.set(Some(e)),
                Err(_) => error.set(Some("channel closed".to_string())),
            }
        });
    });

    rsx! {
        div { class: "flex flex-col h-full p-3 overflow-auto",
            h2 { class: "text-lg font-bold mb-3 text-[#e0e0e0]", "Nodes" }
            if let Some(ref err) = *error.read() {
                div { class: "text-red-400 text-sm", "Error: {err}" }
            } else if nodes.read().is_empty() {
                div { class: "text-[#888] text-sm", "No nodes connected" }
            } else {
                for node in nodes.read().iter() {
                    NodeRow { node: node.clone() }
                }
            }
        }
    }
}

#[component]
fn NodeRow(node: super::super::client::NodeListEntry) -> Element {
    let status_color = if node.status == "online" { "bg-green-500" } else { "bg-red-500" };
    rsx! {
        div { class: "flex items-center gap-3 p-2 border-b border-[#333355] hover:bg-[#2a2a44] rounded",
            div { class: "w-2 h-2 rounded-full {status_color} flex-shrink-0" }
            div { class: "flex-1 min-w-0",
                div { class: "text-[#e0e0e0] text-sm font-medium truncate", "{node.name}" }
                div { class: "text-[#888] text-xs", "id: {node.node_id} · v{node.version}" }
            }
            if let Some(count) = node.agent_count {
                div { class: "text-[#888] text-xs flex-shrink-0", "{count} agents" }
            }
        }
    }
}
```

- [ ] **Step 2: Register component in `mod.rs`**

```rust
pub mod nodes_panel;
// and add use in app.rs
```

- [ ] **Step 3: Add Nodes tab to ActiveTab enum**

In `crates/vol-llm-ui/src/state.rs`:

```rust
pub enum ActiveTab {
    Nodes,     // <-- new, first
    Tasks,
    Agents,
    Tools,
    Workspace,
    Skills,
    Mcp,
    Logs,
    Conversation,
    Sessions,
}
```

- [ ] **Step 4: Wire into TabBar and TabContent in app.rs**

Add `Nodes` tab button (first in order) and match arm in `TabContent`.

- [ ] **Step 5: Build**

```bash
cargo check -p vol-llm-ui --features web
```

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/nodes_panel.rs crates/vol-llm-ui/src/web/components/mod.rs crates/vol-llm-ui/src/web/components/app.rs crates/vol-llm-ui/src/state.rs
git commit -m "feat(ui): add NodesPanel component with node list"
```

---

## Phase 6: UI — AgentsPanel with node_id

### Task 6.1: Show ws_url/node_id in AgentsPanel

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/agents_panel.rs`

- [ ] **Step 1: Display node_id in agent row, group by node**

In the agent list rendering, add node_id badge and group header:

```rust
// In the agents list rendering:
let grouped: std::collections::BTreeMap<String, Vec<&AgentListEntry>> = {
    let mut map = std::collections::BTreeMap::new();
    for agent in agents.iter() {
        let key = agent.node_id.clone().unwrap_or_else(|| "unknown".to_string());
        map.entry(key).or_default().push(agent);
    }
    map
};

for (node_id, node_agents) in &grouped {
    rsx! {
        div { class: "text-[#80a0ff] text-xs font-bold px-2 py-1 mt-2",
            "📡 {node_id}"
        }
        for agent in node_agents {
            AgentRow { agent: (*agent).clone() }
        }
    }
}
```

- [ ] **Step 2: On agent select, activate DP connection**

When user clicks an agent, if it has `ws_url`, call `dp_pool.get_or_create(node_id, ws_url, vec![agent.id])`.

- [ ] **Step 3: Run existing frontend tests**

```bash
cargo test -p vol-llm-ui --features web
```

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/agents_panel.rs
git commit -m "feat(ui): group agents by node_id, activate DP connection on select"
```

---

## Phase 7: UI — InputArea DP routing, StatusBar

### Task 7.1: Route agent.submit to DP connection

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/input_area.rs`

- [ ] **Step 1: Fetch DP connection for submit**

In the submit handler, look up active node's DP connection:

```rust
let active_node = app.active_node_id.read();
let dp_client = match active_node.as_ref().and_then(|nid| app.dp_pool.get(nid)) {
    Some(conn) => conn.client.clone(),
    None => {
        // Fall back to CP client (no DP connection yet)
        app.cp_client.clone()
    }
};
dp_client.submit(&input, target.as_deref())?;
```

- [ ] **Step 2: Build**

```bash
cargo check -p vol-llm-ui --features web
```

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/input_area.rs
git commit -m "feat(ui): route agent.submit to DP connection"
```

### Task 7.2: Update StatusBar with dual connection status

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/status_bar.rs`

- [ ] **Step 1: Show CP + DP connection status**

```rust
rsx! {
    div { class: "flex items-center gap-4 px-2 py-1 bg-[#1a1a2e] border-b border-[#333355] text-xs",
        // CP status
        div { class: "flex items-center gap-1",
            div { class: "w-2 h-2 rounded-full bg-green-500" }
            span { class: "text-[#888]", "CP" }
        }
        // DP status
        if let Some(ref node_id) = *app.active_node_id.read() {
            div { class: "flex items-center gap-1",
                div { class: "w-2 h-2 rounded-full bg-green-500" }
                span { class: "text-[#80a0ff]", "DP: {node_id}" }
            }
        } else {
            div { class: "flex items-center gap-1",
                div { class: "w-2 h-2 rounded-full bg-gray-500" }
                span { class: "text-[#888]", "DP: —" }
            }
        }
    }
}
```

- [ ] **Step 2: Build**

```bash
cargo check -p vol-llm-ui --features web
```

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/status_bar.rs
git commit -m "feat(ui): dual CP/DP connection status in StatusBar"
```

---

## Phase 8: Integration verification

### Task 8.1: Verify everything builds and tests pass

- [ ] **Step 1: Full workspace check**

```bash
cargo check -p vol-agent-server -p vol-llm-ui --features web -p vol-llm-agent-protocol
```

- [ ] **Step 2: Run all tests**

```bash
cargo test -p vol-agent-server -p vol-llm-agent-protocol
cargo test -p vol-llm-ui --features web
```

- [ ] **Step 3: Coverage check**

```bash
make coverage-threshold PKG=vol-agent-server PCT=80
```

- [ ] **Step 4: Commit any final fixes**

```bash
git add -A && git commit -m "chore: fix build and test issues from dual-connection implementation"
```

---

## Summary

| Phase | Tasks | Files |
|-------|-------|-------|
| 1. CP ws_url | 4 | config.rs, state.rs, client.rs, app.rs |
| 2. CP task.list | 2 | protocol.rs, client.rs |
| 3. Ingress | 3 | dp/ingress.yaml, ansible/ingress.yaml, configmap.yaml |
| 4. UI state | 2 | client.rs, dp_connection.rs, mod.rs, app.rs |
| 5. NodesPanel | 2 | client.rs, nodes_panel.rs, mod.rs, app.rs, state.rs |
| 6. AgentsPanel | 1 | agents_panel.rs |
| 7. InputArea + StatusBar | 2 | input_area.rs, status_bar.rs |
| 8. Verify | 1 | — |
