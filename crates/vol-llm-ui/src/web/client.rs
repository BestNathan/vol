//! WebSocket JSON-RPC client for the web frontend.
//!
//! Connects to the agent JSON-RPC server at ws://<host>:3001 and provides:
//! - `submit` — send `agent.submit` request
//! - `subscribe` — send `agent.subscribe` request
//! - Event stream via callback
//!
//! Server event format (AgentPayload::Event via encode_jsonrpc_message):
//! ```json
//! {"jsonrpc":"2.0","method":"agent.event","params":{"run_id":"...","event":{...}}}
//! ```

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use crate::state::{
    McpPromptInfo, McpResourceInfo, McpResourceTemplateInfo, McpServerInfo, McpToolInfo,
};
use dioxus::prelude::{Signal, Writable};
use futures_channel::mpsc;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

/// Agent event received from the server subscription.
/// Format matches AgentPayload::Event { run_id, event }.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEvent {
    pub run_id: String,
    pub event: serde_json::Value,
}

/// File entry returned by file.list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeListEntry {
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
    /// UI-only: not populated by the server; present for future use.
    #[serde(default)]
    pub agent_count: Option<usize>,
}

/// Load metrics mirroring `vol_llm_agent_protocol::agent_server_protocol::NodeLoad`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NodeLoad {
    pub running: u64,
    pub queued: u64,
}

/// Node record mirroring `vol_llm_agent_protocol::agent_server_protocol::NodeRecord`.
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

/// Capability snapshot mirroring
/// `vol_llm_agent_protocol::agent_server_protocol::CapabilitySnapshot`.
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

/// Agent metadata entry returned by agent.list.
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskEntry {
    pub id: u64,
    pub status: String,
    pub kind: String,
    pub publisher: Option<String>,
    pub assignee: Option<String>,
    pub subject: String,
    pub description: String,
    pub active_form: Option<String>,
    pub dependencies: Vec<u64>,
    pub blocks: Vec<u64>,
    pub created_at: u64,
    pub started_at: Option<u64>,
    pub completed_at: Option<u64>,
}

/// Session entry matching the vol-session wire format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEntry {
    pub id: String,
    pub session_id: String,
    pub created_at: i64,
    pub parent_id: Option<String>,
    #[serde(rename = "type")]
    pub entry_type: String,
    pub data: serde_json::Value,
}

/// Response from session.resume RPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionResumeResponse {
    pub session_id: String,
    pub entry_count: usize,
    pub entries: Vec<SessionEntry>,
}

/// Skill metadata returned by skill.list RPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillListEntry {
    pub id: String,
    pub name: String,
    pub version: String,
    pub scope: String,
    pub description: String,
    pub triggers: Vec<String>,
}

/// Connection state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConnectionState {
    Connecting,
    Connected,
    Disconnected,
}

/// Pending request callback.
type ResponseCallback = Box<dyn FnOnce(serde_json::Value)>;

/// Shared client state.
/// `next_id`, `pending` and `on_state_change` are separated from the WebSocket
/// so they can be borrowed independently without conflicting with the WS borrow
/// held by the active message handler.
struct ClientInner {
    ws: RefCell<web_sys::WebSocket>,
    url: String,
    state: Cell<ConnectionState>,
    event_tx: mpsc::UnboundedSender<AgentEvent>,
    /// Pending response callbacks keyed by request ID.
    pending: RefCell<HashMap<u64, ResponseCallback>>,
    /// Queued messages to send once the WebSocket opens.
    send_queue: RefCell<Vec<String>>,
    debug_state: RefCell<Option<Signal<crate::state::DebugState>>>,
    on_state_change: Cell<Option<Box<dyn Fn(ConnectionState)>>>,
    /// Next request ID — shared across clones via Rc.
    next_id: Cell<u64>,
}

/// WebSocket JSON-RPC client.
#[derive(Clone)]
pub struct JsonRpcClient {
    inner: Rc<ClientInner>,
    event_rx: Rc<RefCell<mpsc::UnboundedReceiver<AgentEvent>>>,
}

impl PartialEq for JsonRpcClient {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl JsonRpcClient {
    /// Create a new client and connect to the server.
    /// Auto-subscribes to agent events on connect (behaviour for data-plane connections).
    pub fn new(url: &str) -> Self {
        Self::new_inner(url, true)
    }

    /// Create a new client without auto-subscribing to agent events on connect.
    /// Use for control-plane connections that don't support `agent.subscribe`.
    pub fn new_without_auto_subscribe(url: &str) -> Self {
        Self::new_inner(url, false)
    }

