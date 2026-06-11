//! Protocol test: Connection trait uses AgentServerMessage at the boundary.

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use vol_llm_agent_protocol::{
    agent_server_protocol::{AgentOperation, AgentPayload, Payload},
    AgentServerMessage, Connection, ConnectionError, MessageKind, Operation,
};

/// Minimal connection that uses AgentServerMessage for testing.
struct TestConnection {
    rx: Mutex<mpsc::UnboundedReceiver<AgentServerMessage>>,
    tx: mpsc::UnboundedSender<AgentServerMessage>,
}

impl TestConnection {
    fn new() -> (Self, TestHandle) {
        let (_in_tx, in_rx) = mpsc::unbounded_channel::<AgentServerMessage>();
        let (out_tx, out_rx) = mpsc::unbounded_channel::<AgentServerMessage>();
        (
            Self {
                rx: Mutex::new(in_rx),
                tx: out_tx,
            },
            TestHandle { rx: out_rx },
        )
    }
}

#[async_trait]
impl Connection for TestConnection {
    fn protocol(&self) -> &str {
        "test"
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

struct TestHandle {
    rx: mpsc::UnboundedReceiver<AgentServerMessage>,
}

impl TestHandle {
    async fn recv(&mut self) -> Option<AgentServerMessage> {
        self.rx.recv().await
    }
}

fn make_event_message() -> AgentServerMessage {
    AgentServerMessage {
        protocol: "agent-server/1".to_string(),
        message_id: "msg_1".to_string(),
        sender: "agent_a".to_string(),
        receiver: "client".to_string(),
        kind: MessageKind::Event,
        operation: Operation::Agent(AgentOperation::Event),
        payload: Payload::Agent(AgentPayload::Event {
            run_id: "run_42".to_string(),
            event: serde_json::json!({"type": "thought", "content": "thinking"}),
        }),
        meta: Default::default(),
    }
}

/// Test that a Connection using AgentServerMessage can serialize and deserialize
/// an event message correctly across the boundary.
#[tokio::test]
async fn test_connection_boundary_uses_agent_server_message() {
    let (conn, mut handle) = TestConnection::new();
    let conn = Arc::new(conn);

    // Send an event through the connection
    let msg = make_event_message();
    conn.send(msg.clone()).await.unwrap();

    // Verify the handle receives the same AgentServerMessage
    let received = handle.recv().await.unwrap();
    assert_eq!(received.kind, MessageKind::Event);
    assert_eq!(received.operation, Operation::Agent(AgentOperation::Event));
    assert_eq!(received.sender, "agent_a");
    assert_eq!(received.message_id, "msg_1");
}

/// Test that AgentServerMessage with event kind and agent.event operation
/// can be round-tripped through JSON serialization.
#[test]
fn test_event_message_round_trip() {
    let msg = make_event_message();
    let json = serde_json::to_string(&msg).unwrap();
    let decoded: AgentServerMessage = serde_json::from_str(&json).unwrap();

    assert_eq!(decoded.kind, MessageKind::Event);
    assert_eq!(decoded.operation, Operation::Agent(AgentOperation::Event));

    if let Payload::Agent(AgentPayload::Event { run_id, event }) = &decoded.payload {
        assert_eq!(run_id, "run_42");
        assert_eq!(event["type"], "thought");
    } else {
        panic!("expected AgentPayload::Event, got {:?}", decoded.payload);
    }
}
