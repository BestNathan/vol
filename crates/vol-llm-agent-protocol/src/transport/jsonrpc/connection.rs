//! JSON-RPC WebSocket connection.
//!
//! Provides a full JSON-RPC server over a single WebSocket connection.
//! Domain behavior is delegated through the shared JSON-RPC message service.

use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::ws::{Message as WsMessage, WebSocket};
use futures::sink::SinkExt;
use futures::stream::StreamExt;

use crate::agent_server_protocol::AgentServerMessage;
use crate::connection::Connection;
use crate::error::ConnectionError;

/// JSON-RPC connection over WebSocket.
pub struct JsonRpcConnection {
    ws_tx: Arc<tokio::sync::Mutex<futures::stream::SplitSink<WebSocket, WsMessage>>>,
    msg_rx: tokio::sync::Mutex<
        tokio::sync::mpsc::Receiver<Result<AgentServerMessage, ConnectionError>>,
    >,
}

impl JsonRpcConnection {
    /// Create a new `JsonRpcConnection` and start the background reader.
    pub fn new(ws: WebSocket) -> Self {
        let (ws_tx, ws_rx) = ws.split();
        let (tx, rx) =
            tokio::sync::mpsc::channel::<Result<AgentServerMessage, ConnectionError>>(64);

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
                    match crate::transport::jsonrpc::codec::decode_jsonrpc_frame(&text) {
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
                    tracing::debug!(%e, "WebSocket receive ended");
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
        let text = crate::transport::jsonrpc::codec::encode_jsonrpc_message(msg)
            .map_err(|e| ConnectionError::WsSendError(e.to_string()))?;
        let mut tx = self.ws_tx.lock().await;
        tx.send(WsMessage::Text(text))
            .await
            .map_err(|e| ConnectionError::WsSendError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_server_protocol::{AgentOperation, AgentPayload, Operation, Payload};
    use crate::transport::jsonrpc::codec::encode_jsonrpc_message;

    #[test]
    fn test_send_event_uses_codec_format() {
        let msg = AgentServerMessage::new_event(
            "msg-1",
            Operation::Agent(AgentOperation::Event),
            Payload::Agent(AgentPayload::Event {
                run_id: "run-1".to_string(),
                event: serde_json::json!({"AgentStart": {"input": "hello"}}),
            }),
        );
        let json = encode_jsonrpc_message(msg).unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["method"], "agent.event");
        // New format: params has run_id and event directly (flat)
        assert_eq!(parsed["params"]["run_id"], "run-1");
        assert_eq!(parsed["params"]["event"]["AgentStart"]["input"], "hello");
        // Old subscription/result nesting must NOT exist
        assert!(parsed["params"].get("subscription").is_none());
        assert!(parsed["params"].get("result").is_none());
    }
}