    fn new_inner(url: &str, auto_subscribe: bool) -> Self {
        let ws = web_sys::WebSocket::new(url).expect("failed to create WebSocket");
        let (event_tx, event_rx) = mpsc::unbounded();

        let inner = Rc::new(ClientInner {
            ws: RefCell::new(ws),
            url: url.to_string(),
            state: Cell::new(ConnectionState::Connecting),
            event_tx,
            pending: RefCell::new(HashMap::new()),
            send_queue: RefCell::new(Vec::new()),
            debug_state: RefCell::new(None),
            on_state_change: Cell::new(None),
            next_id: Cell::new(1),
        });

        let client = Self {
            inner: inner.clone(),
            event_rx: Rc::new(RefCell::new(event_rx)),
        };

        // Set up message handler
        let inner_clone = inner.clone();
        let on_msg = Closure::wrap(Box::new(move |e: web_sys::MessageEvent| {
            if let Ok(data) = e.data().dyn_into::<js_sys::JsString>() {
                let data = data.as_string().unwrap();
                Self::handle_message(&inner_clone, &data);
            }
        }) as Box<dyn FnMut(_)>);
        inner
            .ws
            .borrow()
            .set_onmessage(Some(on_msg.as_ref().unchecked_ref()));
        on_msg.forget();

        // Set up close handler
        let inner_clone = inner.clone();
        let on_close = Closure::wrap(Box::new(move |_e: web_sys::CloseEvent| {
            inner_clone.state.set(ConnectionState::Disconnected);
            // Clear all pending callbacks — they will never complete
            let pending: Vec<_> = inner_clone.pending.borrow_mut().drain().collect();
            for (_, cb) in pending {
                let err = serde_json::json!({"error": {"message": "WebSocket disconnected"}});
                cb(err);
            }
            if let Some(cb) = inner_clone.on_state_change.take() {
                cb(ConnectionState::Disconnected);
                inner_clone.on_state_change.set(Some(cb));
            }
        }) as Box<dyn FnMut(_)>);
        inner
            .ws
            .borrow()
            .set_onclose(Some(on_close.as_ref().unchecked_ref()));
        on_close.forget();

        // Set up open handler — auto-subscribe to agent events (only for DP connections)
        let inner_open = inner.clone();
        let client_for_open = client.clone();
        let on_open = Closure::wrap(Box::new(move |_e: web_sys::Event| {
            inner_open.state.set(ConnectionState::Connected);
            // Flush queued messages
            let queue: Vec<String> = inner_open.send_queue.borrow_mut().drain(..).collect();
            for msg in queue {
                let _ = inner_open.ws.borrow().send_with_str(&msg);
            }
            if let Some(cb) = inner_open.on_state_change.take() {
                cb(ConnectionState::Connected);
                inner_open.on_state_change.set(Some(cb));
            }
            // Auto-subscribe to agent events on connect (skip for control-plane)
            if auto_subscribe {
                let _ = client_for_open.subscribe();
            }
        }) as Box<dyn FnMut(_)>);
        inner
            .ws
            .borrow()
            .set_onopen(Some(on_open.as_ref().unchecked_ref()));
        on_open.forget();

        client
    }

    /// Allocate a unique request ID.
    fn alloc_id(&self) -> u64 {
        let id = self.inner.next_id.get();
        self.inner.next_id.set(id.wrapping_add(1));
        id
    }

    /// Send a JSON-RPC message.
    fn send_raw(&self, msg: &str) -> Result<(), String> {
        let state = self.inner.ws.borrow().ready_state();
        let method = serde_json::from_str::<serde_json::Value>(msg)
            .ok()
            .and_then(|v| {
                v.get("method")
                    .and_then(|m| m.as_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or_default();
        log::info!("send_raw: method={method} state={state}");
        let result = match state {
            1 => {
                // OPEN
                self.inner
                    .ws
                    .borrow()
                    .send_with_str(msg)
                    .map_err(|e| format!("send failed: {e:?}"))
            }
            0 => {
                // CONNECTING — queue for on_open
                self.inner.send_queue.borrow_mut().push(msg.to_string());
                Ok(())
            }
            _ => {
                // CLOSING (2) or CLOSED (3)
                Err("WebSocket not connected".to_string())
            }
        };
        if result.is_ok() {
            self.push_debug_out(msg);
        }
        result
    }

    /// Set a callback for connection state changes.
    pub fn on_state_change(&self, cb: impl Fn(ConnectionState) + 'static) {
        self.inner.on_state_change.set(Some(Box::new(cb)));
    }

    /// Get the current connection state.
    pub fn state(&self) -> ConnectionState {
        self.inner.state.get()
    }

    /// Get the WebSocket URL this client connected to.
    pub fn url(&self) -> &str {
        &self.inner.url
    }

    /// Attach debug state for WS message capture.
    pub fn set_debug_state(&self, debug_state: Signal<crate::state::DebugState>) {
        *self.inner.debug_state.borrow_mut() = Some(debug_state);
    }

    fn push_debug_out(&self, msg: &str) {
        if let Some(ref ds) = *self.inner.debug_state.borrow() {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(msg) {
                let method = val
                    .get("method")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                ds.write_unchecked().push_ws(
                    crate::state::WsDirection::Out,
                    method,
                    msg.to_string(),
                );
            }
        }
    }

    /// Get the next event from the event stream (async).
    pub async fn next_event(&self) -> Option<AgentEvent> {
        self.event_rx.borrow_mut().next().await
    }

    /// Submit input to the agent. Returns the request ID.
    pub fn submit(&self, input: &str, target: Option<&str>) -> Result<String, String> {
        let id = self.alloc_id();
        let mut params = serde_json::json!({ "input": input });
        if let Some(t) = target {
            params["target"] = serde_json::Value::String(t.to_string());
        }
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "agent.submit",
            "params": params,
            "id": id,
        });
        let json = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
        self.send_raw(&json)?;

        Ok(id.to_string())
    }

    /// Subscribe to agent events.
    pub fn subscribe(&self) -> Result<(), String> {
        let id = self.alloc_id();

        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "agent.subscribe",
            "params": {},
            "id": id,
        });
        let json = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
        self.send_raw(&json)?;
        Ok(())
    }

