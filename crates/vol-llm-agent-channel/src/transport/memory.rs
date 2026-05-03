//! In-memory transport for local testing and inter-process communication.

use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::connection::Connection;
use crate::error::ConnectionError;
use crate::protocol::InboundMessage;
use crate::request::RunResult;
use super::ws::serialize_stream_event;

/// Internal envelope for outbound messages.
struct OutboundEnvelope {
    data: serde_json::Value,
}

/// In-memory connection for local testing and inter-process communication.
pub struct MemoryConnection {
    rx: mpsc::UnboundedReceiver<InboundMessage>,
    tx: mpsc::UnboundedSender<OutboundEnvelope>,
}

impl MemoryConnection {
    /// Create a paired connection + handle.
    ///
    /// The `MemoryConnection` side implements `Connection`.
    /// The `MemoryHandle` side is used by tests to send messages and receive output.
    pub fn new() -> (Self, MemoryHandle) {
        let (in_tx, in_rx) = mpsc::unbounded_channel::<InboundMessage>();
        let (out_tx, out_rx) = mpsc::unbounded_channel::<OutboundEnvelope>();
        (
            Self {
                rx: in_rx,
                tx: out_tx,
            },
            MemoryHandle {
                tx: in_tx,
                rx: out_rx,
            },
        )
    }
}

#[async_trait]
impl Connection for MemoryConnection {
    fn protocol(&self) -> &str {
        "memory"
    }

    async fn recv(&mut self) -> Option<Result<InboundMessage, ConnectionError>> {
        self.rx.recv().await.map(Ok)
    }

    async fn send_event(
        &self,
        event: &vol_llm_agent::react::AgentStreamEvent,
    ) -> Result<(), ConnectionError> {
        let event_json = serialize_stream_event(event);
        let envelope = OutboundEnvelope {
            data: serde_json::json!({ "type": "event", "event": event_json }),
        };
        self.tx
            .send(envelope)
            .map_err(|e| ConnectionError::ChannelError(e.to_string()))
    }

    async fn send_result(&self, result: &RunResult) -> Result<(), ConnectionError> {
        let response_value = match &result.response {
            Ok(resp) => serde_json::to_value(resp)
                .map_err(|e| ConnectionError::ChannelError(e.to_string()))?,
            Err(err) => serde_json::json!({ "error": err.to_string() }),
        };

        let result_payload = serde_json::json!({
            "req_id": result.req_id,
            "target_id": result.target_id,
            "run_id": result.run_id,
            "response": response_value,
        });

        let envelope = OutboundEnvelope {
            data: serde_json::json!({ "type": "result", "result": result_payload }),
        };
        self.tx
            .send(envelope)
            .map_err(|e| ConnectionError::ChannelError(e.to_string()))
    }
}

/// Test handle for controlling the memory connection from tests.
///
/// Send inbound messages to the connection, receive outbound messages.
pub struct MemoryHandle {
    tx: mpsc::UnboundedSender<InboundMessage>,
    rx: mpsc::UnboundedReceiver<OutboundEnvelope>,
}

impl MemoryHandle {
    /// Send an inbound message to the connection.
    pub fn send(&self, msg: InboundMessage) -> Result<(), &'static str> {
        self.tx.send(msg).map_err(|_| "connection closed")
    }

    /// Receive the next outbound message.
    pub async fn recv(&mut self) -> Option<serde_json::Value> {
        self.rx.recv().await.map(|e| e.data)
    }
}
