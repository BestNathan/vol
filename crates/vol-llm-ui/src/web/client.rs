//! WebSocket JSON-RPC client for the web frontend.
//!
//! Connects to the agent JSON-RPC server at ws://<host>:3001 and provides:
//! - `submit` — send `agent.submit` request
//! - `subscribe` — send `agent.subscribe` request
//! - Event stream via callback
//!
//! Server event format (jsonrpsee subscription):
//! ```json
//! {"jsonrpc":"2.0","method":"agent.event","params":{"subscription":N,"result":{...}}}
//! ```

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use futures_channel::mpsc;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

/// Agent event received from the server subscription.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEvent {
    pub req_id: String,
    #[serde(rename = "event_type")]
    pub event_type: String,
    pub data: serde_json::Value,
}

/// File entry returned by file.list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
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
/// `next_id` and `on_state_change` are separated from the WebSocket so they
/// can be borrowed independently without conflicting with the WS borrow held
/// by the active message handler.
struct ClientInner {
    ws: web_sys::WebSocket,
    url: String,
    state: Cell<ConnectionState>,
    event_tx: mpsc::UnboundedSender<AgentEvent>,
    /// Pending response callbacks keyed by request ID.
    pending: RefCell<HashMap<u64, ResponseCallback>>,
    on_state_change: Cell<Option<Box<dyn Fn(ConnectionState)>>>,
}

/// WebSocket JSON-RPC client.
#[derive(Clone)]
pub struct JsonRpcClient {
    inner: Rc<ClientInner>,
    event_rx: Rc<RefCell<mpsc::UnboundedReceiver<AgentEvent>>>,
    next_id: Cell<u64>,
}

impl JsonRpcClient {
    /// Create a new client and connect to the server.
    pub fn new(url: &str) -> Self {
        let ws = web_sys::WebSocket::new(url).expect("failed to create WebSocket");
        let (event_tx, event_rx) = mpsc::unbounded();

        let inner = Rc::new(ClientInner {
            ws,
            url: url.to_string(),
            state: Cell::new(ConnectionState::Connecting),
            event_tx,
            pending: RefCell::new(HashMap::new()),
            on_state_change: Cell::new(None),
        });

        let client = Self {
            inner: inner.clone(),
            event_rx: Rc::new(RefCell::new(event_rx)),
            next_id: Cell::new(1),
        };

        // Set up message handler
        let inner_clone = inner.clone();
        let on_msg = Closure::wrap(Box::new(move |e: web_sys::MessageEvent| {
            if let Ok(data) = e.data().dyn_into::<js_sys::JsString>() {
                let data = data.as_string().unwrap();
                Self::handle_message(&inner_clone, &data);
            }
        }) as Box<dyn FnMut(_)>);
        inner.ws.set_onmessage(Some(on_msg.as_ref().unchecked_ref()));
        on_msg.forget();

        // Set up close handler
        let inner_clone = inner.clone();
        let on_close = Closure::wrap(Box::new(move |_e: web_sys::CloseEvent| {
            inner_clone.state.set(ConnectionState::Disconnected);
            if let Some(cb) = inner_clone.on_state_change.take() {
                cb(ConnectionState::Disconnected);
                inner_clone.on_state_change.set(Some(cb));
            }
        }) as Box<dyn FnMut(_)>);
        inner.ws.set_onclose(Some(on_close.as_ref().unchecked_ref()));
        on_close.forget();

        // Set up open handler — auto-subscribe to agent events
        let inner_open = inner.clone();
        let client_for_open = client.clone();
        let on_open = Closure::wrap(Box::new(move |_e: web_sys::Event| {
            inner_open.state.set(ConnectionState::Connected);
            if let Some(cb) = inner_open.on_state_change.take() {
                cb(ConnectionState::Connected);
                inner_open.on_state_change.set(Some(cb));
            }
            // Auto-subscribe to agent events on connect
            let _ = client_for_open.subscribe();
        }) as Box<dyn FnMut(_)>);
        inner.ws.set_onopen(Some(on_open.as_ref().unchecked_ref()));
        on_open.forget();

        client
    }

    /// Allocate a unique request ID.
    fn alloc_id(&self) -> u64 {
        let id = self.next_id.get();
        self.next_id.set(id.wrapping_add(1));
        id
    }

    /// Send a JSON-RPC message without holding any borrows across the send.
    fn send_raw(&self, msg: &str) -> Result<(), String> {
        self.inner.ws.send_with_str(msg).map_err(|e| format!("send failed: {e:?}"))
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

    /// Get the next event from the event stream (async).
    pub async fn next_event(&self) -> Option<AgentEvent> {
        self.event_rx.borrow_mut().next().await
    }

    /// Submit input to the agent. Returns the request ID.
    pub fn submit(&self, input: &str) -> Result<String, String> {
        let id = self.alloc_id();

        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "agent.submit",
            "params": { "input": input },
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

    /// Cancel a running agent.
    pub fn cancel(&self, req_id: &str) -> Result<(), String> {
        let id = self.alloc_id();

        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "agent.cancel",
            "params": { "req_id": req_id },
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
            Err(e) => { cb(Err(e.to_string())); return; }
        };
        if let Err(e) = self.send_raw(&json) {
            cb(Err(format!("send failed: {e:?}")));
            return;
        }

        // Register callback for when the response arrives.
        let cb: Box<dyn FnOnce(serde_json::Value)> = Box::new(move |result| {
            match result.get("entries").and_then(|v| v.as_array()) {
                Some(entries) => {
                    let parsed: Vec<FileEntry> = entries.iter()
                        .filter_map(|e| serde_json::from_value(e.clone()).ok())
                        .collect();
                    cb(Ok(parsed));
                }
                None => cb(Err("no entries in response".to_string())),
            }
        });
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
            Err(e) => { cb(Err(e.to_string())); return; }
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
            Err(e) => { cb(Err(e.to_string())); return; }
        };
        if let Err(e) = self.send_raw(&json) {
            cb(Err(format!("send failed: {e:?}")));
            return;
        }

        let cb: Box<dyn FnOnce(serde_json::Value)> = Box::new(move |result| {
            match result.get("agents").and_then(|v| v.as_array()) {
                Some(agents) => {
                    let parsed: Vec<AgentListEntry> = agents.iter()
                        .filter_map(|e| serde_json::from_value(e.clone()).ok())
                        .collect();
                    cb(Ok(parsed));
                }
                None => cb(Err("no agents in response".to_string())),
            }
        });
        self.inner.pending.borrow_mut().insert(id, cb);
    }

    fn handle_message(inner: &Rc<ClientInner>, data: &str) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(data) {
            if val.get("method").and_then(|m| m.as_str()) == Some("agent.event") {
                if let Some(params) = val.get("params") {
                    if let Some(result) = params.get("result") {
                        if let Ok(event) = serde_json::from_value::<AgentEvent>(result.clone()) {
                            let _ = inner.event_tx.unbounded_send(event);
                        } else {
                            log::warn!("Failed to parse AgentEvent: {}", result);
                        }
                    }
                }
            } else if let Some(id) = val.get("id").and_then(|v| v.as_u64()) {
                // Response to a request — check if we have a pending callback.
                let cb = inner.pending.borrow_mut().remove(&id);
                if let Some(cb) = cb {
                    if let Some(result) = val.get("result") {
                        cb(result.clone());
                    } else if let Some(error) = val.get("error") {
                        log::error!("RPC error response for id={id}: {}", error);
                    }
                }
            }
        }
    }
}
