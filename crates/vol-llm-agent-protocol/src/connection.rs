// crates/vol-llm-agent-channel/src/connection.rs

use async_trait::async_trait;

use crate::agent_server_protocol::AgentServerMessage;
use crate::error::ConnectionError;

/// Abstract connection for agent communication.
/// Implement for each transport protocol.
#[async_trait]
pub trait Connection: Send + Sync + 'static {
    /// Protocol identifier (e.g., "ws", "memory").
    fn protocol(&self) -> &str;

    /// Receive the next incoming message.
    async fn recv(&self) -> Option<Result<AgentServerMessage, ConnectionError>>;

    /// Send a message.
    async fn send(&self, msg: AgentServerMessage) -> Result<(), ConnectionError>;
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use tokio::sync::mpsc;

    use super::*;
    use crate::agent_server_protocol::{AgentOperation, AgentPayload, MessageKind, Operation, Payload};

    struct TestConnection {
        protocol: &'static str,
        rx: tokio::sync::Mutex<mpsc::UnboundedReceiver<AgentServerMessage>>,
        tx: mpsc::UnboundedSender<AgentServerMessage>,
    }

    #[async_trait]
    impl Connection for TestConnection {
        fn protocol(&self) -> &str {
            self.protocol
        }
        async fn recv(&self) -> Option<Result<AgentServerMessage, ConnectionError>> {
            self.rx.lock().await.recv().await.map(Ok)
        }
        async fn send(&self, msg: AgentServerMessage) -> Result<(), ConnectionError> {
            self.tx.send(msg).map_err(|e| ConnectionError::ChannelError(e.to_string()))
        }
    }

    fn make_msg() -> AgentServerMessage {
        AgentServerMessage::new_command(
            "msg-1",
            Operation::Agent(AgentOperation::Submit),
            Payload::Agent(AgentPayload::Submit {
                input: vol_llm_agent::AgentInput::text("hello"),
                target: None,
            }),
        )
    }

    #[tokio::test]
    async fn test_connection_send_and_recv() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let (_in_tx, in_rx) = mpsc::unbounded_channel();
        let conn = TestConnection {
            protocol: "test",
            rx: tokio::sync::Mutex::new(in_rx),
            tx,
        };

        let msg = make_msg();
        conn.send(msg.clone()).await.unwrap();
        let received = rx.recv().await.unwrap();
        assert_eq!(received.message_id, "msg-1");
        assert_eq!(received.kind, MessageKind::Command);
    }

    #[test]
    fn test_connection_protocol() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let (_in_tx, in_rx) = mpsc::unbounded_channel();
        let conn = TestConnection {
            protocol: "custom",
            rx: tokio::sync::Mutex::new(in_rx),
            tx,
        };
        assert_eq!(conn.protocol(), "custom");
    }
}
