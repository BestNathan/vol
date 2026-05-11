//! JSON-RPC WebSocket connection implementing the `Connection` trait.
//!
//! Provides a full JSON-RPC server over a single WebSocket connection,
//! handling agent operations (submit, cancel, subscribe, approve),
//! file operations (list, read), log operations (list, read),
//! and session operations (list, resume).

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::ws::{Message as WsMessage, WebSocket};
use futures::sink::SinkExt;
use futures::stream::StreamExt;

use crate::connection::{Connection, ConnectionHolder};
use crate::dispatcher::AgentDispatcher;
use crate::error::ConnectionError;
use crate::protocol::Message;
use crate::request::{AgentRequest, RunResult};
use crate::router::AgentRouter;

use super::serde_helpers::{parse_jsonrpc_request, to_jsonrpc_error, to_jsonrpc_event, to_jsonrpc_response, JsonRpcRequest};

/// JSON-RPC connection over WebSocket.
pub struct JsonRpcConnection {
    /// WebSocket text sender (mutex-wrapped for concurrent sends).
    ws_tx: Arc<tokio::sync::Mutex<futures::stream::SplitSink<WebSocket, WsMessage>>>,
    /// WebSocket text receiver.
    ws_rx: Arc<tokio::sync::Mutex<futures::stream::SplitStream<WebSocket>>>,
    /// Router for dispatching requests to registered agent dispatchers.
    router: AgentRouter,
    /// Per-agent dispatchers (used for cancel across all agents).
    dispatchers: HashMap<String, Arc<AgentDispatcher>>,
    /// Connection holders indexed by agent ID.
    holders: HashMap<String, Arc<ConnectionHolder>>,
    /// Currently active agent holder (last submitted agent).
    active_holder: Arc<tokio::sync::Mutex<Option<String>>>,
    /// Current request ID (for event forwarding).
    current_req_id: Arc<tokio::sync::Mutex<String>>,
    /// Active subscription IDs.
    subscribers: Arc<tokio::sync::Mutex<Vec<u64>>>,
    /// Next subscription ID counter.
    next_sub_id: std::sync::atomic::AtomicU64,
    /// Working directory for file operations.
    working_dir: String,
    /// Store directory for log/session operations.
    store_dir: String,
}

impl JsonRpcConnection {
    /// Create a new `JsonRpcConnection`.
    pub fn new(
        ws: WebSocket,
        router: AgentRouter,
        dispatchers: HashMap<String, Arc<AgentDispatcher>>,
        holders: HashMap<String, Arc<ConnectionHolder>>,
        working_dir: String,
        store_dir: String,
    ) -> Self {
        let (tx, rx) = ws.split();
        Self {
            ws_tx: Arc::new(tokio::sync::Mutex::new(tx)),
            ws_rx: Arc::new(tokio::sync::Mutex::new(rx)),
            router,
            dispatchers,
            holders,
            active_holder: Arc::new(tokio::sync::Mutex::new(None)),
            current_req_id: Arc::new(tokio::sync::Mutex::new(String::new())),
            subscribers: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            next_sub_id: std::sync::atomic::AtomicU64::new(1),
            working_dir,
            store_dir,
        }
    }