    /// Reconnect by creating a new WebSocket and swapping the old one.
    /// Returns Ok(()) immediately — success is signaled via on_state_change callback.
    pub fn reconnect(&self) -> Result<(), String> {
        let new_ws = web_sys::WebSocket::new(&self.inner.url)
            .map_err(|e| format!("failed to create WebSocket: {e:?}"))?;

        let inner = self.inner.clone();
        let client = self.clone();

        let inner_msg = inner.clone();
        let on_msg = Closure::wrap(Box::new(move |e: web_sys::MessageEvent| {
            if let Ok(data) = e.data().dyn_into::<js_sys::JsString>() {
                let data = data.as_string().unwrap();
                Self::handle_message(&inner_msg, &data);
            }
        }) as Box<dyn FnMut(_)>);
        new_ws.set_onmessage(Some(on_msg.as_ref().unchecked_ref()));
        on_msg.forget();

        let inner_c = inner.clone();
        let on_close = Closure::wrap(Box::new(move |_e: web_sys::CloseEvent| {
            inner_c.state.set(ConnectionState::Disconnected);
            // Clear all pending callbacks — they will never complete
            let pending: Vec<_> = inner_c.pending.borrow_mut().drain().collect();
            for (_, cb) in pending {
                let err = serde_json::json!({"error": {"message": "WebSocket disconnected"}});
                cb(err);
            }
            if let Some(cb) = inner_c.on_state_change.take() {
                cb(ConnectionState::Disconnected);
                inner_c.on_state_change.set(Some(cb));
            }
        }) as Box<dyn FnMut(_)>);
        new_ws.set_onclose(Some(on_close.as_ref().unchecked_ref()));
        on_close.forget();

        let inner_o = inner.clone();
        let client_o = client.clone();
        let on_open = Closure::wrap(Box::new(move |_e: web_sys::Event| {
            inner_o.state.set(ConnectionState::Connected);
            let queue: Vec<String> = inner_o.send_queue.borrow_mut().drain(..).collect();
            for msg in queue {
                let _ = inner_o.ws.borrow().send_with_str(&msg);
            }
            if let Some(cb) = inner_o.on_state_change.take() {
                cb(ConnectionState::Connected);
                inner_o.on_state_change.set(Some(cb));
            }
            let _ = client_o.subscribe();
        }) as Box<dyn FnMut(_)>);
        new_ws.set_onopen(Some(on_open.as_ref().unchecked_ref()));
        on_open.forget();

        // Swap WebSocket — old WS is dropped, new one takes over
        *inner.ws.borrow_mut() = new_ws;
        inner.state.set(ConnectionState::Connecting);

        Ok(())
    }

