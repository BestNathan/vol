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

use super::serde_helpers::{to_jsonrpc_error, to_jsonrpc_event};

/// JSON-RPC connection over WebSocket.
pub struct JsonRpcConnection {
    ws_tx: Arc<tokio::sync::Mutex<futures::stream::SplitSink<WebSocket, WsMessage>>>,
    msg_rx: tokio::sync::Mutex<tokio::sync::mpsc::Receiver<Result<AgentServerMessage, ConnectionError>>>,
}

impl JsonRpcConnection {
    /// Create a new `JsonRpcConnection` and start the background reader.
    pub fn new(ws: WebSocket) -> Self {
        let (ws_tx, ws_rx) = ws.split();
        let (tx, rx) = tokio::sync::mpsc::channel::<Result<AgentServerMessage, ConnectionError>>(64);

        // Spawn background reader task.
        tokio::spawn(Self::reader(tx, ws_rx));

        Self {
            ws_tx: Arc::new(tokio::sync::Mutex::new(ws_tx)),
            msg_rx: tokio::sync::Mutex::new(rx),
        }
    }

    /// Background task: read WS frames, decode to AgentServerMessage, push into channel.
    async fn reader(
        tx: tokio::sync::mpsc::Sender<Result<AgentServerMessage, ConnectionError>>,
        mut ws_rx: futures::stream::SplitStream<WebSocket>,
    ) {
        loop {
            let msg = ws_rx.next().await;
            let Some(msg) = msg else {
                tracing::info!("WebSocket connection closed");
                break;
            };
            match msg {
                Ok(WsMessage::Text(text)) => {
                    match crate::gateway::jsonrpc_ws::decode_jsonrpc_frame(&text) {
                        Ok(agent_msg) => {
                            if tx.send(Ok(agent_msg)).await.is_err() {
                                break;
                            }
                        }
                        Err(e) => {
                            let _ = tx.send(Err(e)).await;
                        }
                    }
                }
                Ok(WsMessage::Close(_)) => {
                    tracing::info!("WebSocket close received");
                    break;
                }
                Ok(WsMessage::Ping(_)) => {}
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
    }
}

#[async_trait]
impl Connection for JsonRpcConnection {
    fn protocol(&self) -> &str {
        "jsonrpc-ws"
    }

    async fn recv(&self) -> Option<Result<AgentServerMessage, ConnectionError>> {
        self.msg_rx.lock().await.recv().await
    }

    async fn send(&self, msg: AgentServerMessage) -> Result<(), ConnectionError> {
        match (&msg.kind, &msg.operation, &msg.payload) {
            (MessageKind::Event, Operation::Agent(AgentOperation::Event), Payload::Agent(AgentPayload::Event { event, .. })) => {
                match serde_json::from_value::<vol_llm_agent::react::AgentStreamEvent>(event.clone()) {
                    Ok(agent_event) => {
                        let text = to_jsonrpc_event(&agent_event, 0, "");
                        let mut tx = self.ws_tx.lock().await;
                        tx.send(WsMessage::Text(text))
                            .await
                            .map_err(|e| ConnectionError::WsSendError(e.to_string()))
                    }
                    Err(e) => {
                        tracing::error!(%e, ?event, "failed to deserialize AgentStreamEvent in send");
                        let envelope = serde_json::json!({
                            "jsonrpc": "2.0",
                            "method": "agent.event",
                            "params": {
                                "subscription": 0,
                                "result": {
                                    "event_type": "unknown",
                                    "data": event,
                                },
                            },
                        });
                        let text = serde_json::to_string(&envelope)
                            .map_err(|e| ConnectionError::WsSendError(e.to_string()))?;
                        let mut tx = self.ws_tx.lock().await;
                        tx.send(WsMessage::Text(text))
                            .await
                            .map_err(|e| ConnectionError::WsSendError(e.to_string()))
                    }
                }
            }
            (MessageKind::Error, _, Payload::Error(ErrorPayload { message, .. })) => {
                let text = to_jsonrpc_error(None, -32000, message.clone());
                let mut tx = self.ws_tx.lock().await;
                tx.send(WsMessage::Text(text))
                    .await
                    .map_err(|e| ConnectionError::WsSendError(e.to_string()))
            }
            _ => {
                let text = crate::gateway::jsonrpc_ws::encode_jsonrpc_message(msg)
                    .map_err(|e| ConnectionError::WsSendError(e.to_string()))?;
                let mut tx = self.ws_tx.lock().await;
                tx.send(WsMessage::Text(text))
                    .await
                    .map_err(|e| ConnectionError::WsSendError(e.to_string()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
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
