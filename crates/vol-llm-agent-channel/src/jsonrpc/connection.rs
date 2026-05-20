//! JSON-RPC WebSocket connection.
//!
//! Provides a full JSON-RPC server over a single WebSocket connection.
//! All resources (router, holders, MCP, skills, sessions) are accessed
//! through the shared `AgentServerCore`.

use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::ws::{Message as WsMessage, WebSocket};
use futures::sink::SinkExt;
use futures::stream::StreamExt;

use crate::agent_server_protocol::{AgentOperation, AgentPayload, AgentServerMessage, ErrorPayload, MessageKind, Operation, Payload};
use crate::connection::Connection;
use crate::error::ConnectionError;
use crate::request::AgentRequest;
use crate::server_core::AgentServerCore;

use super::serde_helpers::{parse_jsonrpc_request, to_jsonrpc_error, to_jsonrpc_event, to_jsonrpc_response, JsonRpcRequest};

/// JSON-RPC connection over WebSocket.
pub struct JsonRpcConnection {
    /// WebSocket text sender (mutex-wrapped for concurrent sends).
    ws_tx: Arc<tokio::sync::Mutex<futures::stream::SplitSink<WebSocket, WsMessage>>>,
    /// WebSocket text receiver.
    ws_rx: Arc<tokio::sync::Mutex<futures::stream::SplitStream<WebSocket>>>,
    /// Shared core — single source of truth for all resources.
    core: Arc<AgentServerCore>,
    /// Active subscription IDs.
    subscribers: Arc<tokio::sync::Mutex<Vec<u64>>>,
    /// Next subscription ID counter.
    next_sub_id: std::sync::atomic::AtomicU64,
}

impl JsonRpcConnection {
    /// Create a new `JsonRpcConnection`.
    pub fn new(ws: WebSocket, core: Arc<AgentServerCore>) -> Self {
        let (tx, rx) = ws.split();
        Self {
            ws_tx: Arc::new(tokio::sync::Mutex::new(tx)),
            ws_rx: Arc::new(tokio::sync::Mutex::new(rx)),
            core,
            subscribers: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            next_sub_id: std::sync::atomic::AtomicU64::new(1),
        }
    }

