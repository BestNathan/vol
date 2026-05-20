//! In-memory transport for local testing and inter-process communication.

use async_trait::async_trait;
use tokio::sync::{mpsc, Mutex};

use crate::agent_server_protocol::AgentServerMessage;
use crate::connection::Connection;
use crate::error::ConnectionError;

/// In-memory connection for local testing and inter-process communication.
pub struct MemoryConnection {
    rx: Mutex<mpsc::UnboundedReceiver<AgentServerMessage>>,
    tx: mpsc::UnboundedSender<AgentServerMessage>,
}

impl MemoryConnection {
    /// Create a paired connection + handle.
    ///
    /// The `MemoryConnection` side implements `Connection`.
    /// The `MemoryHandle` side is used by tests to send messages and receive output.
    pub fn new() -> (Self, MemoryHandle) {
        let (in_tx, in_rx) = mpsc::unbounded_channel::<AgentServerMessage>();
        let (out_tx, out_rx) = mpsc::unbounded_channel::<AgentServerMessage>();
        (
            Self {
                rx: Mutex::new(in_rx),
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

    async fn recv(&self) -> Option<Result<AgentServerMessage, ConnectionError>> {
        self.rx.lock().await.recv().await.map(Ok)
    }

    async fn send(&self, msg: AgentServerMessage) -> Result<(), ConnectionError> {
        self.tx
            .send(msg)
            .map_err(|e| ConnectionError::ChannelError(e.to_string()))
    }
}

/// Test handle for controlling the memory connection from tests.
///
/// Send inbound messages to the connection, receive outbound messages.
pub struct MemoryHandle {
    tx: mpsc::UnboundedSender<AgentServerMessage>,
    rx: mpsc::UnboundedReceiver<AgentServerMessage>,
}

impl MemoryHandle {
    /// Send an inbound message to the connection.
    pub fn send(&self, msg: AgentServerMessage) -> Result<(), &'static str> {
        self.tx.send(msg).map_err(|_| "connection closed")
    }

    /// Receive the next outbound message.
    pub async fn recv(&mut self) -> Option<AgentServerMessage> {
        self.rx.recv().await
    }
}
