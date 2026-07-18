use std::sync::Arc;

use async_trait::async_trait;
use vol_llm_agent_protocol::agent_server_protocol::{
    AgentServerMessage, ControlOperation, ControlPayload, Operation, Payload, ProtocolError,
};
use vol_llm_agent_protocol::DomainHandler;

use crate::control_plane::core::make_result;
use crate::control_plane::event::ControlPlaneEvent;
use crate::control_plane::state::ControlPlaneState;

pub struct ControlHandler {
    state: Arc<ControlPlaneState>,
}

impl ControlHandler {
    pub fn new(state: Arc<ControlPlaneState>) -> Self {
        Self { state }
    }
}

#[async_trait]
impl DomainHandler for ControlHandler {
    fn name(&self) -> &str {
        "control"
    }

    fn operations(&self) -> Vec<Operation> {
        vec![
            Operation::Control(ControlOperation::Register),
            Operation::Control(ControlOperation::Heartbeat),
            Operation::Control(ControlOperation::CapabilitySnapshot),
            Operation::Control(ControlOperation::CapabilityDelta),
            Operation::Control(ControlOperation::Event),
            Operation::Control(ControlOperation::CommandResult),
        ]
    }

    async fn handle(
        &self,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        match (message.operation.clone(), message.payload.clone()) {
            (
                Operation::Control(ControlOperation::Register),
                Payload::Control(ControlPayload::Register(reg)),
            ) => {
                // Clear old capability snapshot on re-registration so the
                // new snapshot (which starts at revision 1) is accepted.
                self.state.capabilities.remove_node(&reg.node_id);
                let ack = self
                    .state
                    .nodes
                    .register(reg, message.sender.clone(), now_ms())
                    .map_err(ProtocolError::PayloadDecodeFailedOwned)?;

                Ok(vec![make_result(
                    message,
                    ControlOperation::Register,
                    ControlPayload::RegisterAck(ack),
                )])
            }
            (
                Operation::Control(ControlOperation::Heartbeat),
                Payload::Control(ControlPayload::Heartbeat(hb)),
            ) => {
                let node_id = hb.node_id.clone();
                self.state
                    .nodes
                    .heartbeat(&hb.node_id, hb.load, now_ms())
                    .map_err(ProtocolError::PayloadDecodeFailedOwned)?;
                let ack = vol_llm_agent_protocol::agent_server_protocol::HeartbeatAck { node_id };
                Ok(vec![make_result(
                    message,
                    ControlOperation::Heartbeat,
                    ControlPayload::HeartbeatAck(ack),
                )])
            }
            (
                Operation::Control(ControlOperation::CapabilitySnapshot),
                Payload::Control(ControlPayload::CapabilitySnapshot(snapshot)),
            ) => {
                let node_id = snapshot.node_id.clone();
                let revision = snapshot.revision;
                self.state
                    .capabilities
                    .apply_snapshot(snapshot)
                    .map_err(ProtocolError::PayloadDecodeFailedOwned)?;
                self.state
                    .nodes
                    .update_capability_revision(&node_id, revision)
                    .map_err(ProtocolError::PayloadDecodeFailedOwned)?;
                let ack = vol_llm_agent_protocol::agent_server_protocol::CapabilitySnapshotAck {
                    node_id,
                    revision,
                };
                Ok(vec![make_result(
                    message,
                    ControlOperation::CapabilitySnapshot,
                    ControlPayload::CapabilitySnapshotAck(ack),
                )])
            }
            (
                Operation::Control(ControlOperation::Event),
                Payload::Control(ControlPayload::Event(event)),
            ) => {
                self.state.events.publish(ControlPlaneEvent {
                    event_type: event.event_type,
                    node_id: Some(event.node_id),
                });
                Ok(vec![])
            }
            (
                Operation::Control(ControlOperation::CapabilityDelta),
                Payload::Control(ControlPayload::CapabilityDelta(_)),
            )
            | (
                Operation::Control(ControlOperation::CommandResult),
                Payload::Control(ControlPayload::CommandResult(_)),
            ) => Ok(vec![]),
            _ => Err(ProtocolError::PayloadDecodeFailedOwned(
                "unsupported control operation".to_string(),
            )),
        }
    }
}