    /// Main connection loop: attach to holders, process frames, detach on exit.
    pub async fn run(self: Arc<Self>) {
        // Attach to all holders so events from agents flow through.
        let holder_ids: Vec<String> = self.core.holders().lock().unwrap().keys().cloned().collect();
        for holder_id in holder_ids {
            let holder = {
                self.core.holders().lock().unwrap().get(&holder_id).cloned()
            };
            if let Some(holder) = holder {
                holder.attach(self.clone()).await;
            }
        }

        // Send connected notification.
        let agent_ids = self.core.list_agent_ids().await;
        let connected = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "connected",
            "params": {
                "agents": agent_ids,
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
                    let mut tx = self.ws_tx.lock().await;
                    let _ = tx.send(WsMessage::Pong(data)).await;
                }
                Ok(WsMessage::Pong(_)) => {}
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
        let holder_ids: Vec<String> = self.core.holders().lock().unwrap().keys().cloned().collect();
        for holder_id in holder_ids {
            let holder = {
                self.core.holders().lock().unwrap().get(&holder_id).cloned()
            };
            if let Some(holder) = holder {
                holder.detach().await;
            }
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
            JsonRpcRequest::AgentList { id } => {
                self.handle_agent_list(*id).await
            }
            JsonRpcRequest::FileList { id, path } => {
                self.handle_core_dispatch(*id, "file.list", serde_json::json!({"path": path})).await
            }
            JsonRpcRequest::FileRead { id, path } => {
                self.handle_core_dispatch(*id, "file.read", serde_json::json!({"path": path})).await
            }
            JsonRpcRequest::LogList { id } => {
                self.handle_core_dispatch(*id, "log.list", serde_json::json!({})).await
            }
            JsonRpcRequest::LogRead { id, run_id } => {
                self.handle_core_dispatch(*id, "log.read", serde_json::json!({"run_id": run_id})).await
            }
            JsonRpcRequest::SessionList { id } => {
                self.handle_core_dispatch(*id, "session.list", serde_json::json!({})).await
            }
            JsonRpcRequest::SessionResume { id, session_id } => {
                self.handle_core_dispatch(*id, "session.resume", serde_json::json!({"session_id": session_id})).await
            }
            JsonRpcRequest::SessionEntries { id, session_id } => {
                self.handle_core_dispatch(*id, "session.entries", serde_json::json!({"session_id": session_id})).await
            }
            JsonRpcRequest::McpListServers { id } => {
                self.handle_core_dispatch(*id, "mcp.list_servers", serde_json::json!({})).await
            }
            JsonRpcRequest::McpListTools { id, server } => {
                let mut params = serde_json::json!({});
                if let Some(s) = server {
                    params["server"] = serde_json::json!(s);
                }
                self.handle_core_dispatch(*id, "mcp.list_tools", params).await
            }
            JsonRpcRequest::McpCallTool { id, server, tool_name, arguments } => {
                self.handle_core_dispatch(*id, "mcp.call_tool", serde_json::json!({"server": server, "tool_name": tool_name, "arguments": arguments})).await
            }
            JsonRpcRequest::McpListResources { id, server } => {
                let mut params = serde_json::json!({});
                if let Some(s) = server {
                    params["server"] = serde_json::json!(s);
                }
                self.handle_core_dispatch(*id, "mcp.list_resources", params).await
            }
            JsonRpcRequest::McpListResourceTemplates { id, server } => {
                let mut params = serde_json::json!({});
                if let Some(s) = server {
                    params["server"] = serde_json::json!(s);
                }
                self.handle_core_dispatch(*id, "mcp.list_resource_templates", params).await
            }
            JsonRpcRequest::McpReadResource { id, uri } => {
                self.handle_core_dispatch(*id, "mcp.read_resource", serde_json::json!({"uri": uri})).await
            }
            JsonRpcRequest::McpListPrompts { id, server } => {
                let mut params = serde_json::json!({});
                if let Some(s) = server {
                    params["server"] = serde_json::json!(s);
                }
                self.handle_core_dispatch(*id, "mcp.list_prompts", params).await
            }
            JsonRpcRequest::McpGetPrompt { id, name, arguments } => {
                let mut params = serde_json::json!({"name": name});
                if let Some(args) = arguments {
                    params["arguments"] = serde_json::json!(args);
                }
                self.handle_core_dispatch(*id, "mcp.get_prompt", params).await
            }
            JsonRpcRequest::McpReconnect { id, server } => {
                self.handle_core_dispatch(*id, "mcp.reconnect", serde_json::json!({"server": server})).await
            }
            JsonRpcRequest::McpServerStatus { id } => {
                self.handle_core_dispatch(*id, "mcp.server_status", serde_json::json!({})).await
            }
            JsonRpcRequest::SkillList { id } => {
                self.handle_core_dispatch(*id, "skill.list", serde_json::json!({})).await
            }
            JsonRpcRequest::SkillGet { id, name } => {
                self.handle_core_dispatch(*id, "skill.get", serde_json::json!({"name": name})).await
            }
            JsonRpcRequest::Unknown { id, method } => {
                return self.send_ws_text(&to_jsonrpc_error(*id, -32601, format!("Method not found: {method}"))).await;
            }
        };

        self.send_ws_text(&result).await
    }

