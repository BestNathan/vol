use std::sync::Arc;

use async_trait::async_trait;
use vol_llm_agent_protocol::agent_server_protocol::{
    AgentServerMessage, CapabilityListResult, ControlOperation, ControlPayload, Operation, Payload,
    ProtocolError,
};
use vol_llm_agent_protocol::DomainHandler;

use crate::control_plane::core::make_result;
use crate::control_plane::state::ControlPlaneState;

pub struct CapabilityHandler {
    state: Arc<ControlPlaneState>,
}

impl CapabilityHandler {
    pub fn new(state: Arc<ControlPlaneState>) -> Self {
        Self { state }
    }
}

#[async_trait]
impl DomainHandler for CapabilityHandler {
    fn name(&self) -> &str {
        "capability"
    }

    fn operations(&self) -> Vec<Operation> {
        vec![Operation::Control(ControlOperation::CapabilityList)]
    }

    async fn handle(
        &self,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        match (message.operation.clone(), message.payload.clone()) {
            (
                Operation::Control(ControlOperation::CapabilityList),
                Payload::Control(ControlPayload::CapabilityList(req)),
            ) => {
                let result = CapabilityListResult {
                    snapshots: self.state.capabilities.list(req.node_id.as_deref()),
                };

                Ok(vec![make_result(
                    message,
                    ControlOperation::CapabilityList,
                    ControlPayload::CapabilityListResult(result),
                )])
            }
            _ => Err(ProtocolError::PayloadDecodeFailedOwned(
                "unsupported capability operation".to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use vol_llm_agent_protocol::agent_server_protocol::{
        AgentServerMessage, ControlOperation, ControlPayload, MessageKind, Operation, Payload,
    };
    use vol_llm_agent_protocol::DomainHandler;

    use crate::control_plane::handlers::capability::CapabilityHandler;
    use crate::control_plane::state::ControlPlaneState;
    use vol_llm_agent_protocol::agent_server_protocol::{AgentCapability, CapabilitySnapshot};

    fn msg(id: &str, op: Operation, payload: Payload) -> AgentServerMessage {
        AgentServerMessage {
            protocol: "agent-server/1".to_string(),
            message_id: id.to_string(),
            sender: "client".to_string(),
            receiver: "control".to_string(),
            kind: MessageKind::Command,
            operation: op,
            payload,
            meta: Default::default(),
        }
    }

    fn make_snapshot(node_id: &str, revision: u64) -> CapabilitySnapshot {
        CapabilitySnapshot {
            node_id: node_id.to_string(),
            revision,
            generated_at_ms: Some(1000 + revision),
            agents: vec![AgentCapability {
                agent_id: format!("{node_id}-agent"),
                name: format!("{node_id}-agent"),
                description: None,
                status: Some("idle".to_string()),
            }],
            tools: vec![],
            mcp_servers: vec![],
            skills: vec![],
        }
    }

    #[tokio::test]
    async fn capability_list_returns_all_snapshots_without_node_id() {
        let state = Arc::new(ControlPlaneState::new());
        state
            .capabilities
            .apply_snapshot(make_snapshot("node-a", 1))
            .unwrap();
        state
            .capabilities
            .apply_snapshot(make_snapshot("node-b", 1))
            .unwrap();

        let handler = CapabilityHandler::new(state);
        let replies = handler
            .handle(msg(
                "1",
                Operation::Control(ControlOperation::CapabilityList),
                Payload::Control(ControlPayload::CapabilityList(
                    vol_llm_agent_protocol::agent_server_protocol::CapabilityListRequest {
                        node_id: None,
                    },
                )),
            ))
            .await
            .unwrap();

        let json = replies[0].payload.data_json();
        let snapshots = json["snapshots"].as_array().unwrap();
        assert_eq!(snapshots.len(), 2);
    }

    #[tokio::test]
    async fn capability_list_filters_by_node_id() {
        let state = Arc::new(ControlPlaneState::new());
        state
            .capabilities
            .apply_snapshot(make_snapshot("node-a", 1))
            .unwrap();
        state
            .capabilities
            .apply_snapshot(make_snapshot("node-b", 1))
            .unwrap();

        let handler = CapabilityHandler::new(state);
        let replies = handler
            .handle(msg(
                "1",
                Operation::Control(ControlOperation::CapabilityList),
                Payload::Control(ControlPayload::CapabilityList(
                    vol_llm_agent_protocol::agent_server_protocol::CapabilityListRequest {
                        node_id: Some("node-a".to_string()),
                    },
                )),
            ))
            .await
            .unwrap();

        let json = replies[0].payload.data_json();
        let snapshots = json["snapshots"].as_array().unwrap();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0]["node_id"], "node-a");
    }

    #[tokio::test]
    async fn capability_list_returns_empty_when_no_snapshots() {
        let handler = CapabilityHandler::new(Arc::new(ControlPlaneState::new()));
        let replies = handler
            .handle(msg(
                "1",
                Operation::Control(ControlOperation::CapabilityList),
                Payload::Control(ControlPayload::CapabilityList(
                    vol_llm_agent_protocol::agent_server_protocol::CapabilityListRequest {
                        node_id: None,
                    },
                )),
            ))
            .await
            .unwrap();

        let json = replies[0].payload.data_json();
        let snapshots = json["snapshots"].as_array().unwrap();
        assert!(snapshots.is_empty());
    }

    #[tokio::test]
    async fn capability_handler_returns_error_on_unknown_operation() {
        let handler = CapabilityHandler::new(Arc::new(ControlPlaneState::new()));
        let err = handler
            .handle(msg(
                "1",
                Operation::Control(ControlOperation::Register),
                Payload::Control(ControlPayload::Register(
                    vol_llm_agent_protocol::agent_server_protocol::NodeRegistration {
                        node_id: "x".to_string(),
                        name: "x".to_string(),
                        version: "0".to_string(),
                    },
                )),
            ))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("unsupported capability operation"));
    }
}