    /// Main connection loop: attach to holders, process frames, detach on exit.
    pub async fn run(self: Arc<Self>) {
        // Attach to all holders at startup so events from all agents flow through.
        for holder in self.holders.values() {
            holder.attach(self.clone()).await;
        }

        // Send connected notification.
        let connected = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "connected",
            "params": {
                "agents": self.holders.keys().cloned().collect::<Vec<_>>(),
            },
        });
        let _ = self.ws_tx.lock().await.send(WsMessage::Text(connected.to_string())).await;

        // Main frame processing loop.
        loop {
            let msg = {
                let mut rx = self.ws_rx.lock().await;
                rx.next().await
            };

            let Some(msg) = msg else {
                tracing::info!("WebSocket connection closed");
                break;
            };

            match msg {
                Ok(WsMessage::Text(text)) => {
                    if let Err(e) = self.handle_text_frame(&text).await {
                        tracing::warn!(%e, "error handling inbound frame");
                    }
                }
                Ok(WsMessage::Close(_)) => {
                    tracing::info!("WebSocket close received");
                    break;
                }
                Ok(WsMessage::Ping(data)) => {
                    tracing::debug!("WebSocket ping: {} bytes", data.len());
                    let mut tx = self.ws_tx.lock().await;
                    let _ = tx.send(WsMessage::Pong(data)).await;
                }
                Ok(WsMessage::Pong(_)) => {
                    // Ignore pong.
                }
                Ok(WsMessage::Binary(_)) => {
                    tracing::debug!("Ignoring binary message");
                }
                Err(e) => {
                    tracing::warn!(%e, "WebSocket receive error");
                    break;
                }
            }
        }

        // Detach from all holders on exit.
        for holder in self.holders.values() {
            holder.detach().await;
        }
    }

    /// Handle a text WebSocket frame.
    async fn handle_text_frame(&self, text: &str) -> Result<(), ConnectionError> {
        let request = match parse_jsonrpc_request(text) {
            Ok(req) => req,
            Err(e) => {
                let resp = to_jsonrpc_error(None, -32700, e);
                return self.send_ws_text(&resp).await;
            }
        };

        // Dispatch to handler based on request type.
        let result: String = match &request {
            JsonRpcRequest::AgentSubmit { id, input, target } => {
                self.handle_submit(*id, input.clone(), target.clone()).await
            }
            JsonRpcRequest::AgentCancel { id, req_id } => {
                self.handle_cancel(*id, req_id.clone()).await
            }
            JsonRpcRequest::AgentSubscribe { id } => {
                self.handle_subscribe(*id).await
            }
            JsonRpcRequest::AgentUnsubscribe { id } => {
                self.handle_unsubscribe(*id).await
            }
            JsonRpcRequest::AgentApprove { id, req_id, approved, reason } => {
                self.handle_approve(*id, req_id.clone(), *approved, reason.clone()).await
            }
            JsonRpcRequest::FileList { id, path } => {
                self.handle_file_list(*id, path.clone()).await
            }
            JsonRpcRequest::FileRead { id, path } => {
                self.handle_file_read(*id, path.clone()).await
            }
            JsonRpcRequest::LogList { id } => {
                self.handle_log_list(*id).await
            }
            JsonRpcRequest::LogRead { id, run_id } => {
                self.handle_log_read(*id, run_id.clone()).await
            }
            JsonRpcRequest::SessionList { id } => {
                self.handle_session_list(*id).await
            }
            JsonRpcRequest::SessionResume { id, session_id } => {
                self.handle_session_resume(*id, session_id.clone()).await
            }
            JsonRpcRequest::AgentList { id } => {
                return self.send_ws_text(&to_jsonrpc_error(Some(*id), -32601, "Method not yet implemented: agent.list".into())).await;
            }
            JsonRpcRequest::Unknown { id, method } => {
                return self.send_ws_text(&to_jsonrpc_error(*id, -32601, format!("Method not found: {method}"))).await;
            }
        };

        self.send_ws_text(&result).await
    }

    // === Handler methods ===

    /// Handle `agent.submit`: submit input to router, return `{ req_id }`.
    async fn handle_submit(&self, id: u64, input: String, target: Option<String>) -> String {
        // Use specified target, or fall back to first registered agent.
        let target_id = target
            .filter(|t| self.holders.contains_key(t))
            .or_else(|| self.holders.keys().next().cloned())
            .unwrap_or_else(|| "agent".to_string());

        let request = AgentRequest::new(&target_id, &input);
        let req_id = request.req_id.clone();

        // Set active holder for this agent.
        *self.active_holder.lock().await = Some(target_id.clone());

        // Submit via router.
        let rx = match self.router.send(&target_id, request).await {
            Ok(rx) => rx,
            Err(e) => {
                return to_jsonrpc_response(id, serde_json::json!({
                    "error": e.to_string(),
                }));
            }
        };

        // Only track current_req_id after successful submit.
        *self.current_req_id.lock().await = req_id.clone();

        // Spawn background task to await result and clear req_id.
        let current_req_id = self.current_req_id.clone();
        let _ = tokio::spawn(Self::process_run_result(rx, req_id.clone(), current_req_id));

        to_jsonrpc_response(id, serde_json::json!({ "req_id": req_id }))
    }

    /// Handle `agent.cancel`: cancel across all dispatchers.
    async fn handle_cancel(&self, id: u64, req_id: String) -> String {
        let mut cancelled = false;
        for dispatcher in self.dispatchers.values() {
            if dispatcher.cancel(&req_id).await {
                cancelled = true;
                break;
            }
        }
        to_jsonrpc_response(id, serde_json::json!({ "cancelled": cancelled }))
    }

    /// Handle `agent.subscribe`: add subscription ID to list.
    async fn handle_subscribe(&self, id: u64) -> String {
        let sub_id = self.next_sub_id.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.subscribers.lock().await.push(sub_id);
        to_jsonrpc_response(id, serde_json::json!({ "subscription_id": sub_id }))
    }

    /// Handle `agent.unsubscribe`: remove subscription ID from list.
    async fn handle_unsubscribe(&self, id: u64) -> String {
        // The id from the JSON-RPC request is treated as the subscription ID to remove.
        let mut subs = self.subscribers.lock().await;
        let removed = subs.iter().position(|&s| s == id).map(|i| subs.remove(i)).is_some();
        to_jsonrpc_response(id, serde_json::json!({ "unsubscribed": removed }))
    }

    /// Handle `agent.approve`: stub — always approved.
    async fn handle_approve(&self, id: u64, _req_id: String, _approved: bool, _reason: Option<String>) -> String {
        to_jsonrpc_response(id, serde_json::json!({ "approved": true }))
    }

    /// Handle `file.list`: list directory contents.
    async fn handle_file_list(&self, id: u64, path: String) -> String {
        match std::fs::read_dir(&path) {
            Ok(entries) => {
                let mut list: Vec<serde_json::Value> = Vec::new();
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
                    let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                    list.push(serde_json::json!({
                        "name": name,
                        "is_dir": is_dir,
                        "size": size,
                    }));
                }
                // Sort: directories first, then by name.
                list.sort_by(|a, b| {
                    let a_dir = a["is_dir"].as_bool().unwrap_or(false);
                    let b_dir = b["is_dir"].as_bool().unwrap_or(false);
                    b_dir.cmp(&a_dir).then_with(|| a["name"].as_str().cmp(&b["name"].as_str()))
                });
                to_jsonrpc_response(id, serde_json::json!({ "entries": list }))
            }
            Err(e) => to_jsonrpc_error(Some(id), -32000, format!("Failed to read directory: {e}")),
        }
    }

    /// Handle `file.read`: read file contents.
    async fn handle_file_read(&self, id: u64, path: String) -> String {
        match std::fs::read_to_string(&path) {
            Ok(content) => to_jsonrpc_response(id, serde_json::json!({ "content": content })),
            Err(e) => to_jsonrpc_error(Some(id), -32000, format!("Failed to read file: {e}")),
        }
    }

    /// Handle `log.list`: stub — returns empty list.
    async fn handle_log_list(&self, id: u64) -> String {
        to_jsonrpc_response(id, serde_json::json!({ "runs": [] }))
    }

    /// Handle `log.read`: stub — returns empty entries.
    async fn handle_log_read(&self, id: u64, _run_id: String) -> String {
        to_jsonrpc_response(id, serde_json::json!({ "entries": [] }))
    }

    /// Handle `session.list`: stub — returns empty list.
    async fn handle_session_list(&self, id: u64) -> String {
        to_jsonrpc_response(id, serde_json::json!({ "sessions": [] }))
    }

    /// Handle `session.resume`: stub — returns session info.
    async fn handle_session_resume(&self, id: u64, session_id: String) -> String {
        to_jsonrpc_response(id, serde_json::json!({
            "session_id": session_id,
            "entry_count": 0,
        }))
    }

    /// Background task: process agent run result.
    async fn process_run_result(
        rx: tokio::sync::oneshot::Receiver<RunResult>,
        req_id: String,
        current_req_id: Arc<tokio::sync::Mutex<String>>,
    ) {
        match rx.await {
            Ok(result) => {
                match &result.response {
                    Ok(response) => {
                        tracing::info!(%req_id, run_id = ?result.run_id, iterations = response.iterations, "agent run completed");
                    }
                    Err(e) => {
                        tracing::error!(%req_id, %e, "agent run failed");
                    }
                }
            }
            Err(_) => {
                tracing::warn!(%req_id, "agent run receiver dropped (possibly cancelled)");
            }
        }
        // Clear the current req_id after the run completes.
        *current_req_id.lock().await = String::new();
    }

    /// Send raw text over WebSocket.
    async fn send_ws_text(&self, text: &str) -> Result<(), ConnectionError> {
        let mut tx = self.ws_tx.lock().await;
        tx.send(WsMessage::Text(text.to_string()))
            .await
            .map_err(|e| ConnectionError::WsSendError(e.to_string()))
    }
}