    /// Cancel a running agent.
    pub fn cancel(&self, run_id: &str) -> Result<(), String> {
        let id = self.alloc_id();

        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "agent.cancel",
            "params": { "run_id": run_id },
            "id": id,
        });
        let json = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
        self.send_raw(&json)?;
        Ok(())
    }

    /// List a directory on the server. Returns entries asynchronously via callback.
    pub fn file_list(&self, path: &str, cb: impl FnOnce(Result<Vec<FileEntry>, String>) + 'static) {
        let id = self.alloc_id();

        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "file.list",
            "params": { "path": path },
            "id": id,
        });
        let json = match serde_json::to_string(&msg) {
            Ok(j) => j,
            Err(e) => {
                cb(Err(e.to_string()));
                return;
            }
        };
        if let Err(e) = self.send_raw(&json) {
            cb(Err(format!("send failed: {e:?}")));
            return;
        }

        // Register callback for when the response arrives.
        let cb: Box<dyn FnOnce(serde_json::Value)> =
            Box::new(
                move |result| match result.get("entries").and_then(|v| v.as_array()) {
                    Some(entries) => {
                        let parsed: Vec<FileEntry> = entries
                            .iter()
                            .filter_map(|e| serde_json::from_value(e.clone()).ok())
                            .collect();
                        cb(Ok(parsed));
                    }
                    None => cb(Err("no entries in response".to_string())),
                },
            );
        self.inner.pending.borrow_mut().insert(id, cb);
    }

    /// Read a file on the server. Returns content asynchronously via callback.
    pub fn file_read(&self, path: &str, cb: impl FnOnce(Result<String, String>) + 'static) {
        let id = self.alloc_id();

        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "file.read",
            "params": { "path": path },
            "id": id,
        });
        let json = match serde_json::to_string(&msg) {
            Ok(j) => j,
            Err(e) => {
                cb(Err(e.to_string()));
                return;
            }
        };
        if let Err(e) = self.send_raw(&json) {
            cb(Err(format!("send failed: {e:?}")));
            return;
        }

        let cb: Box<dyn FnOnce(serde_json::Value)> = Box::new(move |result| {
            if let Some(content) = result.get("content").and_then(|v| v.as_str()) {
                cb(Ok(content.to_string()));
            } else if let Some(error) = result.get("error").and_then(|v| v.as_str()) {
                cb(Err(error.to_string()));
            } else {
                cb(Err("no content in response".to_string()));
            }
        });
        self.inner.pending.borrow_mut().insert(id, cb);
    }

    /// List all registered agents on the server. Returns entries via callback.
    pub fn agent_list(&self, cb: impl FnOnce(Result<Vec<AgentListEntry>, String>) + 'static) {
        let id = self.alloc_id();

        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "agent.list",
            "params": {},
            "id": id,
        });
        let json = match serde_json::to_string(&msg) {
            Ok(j) => j,
            Err(e) => {
                cb(Err(e.to_string()));
                return;
            }
        };
        if let Err(e) = self.send_raw(&json) {
            cb(Err(format!("send failed: {e:?}")));
            return;
        }

        let cb: Box<dyn FnOnce(serde_json::Value)> =
            Box::new(
                move |result| match result.get("agents").and_then(|v| v.as_array()) {
                    Some(agents) => {
                        let parsed: Vec<AgentListEntry> = agents
                            .iter()
                            .filter_map(|e| serde_json::from_value(e.clone()).ok())
                            .collect();
                        cb(Ok(parsed));
                    }
                    None => cb(Err("no agents in response".to_string())),
                },
            );
        self.inner.pending.borrow_mut().insert(id, cb);
    }

    /// List all registered data-plane nodes. Returns entries via callback.
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
            Err(e) => {
                cb(Err(e.to_string()));
                return;
            }
        };
        if let Err(e) = self.send_raw(&json) {
            cb(Err(format!("send failed: {e:?}")));
            return;
        }
        let cb: Box<dyn FnOnce(serde_json::Value)> =
            Box::new(
                move |result| match result.get("nodes").and_then(|v| v.as_array()) {
                    Some(nodes) => {
                        let parsed: Vec<NodeListEntry> = nodes
                            .iter()
                            .filter_map(|n| serde_json::from_value(n.clone()).ok())
                            .collect();
                        cb(Ok(parsed));
                    }
                    None => cb(Err("no nodes in response".to_string())),
                },
            );
        self.inner.pending.borrow_mut().insert(id, cb);
    }

    /// Get a single node by ID. Returns `None` if the node is not found.
    pub fn node_get(
        &self,
        node_id: &str,
        cb: impl FnOnce(Result<Option<NodeRecord>, String>) + 'static,
    ) {
        let id = self.alloc_id();
        let params = serde_json::json!({ "node_id": node_id });
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "control.node_get",
            "params": params,
            "id": id,
        });
        let json = match serde_json::to_string(&msg) {
            Ok(j) => j,
            Err(e) => {
                cb(Err(e.to_string()));
                return;
            }
        };
        if let Err(e) = self.send_raw(&json) {
            cb(Err(format!("send failed: {e:?}")));
            return;
        }
        let cb: Box<dyn FnOnce(serde_json::Value)> = Box::new(move |result| {
            // Server returns NodeGetResult { node: Option<NodeRecord> };
            // data_json() strips the variant wrapper, leaving {"node": ...}.
            let node_value = result.get("node").unwrap_or(&result);
            if node_value.is_null() {
                cb(Ok(None));
                return;
            }
            match serde_json::from_value::<NodeRecord>(node_value.clone()) {
                Ok(r) => cb(Ok(Some(r))),
                Err(e) => cb(Err(e.to_string())),
            }
        });
        self.inner.pending.borrow_mut().insert(id, cb);
    }

    /// List capabilities for a node (or all nodes if `node_id` is `None`).
    pub fn capability_list(
        &self,
        node_id: Option<&str>,
        cb: impl FnOnce(Result<Vec<CapabilitySnapshot>, String>) + 'static,
    ) {
        let id = self.alloc_id();
        let params = serde_json::json!({ "node_id": node_id });
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "control.capability_list",
            "params": params,
            "id": id,
        });
        let json = match serde_json::to_string(&msg) {
            Ok(j) => j,
            Err(e) => {
                cb(Err(e.to_string()));
                return;
            }
        };
        if let Err(e) = self.send_raw(&json) {
            cb(Err(format!("send failed: {e:?}")));
            return;
        }
        let cb: Box<dyn FnOnce(serde_json::Value)> = Box::new(move |result| {
            // Server returns CapabilityListResult { snapshots: Vec<CapabilitySnapshot> };
            // data_json() strips the variant wrapper, leaving {"snapshots": [...]}.
            match result.get("snapshots").and_then(|v| v.as_array()) {
                Some(arr) => {
                    let parsed: Vec<CapabilitySnapshot> = arr
                        .iter()
                        .filter_map(|s| serde_json::from_value(s.clone()).ok())
                        .collect();
                    cb(Ok(parsed));
                }
                None => cb(Err("no snapshots in response".to_string())),
            }
        });
        self.inner.pending.borrow_mut().insert(id, cb);
    }

    /// Query agent running status.
    pub fn agent_status(
        &self,
        agent_id: &str,
        cb: impl FnOnce(Result<(String, Option<String>), String>) + 'static,
    ) {
        let id = self.alloc_id();
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "agent.status",
            "params": { "agent_id": agent_id },
            "id": id,
        });
        let json = match serde_json::to_string(&msg) {
            Ok(j) => j,
            Err(e) => {
                cb(Err(e.to_string()));
                return;
            }
        };
        if let Err(e) = self.send_raw(&json) {
            cb(Err(format!("send failed: {e:?}")));
            return;
        }
        let cb: Box<dyn FnOnce(serde_json::Value)> = Box::new(move |result| {
            let status = result
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("idle")
                .to_string();
            let run_id = result
                .get("run_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            cb(Ok((status, run_id)));
        });
        self.inner.pending.borrow_mut().insert(id, cb);
    }

    /// Fetch contributor metadata for an agent.
    pub fn agent_context_config(
        &self,
        agent_id: &str,
        cb: impl FnOnce(Result<Vec<crate::state::ContributorInfoEntry>, String>) + 'static,
    ) {
        let id = self.alloc_id();
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "agent.context_config",
            "params": { "agent_id": agent_id },
            "id": id,
        });
        let json = match serde_json::to_string(&msg) {
            Ok(j) => j,
            Err(e) => {
                cb(Err(e.to_string()));
                return;
            }
        };
        if let Err(e) = self.send_raw(&json) {
            cb(Err(format!("send failed: {e:?}")));
            return;
        }
        let cb: Box<dyn FnOnce(serde_json::Value)> =
            Box::new(
                move |result| match result.get("contributors").and_then(|v| v.as_array()) {
                    Some(arr) => {
                        let parsed: Vec<crate::state::ContributorInfoEntry> = arr
                            .iter()
                            .filter_map(|e| serde_json::from_value(e.clone()).ok())
                            .collect();
                        cb(Ok(parsed));
                    }
                    None => cb(Err("no contributors in response".to_string())),
                },
            );
        self.inner.pending.borrow_mut().insert(id, cb);
    }

    /// Fetch full message snapshot for a named contributor.
    pub fn agent_context_snapshot(
        &self,
        agent_id: &str,
        contributor_name: &str,
        cb: impl FnOnce(Result<Vec<crate::state::ContextMessageEntry>, String>) + 'static,
    ) {
        let id = self.alloc_id();
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "agent.context_snapshot",
            "params": { "agent_id": agent_id, "contributor_name": contributor_name },
            "id": id,
        });
        let json = match serde_json::to_string(&msg) {
            Ok(j) => j,
            Err(e) => {
                cb(Err(e.to_string()));
                return;
            }
        };
        if let Err(e) = self.send_raw(&json) {
            cb(Err(format!("send failed: {e:?}")));
            return;
        }
        let cb: Box<dyn FnOnce(serde_json::Value)> =
            Box::new(
                move |result| match result.get("messages").and_then(|v| v.as_array()) {
                    Some(arr) => {
                        let parsed: Vec<crate::state::ContextMessageEntry> = arr
                            .iter()
                            .filter_map(|e| serde_json::from_value(e.clone()).ok())
                            .collect();
                        cb(Ok(parsed));
                    }
                    None => cb(Err("no messages in response".to_string())),
                },
            );
        self.inner.pending.borrow_mut().insert(id, cb);
    }

    /// List all persisted sessions on the server. Returns entries via callback.
    pub fn session_list(
        &self,
        agent_id: Option<&str>,
        cb: impl FnOnce(Result<Vec<crate::state::SessionListEntry>, String>) + 'static,
    ) {
        let id = self.alloc_id();

        let mut params = serde_json::Map::new();
        if let Some(aid) = agent_id {
            params.insert("agent_id".to_string(), serde_json::json!(aid));
        }

        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "session.list",
            "params": params,
            "id": id,
        });
        let json = match serde_json::to_string(&msg) {
            Ok(j) => j,
            Err(e) => {
                cb(Err(e.to_string()));
                return;
            }
        };
        if let Err(e) = self.send_raw(&json) {
            cb(Err(format!("send failed: {e:?}")));
            return;
        }

        let cb: Box<dyn FnOnce(serde_json::Value)> =
            Box::new(
                move |result| match result.get("sessions").and_then(|v| v.as_array()) {
                    Some(sessions) => {
                        let parsed: Vec<crate::state::SessionListEntry> = sessions
                            .iter()
                            .filter_map(|s| {
                                let id = s.get("id").and_then(|v| v.as_str())?.to_string();
                                let entry_count =
                                    s.get("entry_count").and_then(|v| v.as_u64()).unwrap_or(0)
                                        as usize;
                                let created_at = s.get("created_at").and_then(|v| v.as_i64())?;
                                Some(crate::state::SessionListEntry {
                                    id,
                                    entry_count,
                                    created_at,
                                })
                            })
                            .collect();
                        cb(Ok(parsed));
                    }
                    None => cb(Err("no sessions in response".to_string())),
                },
            );
        self.inner.pending.borrow_mut().insert(id, cb);
    }

    /// Fetch all entries for a specific session. Returns entries via callback.
    pub fn session_entries(
        &self,
        session_id: &str,
        cb: impl FnOnce(Result<Vec<SessionEntry>, String>) + 'static,
    ) {
        let id = self.alloc_id();

        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "session.entries",
            "params": { "session_id": session_id },
            "id": id,
        });
        let json = match serde_json::to_string(&msg) {
            Ok(j) => j,
            Err(e) => {
                cb(Err(e.to_string()));
                return;
            }
        };
        if let Err(e) = self.send_raw(&json) {
            cb(Err(format!("send failed: {e:?}")));
            return;
        }

        let cb: Box<dyn FnOnce(serde_json::Value)> = Box::new(move |result| {
            if let Some(error) = result.get("error") {
                let msg = error
                    .get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("unknown RPC error");
                cb(Err(msg.to_string()));
                return;
            }
            match result.get("entries").and_then(|v| v.as_array()) {
                Some(entries) => {
                    let parsed: Vec<SessionEntry> = entries
                        .iter()
                        .filter_map(|e| serde_json::from_value(e.clone()).ok())
                        .collect();
                    cb(Ok(parsed));
                }
                None => cb(Err("no entries in response".to_string())),
            }
        });
        self.inner.pending.borrow_mut().insert(id, cb);
    }

    /// Resume a session on the server (swaps agent session). Returns response via callback.
    pub fn session_resume(
        &self,
        session_id: &str,
        agent_id: Option<&str>,
        cb: impl FnOnce(Result<SessionResumeResponse, String>) + 'static,
    ) {
        let id = self.alloc_id();
        log::info!("session_resume: id={id} session_id={session_id} agent_id={agent_id:?}");

        let mut params = serde_json::json!({ "session_id": session_id });
        if let Some(aid) = agent_id {
            params["agent_id"] = serde_json::json!(aid);
        }
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "session.resume",
            "params": params,
            "id": id,
        });
        let json = match serde_json::to_string(&msg) {
            Ok(j) => j,
            Err(e) => {
                cb(Err(e.to_string()));
                return;
            }
        };
        if let Err(e) = self.send_raw(&json) {
            cb(Err(format!("send failed: {e:?}")));
            return;
        }

        let cb: Box<dyn FnOnce(serde_json::Value)> = Box::new(move |result| {
            log::info!("session_resume response received");
            if let Some(error) = result.get("error") {
                let msg = error
                    .get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("unknown RPC error");
                log::error!("session_resume RPC error: {msg}");
                cb(Err(msg.to_string()));
                return;
            }
            let session_id = match result.get("session_id").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => {
                    log::error!("session_resume: no session_id in response");
                    cb(Err("no session_id in response".to_string()));
                    return;
                }
            };
            let entry_count = result
                .get("entry_count")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            let entries = result
                .get("entries")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|e| serde_json::from_value(e.clone()).ok())
                        .collect()
                })
                .unwrap_or_default();
            log::info!("session_resume success: session_id={session_id} entries={entry_count}");
            cb(Ok(SessionResumeResponse {
                session_id,
                entry_count,
                entries,
            }));
        });
        self.inner.pending.borrow_mut().insert(id, cb);
    }

    /// List all configured MCP servers.
    pub fn mcp_list_servers(&self, cb: impl FnOnce(Result<Vec<McpServerInfo>, String>) + 'static) {
        let id = self.alloc_id();
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "mcp.list_servers",
            "params": {},
            "id": id,
        });
        let json = match serde_json::to_string(&msg) {
            Ok(j) => j,
            Err(e) => {
                cb(Err(e.to_string()));
                return;
            }
        };
        if let Err(e) = self.send_raw(&json) {
            cb(Err(format!("send failed: {e:?}")));
            return;
        }
        let cb: Box<dyn FnOnce(serde_json::Value)> =
            Box::new(
                move |result| match result.get("servers").and_then(|v| v.as_array()) {
                    Some(servers) => {
                        let parsed: Vec<McpServerInfo> = servers
                            .iter()
                            .filter_map(|s| serde_json::from_value(s.clone()).ok())
                            .collect();
                        cb(Ok(parsed));
                    }
                    None => cb(Err("no servers in response".to_string())),
                },
            );
        self.inner.pending.borrow_mut().insert(id, cb);
    }

    /// List MCP tools across all servers.
    pub fn mcp_list_tools(
        &self,
        server: Option<&str>,
        cb: impl FnOnce(Result<Vec<McpToolInfo>, String>) + 'static,
    ) {
        let id = self.alloc_id();
        let mut params = serde_json::Map::new();
        if let Some(s) = server {
            params.insert("server".to_string(), serde_json::json!(s));
        }
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "mcp.list_tools",
            "params": params,
            "id": id,
        });
        let json = match serde_json::to_string(&msg) {
            Ok(j) => j,
            Err(e) => {
                cb(Err(e.to_string()));
                return;
            }
        };
        if let Err(e) = self.send_raw(&json) {
            cb(Err(format!("send failed: {e:?}")));
            return;
        }
        let cb: Box<dyn FnOnce(serde_json::Value)> =
            Box::new(
                move |result| match result.get("tools").and_then(|v| v.as_array()) {
                    Some(tools) => {
                        let parsed: Vec<McpToolInfo> = tools
                            .iter()
                            .filter_map(|t| serde_json::from_value(t.clone()).ok())
                            .collect();
                        cb(Ok(parsed));
                    }
                    None => cb(Err("no tools in response".to_string())),
                },
            );
        self.inner.pending.borrow_mut().insert(id, cb);
    }

    /// Call an MCP tool on a specific server.
    pub fn mcp_call_tool(
        &self,
        server: &str,
        tool_name: &str,
        arguments: serde_json::Value,
        cb: impl FnOnce(Result<String, String>) + 'static,
    ) {
        let id = self.alloc_id();
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "mcp.call_tool",
            "params": { "server": server, "tool_name": tool_name, "arguments": arguments },
            "id": id,
        });
        let json = match serde_json::to_string(&msg) {
            Ok(j) => j,
            Err(e) => {
                cb(Err(e.to_string()));
                return;
            }
        };
        if let Err(e) = self.send_raw(&json) {
            cb(Err(format!("send failed: {e:?}")));
            return;
        }
        let cb: Box<dyn FnOnce(serde_json::Value)> = Box::new(move |result| {
            if let Some(error) = result.get("error") {
                let msg = error
                    .get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("unknown error");
                cb(Err(msg.to_string()));
            } else if let Some(content) = result.get("result").and_then(|v| v.as_str()) {
                cb(Ok(content.to_string()));
            } else {
                cb(Err("no result in response".to_string()));
            }
        });
        self.inner.pending.borrow_mut().insert(id, cb);
    }

    /// List MCP resources across all servers.
    pub fn mcp_list_resources(
        &self,
        server: Option<&str>,
        cb: impl FnOnce(Result<Vec<McpResourceInfo>, String>) + 'static,
    ) {
        let id = self.alloc_id();
        let mut params = serde_json::Map::new();
        if let Some(s) = server {
            params.insert("server".to_string(), serde_json::json!(s));
        }
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "mcp.list_resources",
            "params": params,
            "id": id,
        });
        let json = match serde_json::to_string(&msg) {
            Ok(j) => j,
            Err(e) => {
                cb(Err(e.to_string()));
                return;
            }
        };
        if let Err(e) = self.send_raw(&json) {
            cb(Err(format!("send failed: {e:?}")));
            return;
        }
        let cb: Box<dyn FnOnce(serde_json::Value)> =
            Box::new(
                move |result| match result.get("resources").and_then(|v| v.as_array()) {
                    Some(resources) => {
                        let parsed: Vec<McpResourceInfo> = resources
                            .iter()
                            .filter_map(|r| serde_json::from_value(r.clone()).ok())
                            .collect();
                        cb(Ok(parsed));
                    }
                    None => cb(Err("no resources in response".to_string())),
                },
            );
        self.inner.pending.borrow_mut().insert(id, cb);
    }

    /// List MCP resource templates across all servers.
    pub fn mcp_list_resource_templates(
        &self,
        server: Option<&str>,
        cb: impl FnOnce(Result<Vec<McpResourceTemplateInfo>, String>) + 'static,
    ) {
        let id = self.alloc_id();
        let mut params = serde_json::Map::new();
        if let Some(s) = server {
            params.insert("server".to_string(), serde_json::json!(s));
        }
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "mcp.list_resource_templates",
            "params": params,
            "id": id,
        });
        let json = match serde_json::to_string(&msg) {
            Ok(j) => j,
            Err(e) => {
                cb(Err(e.to_string()));
                return;
            }
        };
        if let Err(e) = self.send_raw(&json) {
            cb(Err(format!("send failed: {e:?}")));
            return;
        }
        let cb: Box<dyn FnOnce(serde_json::Value)> =
            Box::new(
                move |result| match result.get("templates").and_then(|v| v.as_array()) {
                    Some(templates) => {
                        let parsed: Vec<McpResourceTemplateInfo> = templates
                            .iter()
                            .filter_map(|t| serde_json::from_value(t.clone()).ok())
                            .collect();
                        cb(Ok(parsed));
                    }
                    None => cb(Err("no templates in response".to_string())),
                },
            );
        self.inner.pending.borrow_mut().insert(id, cb);
    }

    /// Read an MCP resource by URI.
    pub fn mcp_read_resource(&self, uri: &str, cb: impl FnOnce(Result<String, String>) + 'static) {
        let id = self.alloc_id();
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "mcp.read_resource",
            "params": { "uri": uri },
            "id": id,
        });
        let json = match serde_json::to_string(&msg) {
            Ok(j) => j,
            Err(e) => {
                cb(Err(e.to_string()));
                return;
            }
        };
        if let Err(e) = self.send_raw(&json) {
            cb(Err(format!("send failed: {e:?}")));
            return;
        }
        let cb: Box<dyn FnOnce(serde_json::Value)> = Box::new(move |result| {
            if let Some(error) = result.get("error") {
                let msg = error
                    .get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("unknown error");
                cb(Err(msg.to_string()));
            } else if let Some(content) = result.get("content").and_then(|v| v.as_str()) {
                cb(Ok(content.to_string()));
            } else {
                cb(Err("no content in response".to_string()));
            }
        });
        self.inner.pending.borrow_mut().insert(id, cb);
    }

    /// List MCP prompts across all servers.
    pub fn mcp_list_prompts(
        &self,
        server: Option<&str>,
        cb: impl FnOnce(Result<Vec<McpPromptInfo>, String>) + 'static,
    ) {
        let id = self.alloc_id();
        let mut params = serde_json::Map::new();
        if let Some(s) = server {
            params.insert("server".to_string(), serde_json::json!(s));
        }
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "mcp.list_prompts",
            "params": params,
            "id": id,
        });
        let json = match serde_json::to_string(&msg) {
            Ok(j) => j,
            Err(e) => {
                cb(Err(e.to_string()));
                return;
            }
        };
        if let Err(e) = self.send_raw(&json) {
            cb(Err(format!("send failed: {e:?}")));
            return;
        }
        let cb: Box<dyn FnOnce(serde_json::Value)> =
            Box::new(
                move |result| match result.get("prompts").and_then(|v| v.as_array()) {
                    Some(prompts) => {
                        let parsed: Vec<McpPromptInfo> = prompts
                            .iter()
                            .filter_map(|p| serde_json::from_value(p.clone()).ok())
                            .collect();
                        cb(Ok(parsed));
                    }
                    None => cb(Err("no prompts in response".to_string())),
                },
            );
        self.inner.pending.borrow_mut().insert(id, cb);
    }

    /// Reconnect a disconnected MCP server.
    pub fn mcp_reconnect(&self, server: &str, cb: impl FnOnce(Result<bool, String>) + 'static) {
        let id = self.alloc_id();
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "mcp.reconnect",
            "params": { "server": server },
            "id": id,
        });
        let json = match serde_json::to_string(&msg) {
            Ok(j) => j,
            Err(e) => {
                cb(Err(e.to_string()));
                return;
            }
        };
        if let Err(e) = self.send_raw(&json) {
            cb(Err(format!("send failed: {e:?}")));
            return;
        }
        let cb: Box<dyn FnOnce(serde_json::Value)> = Box::new(move |result| {
            let success = result
                .get("success")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            cb(Ok(success));
        });
        self.inner.pending.borrow_mut().insert(id, cb);
    }

    /// List all discovered skills.
    pub fn skill_list(&self, cb: impl FnOnce(Result<Vec<SkillListEntry>, String>) + 'static) {
        let id = self.alloc_id();
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "skill.list",
            "params": {},
            "id": id,
        });
        let json = match serde_json::to_string(&msg) {
            Ok(j) => j,
            Err(e) => {
                cb(Err(e.to_string()));
                return;
            }
        };
        if let Err(e) = self.send_raw(&json) {
            cb(Err(format!("send failed: {e:?}")));
            return;
        }
        let cb: Box<dyn FnOnce(serde_json::Value)> =
            Box::new(
                move |result| match result.get("skills").and_then(|v| v.as_array()) {
                    Some(skills) => {
                        let parsed: Vec<SkillListEntry> = skills
                            .iter()
                            .filter_map(|s| serde_json::from_value(s.clone()).ok())
                            .collect();
                        cb(Ok(parsed));
                    }
                    None => cb(Err("no skills in response".to_string())),
                },
            );
        self.inner.pending.borrow_mut().insert(id, cb);
    }

    /// Get full skill details by name.
    pub fn skill_get(
        &self,
        name: &str,
        cb: impl FnOnce(Result<crate::state::SkillDetail, String>) + 'static,
    ) {
        let id = self.alloc_id();
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "skill.get",
            "params": { "name": name },
            "id": id,
        });
        let json = match serde_json::to_string(&msg) {
            Ok(j) => j,
            Err(e) => {
                cb(Err(e.to_string()));
                return;
            }
        };
        if let Err(e) = self.send_raw(&json) {
            cb(Err(format!("send failed: {e:?}")));
            return;
        }
        let cb: Box<dyn FnOnce(serde_json::Value)> = Box::new(move |result| {
            if let Some(error) = result.get("error") {
                let msg = error
                    .get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("unknown error");
                cb(Err(msg.to_string()));
            } else {
                // SkillGetResult has { skill, name }; extract the skill payload.
                let skill_payload = result.get("skill").unwrap_or(&result);
                match serde_json::from_value::<crate::state::SkillDetail>(skill_payload.clone()) {
                    Ok(detail) => cb(Ok(detail)),
                    Err(e) => cb(Err(format!("failed to parse skill: {e}"))),
                }
            }
        });
        self.inner.pending.borrow_mut().insert(id, cb);
    }

    /// Refresh skills by re-discovering from all roots.
    pub fn skill_refresh(&self, cb: impl FnOnce(Result<usize, String>) + 'static) {
        let id = self.alloc_id();
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "skill.refresh",
            "params": {},
            "id": id,
        });
        let json = match serde_json::to_string(&msg) {
            Ok(j) => j,
            Err(e) => {
                cb(Err(e.to_string()));
                return;
            }
        };
        if let Err(e) = self.send_raw(&json) {
            cb(Err(format!("send failed: {e:?}")));
            return;
        }
        let cb: Box<dyn FnOnce(serde_json::Value)> =
            Box::new(
                move |result| match result.get("discovered").and_then(|v| v.as_u64()) {
                    Some(count) => cb(Ok(count as usize)),
                    None => {
                        if let Some(error) = result.get("error") {
                            let msg = error
                                .get("message")
                                .and_then(|m| m.as_str())
                                .unwrap_or("unknown error");
                            cb(Err(msg.to_string()));
                        } else {
                            cb(Err("no discovered count in response".to_string()));
                        }
                    }
                },
            );
        self.inner.pending.borrow_mut().insert(id, cb);
    }

    /// List all system tools.
    pub fn tool_list(&self, cb: impl FnOnce(Result<Vec<serde_json::Value>, String>) + 'static) {
        let id = self.alloc_id();
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tool.list",
            "params": {},
            "id": id,
        });
        let json = match serde_json::to_string(&msg) {
            Ok(j) => j,
            Err(e) => {
                cb(Err(e.to_string()));
                return;
            }
        };
        if let Err(e) = self.send_raw(&json) {
            cb(Err(format!("send failed: {e:?}")));
            return;
        }
        let cb: Box<dyn FnOnce(serde_json::Value)> =
            Box::new(
                move |result| match result.get("tools").and_then(|v| v.as_array()) {
                    Some(tools) => cb(Ok(tools.clone())),
                    None => cb(Err("no tools in response".to_string())),
                },
            );
        self.inner.pending.borrow_mut().insert(id, cb);
    }

    /// Call a tool directly.
    pub fn tool_call(
        &self,
        tool_name: &str,
        arguments: &serde_json::Value,
        cb: impl FnOnce(Result<serde_json::Value, String>) + 'static,
    ) {
        let id = self.alloc_id();
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tool.call",
            "params": { "tool_name": tool_name, "arguments": arguments },
            "id": id,
        });
        let json = match serde_json::to_string(&msg) {
            Ok(j) => j,
            Err(e) => {
                cb(Err(e.to_string()));
                return;
            }
        };
        if let Err(e) = self.send_raw(&json) {
            cb(Err(format!("send failed: {e:?}")));
            return;
        }
        let cb: Box<dyn FnOnce(serde_json::Value)> = Box::new(move |result| {
            if let Some(error) = result.get("error") {
                let msg = error
                    .get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("unknown error");
                cb(Err(msg.to_string()));
            } else {
                cb(Ok(result.clone()));
            }
        });
        self.inner.pending.borrow_mut().insert(id, cb);
    }

    pub fn task_list(
        &self,
        status: Option<&str>,
        assignee: Option<&str>,
        cb: impl FnOnce(Result<Vec<TaskEntry>, String>) + 'static,
    ) {
        let id = self.alloc_id();

        let mut params = serde_json::Map::new();
        if let Some(s) = status {
            params.insert("status".to_string(), serde_json::json!(s));
        }
        if let Some(a) = assignee {
            params.insert("assignee".to_string(), serde_json::json!(a));
        }

        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "task.list",
            "params": params,
            "id": id,
        });
        let json = match serde_json::to_string(&msg) {
            Ok(j) => j,
            Err(e) => {
                cb(Err(e.to_string()));
                return;
            }
        };
        if let Err(e) = self.send_raw(&json) {
            cb(Err(format!("send failed: {e:?}")));
            return;
        }
        let cb: Box<dyn FnOnce(serde_json::Value)> =
            Box::new(
                move |result| match result.get("tasks").and_then(|v| v.as_array()) {
                    Some(tasks) => {
                        let parsed: Vec<TaskEntry> = tasks
                            .iter()
                            .filter_map(|t| serde_json::from_value(t.clone()).ok())
                            .collect();
                        cb(Ok(parsed));
                    }
                    None => cb(Err("no tasks in response".to_string())),
                },
            );
        self.inner.pending.borrow_mut().insert(id, cb);
    }

    pub fn task_get(&self, task_id: u64, cb: impl FnOnce(Result<TaskEntry, String>) + 'static) {
        let id = self.alloc_id();
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "task.get",
            "params": { "task_id": task_id },
            "id": id,
        });
        let json = match serde_json::to_string(&msg) {
            Ok(j) => j,
            Err(e) => {
                cb(Err(e.to_string()));
                return;
            }
        };
        if let Err(e) = self.send_raw(&json) {
            cb(Err(format!("send failed: {e:?}")));
            return;
        }
        let cb: Box<dyn FnOnce(serde_json::Value)> =
            Box::new(move |result| match result.get("task") {
                Some(task) => match serde_json::from_value(task.clone()) {
                    Ok(t) => cb(Ok(t)),
                    Err(e) => cb(Err(format!("parse error: {e}"))),
                },
                None => cb(Err("no task in response".to_string())),
            });
        self.inner.pending.borrow_mut().insert(id, cb);
    }

    fn push_debug_in(inner: &Rc<ClientInner>, data: &str) {
        if let Some(ref ds) = *inner.debug_state.borrow() {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(data) {
                let method = val
                    .get("method")
                    .and_then(|v| v.as_str())
                    .unwrap_or("<response>")
                    .to_string();
                ds.write_unchecked().push_ws(
                    crate::state::WsDirection::In,
                    method,
                    data.to_string(),
                );
            }
        }
    }

    fn handle_message(inner: &Rc<ClientInner>, data: &str) {
        log::info!("handle_message: {:.200}", data);
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(data) {
            if val.get("method").and_then(|m| m.as_str()) == Some("agent.event") {
                if let Some(params) = val.get("params") {
                    if let Ok(event) = serde_json::from_value::<AgentEvent>(params.clone()) {
                        let _ = inner.event_tx.unbounded_send(event);
                    } else {
                        log::warn!("Failed to parse AgentEvent: {}", params);
                    }
                }
            } else if let Some(id) = val.get("id").and_then(|v| v.as_u64()) {
                // Response to a request — check if we have a pending callback.
                let cb = inner.pending.borrow_mut().remove(&id);
                if let Some(cb) = cb {
                    if let Some(result) = val.get("result") {
                        cb(result.clone());
                    } else if let Some(error) = val.get("error") {
                        cb(error.clone());
                    }
                }
            }
        }
        // Capture for debug AFTER processing — never blocks message handling
        Self::push_debug_in(inner, data);
    }
}