#[allow(clippy::cast_possible_truncation)]
fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use vol_llm_agent_protocol::agent_server_protocol::{
        AgentServerMessage, ControlOperation, ControlPayload, MessageKind, NodeRegistration,
        Operation, Payload,
    };
    use vol_llm_agent_protocol::DomainHandler;

    use super::ControlHandler;
    use crate::control_plane::state::ControlPlaneState;
    use vol_llm_agent_protocol::agent_server_protocol::CapabilitySnapshot;

    #[tokio::test]
    async fn control_register_creates_node() {
        let state = Arc::new(ControlPlaneState::new());
        let handler = ControlHandler::new(state.clone());
        let msg = AgentServerMessage {
            protocol: "agent-server/1".to_string(),
            message_id: "1".to_string(),
            sender: "node-a".to_string(),
            receiver: "control".to_string(),
            kind: MessageKind::Command,
            operation: Operation::Control(ControlOperation::Register),
            payload: Payload::Control(ControlPayload::Register(NodeRegistration {
                node_id: "node-a".to_string(),
                name: "Node A".to_string(),
                version: "0.1.0".to_string(),
            })),
            meta: Default::default(),
        };

        let replies = handler.handle(msg).await.unwrap();
        assert_eq!(replies.len(), 1);
        assert!(state.nodes.get("node-a").is_some());
    }

    #[tokio::test]
    async fn capability_snapshot_updates_node_capability_revision() {
        let state = Arc::new(ControlPlaneState::new());
        state
            .nodes
            .register(
                NodeRegistration {
                    node_id: "node-a".to_string(),
                    name: "Node A".to_string(),
                    version: "0.1.0".to_string(),
                },
                "node-a".to_string(),
                1000,
            )
            .unwrap();

        let handler = ControlHandler::new(state.clone());
        let msg = AgentServerMessage {
            protocol: "agent-server/1".to_string(),
            message_id: "snap-1".to_string(),
            sender: "node-a".to_string(),
            receiver: "control".to_string(),
            kind: MessageKind::Command,
            operation: Operation::Control(ControlOperation::CapabilitySnapshot),
            payload: Payload::Control(ControlPayload::CapabilitySnapshot(CapabilitySnapshot {
                node_id: "node-a".to_string(),
                revision: 7,
                generated_at_ms: Some(1000),
                agents: vec![],
                tools: vec![],
                mcp_servers: vec![],
                skills: vec![],
            })),
            meta: Default::default(),
        };

        handler.handle(msg).await.unwrap();
        let node = state.nodes.get("node-a").unwrap();
        assert_eq!(node.capability_revision, 7);
    }

    #[tokio::test]
    async fn capability_delta_returns_ok() {
        let state = Arc::new(ControlPlaneState::new());
        let handler = ControlHandler::new(state);
        let msg = AgentServerMessage {
            protocol: "agent-server/1".to_string(),
            message_id: "delta-1".to_string(),
            sender: "node-a".to_string(),
            receiver: "control".to_string(),
            kind: MessageKind::Command,
            operation: Operation::Control(ControlOperation::CapabilityDelta),
            payload: Payload::Control(ControlPayload::CapabilityDelta(
                vol_llm_agent_protocol::agent_server_protocol::CapabilityDelta {
                    node_id: "node-a".to_string(),
                    base_revision: 1,
                    revision: 2,
                },
            )),
            meta: Default::default(),
        };
        let replies = handler.handle(msg).await.unwrap();
        assert!(replies.is_empty());
    }

    #[tokio::test]
    async fn command_result_returns_ok() {
        let state = Arc::new(ControlPlaneState::new());
        let handler = ControlHandler::new(state);
        let msg = AgentServerMessage {
            protocol: "agent-server/1".to_string(),
            message_id: "cr-1".to_string(),
            sender: "node-a".to_string(),
            receiver: "control".to_string(),
            kind: MessageKind::Command,
            operation: Operation::Control(ControlOperation::CommandResult),
            payload: Payload::Control(ControlPayload::CommandResult(
                vol_llm_agent_protocol::agent_server_protocol::CommandResult {
                    command_id: "cmd-1".to_string(),
                    status: "completed".to_string(),
                    result: serde_json::json!({}),
                    error: None,
                },
            )),
            meta: Default::default(),
        };
        let replies = handler.handle(msg).await.unwrap();
        assert!(replies.is_empty());
    }

    #[tokio::test]
    async fn event_publishes_through_bus() {
        let state = Arc::new(ControlPlaneState::new());
        let mut rx = state.events.subscribe();
        let handler = ControlHandler::new(state);
        let msg = AgentServerMessage {
            protocol: "agent-server/1".to_string(),
            message_id: "evt-1".to_string(),
            sender: "node-a".to_string(),
            receiver: "control".to_string(),
            kind: MessageKind::Command,
            operation: Operation::Control(ControlOperation::Event),
            payload: Payload::Control(ControlPayload::Event(
                vol_llm_agent_protocol::agent_server_protocol::DataPlaneEvent {
                    node_id: "node-a".to_string(),
                    event_type: "node_connected".to_string(),
                    data: serde_json::json!({}),
                },
            )),
            meta: Default::default(),
        };
        let replies = handler.handle(msg).await.unwrap();
        assert!(replies.is_empty());
        let event = rx.recv().await.unwrap();
        assert_eq!(event.event_type, "node_connected");
    }

    #[tokio::test]
    async fn control_returns_error_on_unknown_operation() {
        let state = Arc::new(ControlPlaneState::new());
        let handler = ControlHandler::new(state);
        let msg = AgentServerMessage {
            protocol: "agent-server/1".to_string(),
            message_id: "1".to_string(),
            sender: "client".to_string(),
            receiver: "control".to_string(),
            kind: MessageKind::Command,
            operation: Operation::Control(ControlOperation::NodeList),
            payload: Payload::Control(ControlPayload::NodeList(
                vol_llm_agent_protocol::agent_server_protocol::NodeListRequest {},
            )),
            meta: Default::default(),
        };
        let err = handler.handle(msg).await.unwrap_err();
        assert!(err.to_string().contains("unsupported control operation"));
    }
}