#[async_trait]
impl Connection for JsonRpcConnection {
    fn protocol(&self) -> &str {
        "jsonrpc-ws"
    }

    async fn recv(&mut self) -> Option<Result<Message, ConnectionError>> {
        // Reading is done by the `run()` loop.
        None
    }

    async fn send(&self, msg: Message) -> Result<(), ConnectionError> {
        match msg {
            Message::Event { event, .. } => {
                let req_id = self.current_req_id.lock().await.clone();
                let sub_id = self.subscribers.lock().await.first().copied().unwrap_or(0);

                // Try to deserialize the event Value back to AgentStreamEvent
                match serde_json::from_value::<vol_llm_agent::react::AgentStreamEvent>(event.clone()) {
                    Ok(agent_event) => {
                        let text = to_jsonrpc_event(&agent_event, sub_id, &req_id);
                        self.send_ws_text(&text).await
                    }
                    Err(e) => {
                        tracing::error!(%e, ?event, "failed to deserialize AgentStreamEvent in Connection::send");
                        // Fallback: send raw event with unknown type
                        let envelope = serde_json::json!({
                            "jsonrpc": "2.0",
                            "method": "agent.event",
                            "params": {
                                "subscription": sub_id,
                                "result": {
                                    "req_id": req_id,
                                    "event_type": "unknown",
                                    "data": event,
                                },
                            },
                        });
                        let text = serde_json::to_string(&envelope)
                            .map_err(|e| ConnectionError::WsSendError(e.to_string()))?;
                        self.send_ws_text(&text).await
                    }
                }
            }
            // Connected, Result, Error are sent directly by the run() loop or handlers.
            _ => Ok(()),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use vol_llm_agent::react::AgentStreamEvent;

    #[test]
    fn test_jsonrpc_event_format() {
        // Verify that to_jsonrpc_event produces the expected JSON structure.
        use crate::jsonrpc::serde_helpers::to_jsonrpc_event;

        let event = AgentStreamEvent::agent_start("hello world".to_string());
        let json = to_jsonrpc_event(&event, 1, "req-abc-123");

        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");

        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["method"], "agent.event");
        assert_eq!(parsed["params"]["subscription"], 1);
        assert_eq!(parsed["params"]["result"]["req_id"], "req-abc-123");
        assert_eq!(parsed["params"]["result"]["event_type"], "agent_start");
        assert_eq!(parsed["params"]["result"]["data"]["input"], "hello world");
    }
}