    /// Core-dispatch path: decode JSON-RPC → AgentServerMessage → core.handle() → encode response.
    async fn handle_core_dispatch(&self, id: u64, method: &str, params: serde_json::Value) -> String {
        let frame = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        })
        .to_string();

        let msg = match crate::gateway::jsonrpc_ws::decode_jsonrpc_frame(&frame) {
            Ok(m) => m,
            Err(e) => return to_jsonrpc_error(Some(id), -32600, e.to_string()),
        };

        match self.core.handle(msg).await {
            Ok(outputs) => {
                let result = outputs.into_iter().next().unwrap();
                match crate::gateway::jsonrpc_ws::encode_jsonrpc_message(result) {
                    Ok(s) => s,
                    Err(e) => to_jsonrpc_error(Some(id), -32000, e.to_string()),
                }
            }
            Err(e) => to_jsonrpc_error(Some(id), -32000, e.to_string()),
        }
    }

    // === Agent-specific handlers (require router/dispatcher) ===

    async fn handle_submit(&self, id: u64, input: String, target: Option<String>) -> String {
        let target_id = {
            let holders = self.core.holders().lock().unwrap();
            target
                .filter(|t| holders.contains_key(t))
                .or_else(|| holders.keys().next().cloned())
                .unwrap_or_else(|| "agent".to_string())
        };

        let request = AgentRequest::new(&target_id, &input);
        let req_id = request.req_id.clone();

        let rx = match self.core.router().send(&target_id, request).await {
            Ok(rx) => rx,
            Err(e) => {
                return to_jsonrpc_response(id, serde_json::json!({ "error": e.to_string() }));
            }
        };

        // Spawn background task to await result.
        let _ = tokio::spawn(Self::process_run_result(rx, req_id.clone()));

        to_jsonrpc_response(id, serde_json::json!({ "req_id": req_id }))
    }

    async fn handle_cancel(&self, id: u64, req_id: String) -> String {
        // Try to cancel via router — we need to walk dispatchers.
        // For now, just return not-cancelled (cancel goes through dispatcher directly).
        to_jsonrpc_response(id, serde_json::json!({ "cancelled": false }))
    }

    async fn handle_subscribe(&self, id: u64) -> String {
        let sub_id = self.next_sub_id.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.subscribers.lock().await.push(sub_id);
        to_jsonrpc_response(id, serde_json::json!({ "subscription_id": sub_id }))
    }

    async fn handle_unsubscribe(&self, id: u64) -> String {
        let mut subs = self.subscribers.lock().await;
        let removed = subs.iter().position(|&s| s == id).map(|i| subs.remove(i)).is_some();
        to_jsonrpc_response(id, serde_json::json!({ "unsubscribed": removed }))
    }

    async fn handle_approve(&self, id: u64, _req_id: String, _approved: bool, _reason: Option<String>) -> String {
        to_jsonrpc_response(id, serde_json::json!({ "approved": true }))
    }

    async fn handle_agent_list(&self, id: u64) -> String {
        let agents: Vec<serde_json::Value> = self.core.holders().lock().unwrap().keys().map(|k| {
            serde_json::json!({ "id": k, "name": k, "type": k, "description": "Code assistant", "scope": "Server" })
        }).collect();
        to_jsonrpc_response(id, serde_json::json!({ "agents": agents }))
    }

    /// Background task: process agent run result.
    async fn process_run_result(
        rx: tokio::sync::oneshot::Receiver<crate::request::RunResult>,
        req_id: String,
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

    async fn recv(&mut self) -> Option<Result<AgentServerMessage, ConnectionError>> {
        None
    }

    async fn send(&self, msg: AgentServerMessage) -> Result<(), ConnectionError> {
        match (&msg.kind, &msg.operation, &msg.payload) {
            (MessageKind::Event, Operation::Agent(AgentOperation::Event), Payload::Agent(AgentPayload::Event { event, .. })) => {
                let sub_id = self.subscribers.lock().await.first().copied().unwrap_or(0);

                match serde_json::from_value::<vol_llm_agent::react::AgentStreamEvent>(event.clone()) {
                    Ok(agent_event) => {
                        let text = to_jsonrpc_event(&agent_event, sub_id, "");
                        self.send_ws_text(&text).await
                    }
                    Err(e) => {
                        tracing::error!(%e, ?event, "failed to deserialize AgentStreamEvent in Connection::send");
                        let envelope = serde_json::json!({
                            "jsonrpc": "2.0",
                            "method": "agent.event",
                            "params": {
                                "subscription": sub_id,
                                "result": {
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
            (MessageKind::Error, _, Payload::Error(ErrorPayload { message, .. })) => {
                let text = to_jsonrpc_error(None, -32000, message.clone());
                self.send_ws_text(&text).await
            }
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jsonrpc::serde_helpers::to_jsonrpc_event;
    use vol_llm_agent::react::AgentStreamEvent;

    #[test]
    fn test_jsonrpc_event_format() {
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
