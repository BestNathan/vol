use std::sync::Arc;

use async_trait::async_trait;
use vol_llm_agent_protocol::agent_server_protocol::{
    AgentOperation, AgentPayload, AgentServerMessage, Operation, Payload, ProtocolError,
};
use vol_llm_agent_protocol::DomainHandler;

use crate::control_plane::state::ControlPlaneState;

pub struct ClientHandler {
    state: Arc<ControlPlaneState>,
}

impl ClientHandler {
    pub fn new(state: Arc<ControlPlaneState>) -> Self {
        Self { state }
    }
}

#[async_trait]
impl DomainHandler for ClientHandler {
    fn name(&self) -> &str {
        "control-client"
    }

    fn operations(&self) -> Vec<Operation> {
        vec![
            Operation::Agent(AgentOperation::List),
            Operation::Agent(AgentOperation::Status),
            Operation::Agent(AgentOperation::Submit),
        ]
    }

    async fn handle(
        &self,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        match message.operation.clone() {
            Operation::Agent(AgentOperation::List) => {
                let snapshots = self.state.capabilities.list(None);
                let agents: Vec<serde_json::Value> = snapshots
                    .into_iter()
                    .flat_map(|snapshot| {
                        let node_id = snapshot.node_id;
                        snapshot.agents.into_iter().map(move |agent| {
                            serde_json::json!({
                                "id": agent.agent_id,
                                "name": agent.name,
                                "description": agent.description,
                                "status": agent.status,
                                "node_id": node_id,
                            })
                        })
                    })
                    .collect();
                let payload = Payload::Agent(AgentPayload::ListResult { agents });
                let mut reply = AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Agent(AgentOperation::List),
                    payload,
                );
                reply.sender = "control".to_string();
                reply.receiver = message.sender;
                Ok(vec![reply])
            }
            Operation::Agent(AgentOperation::Status) => {
                let mut reply = AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Agent(AgentOperation::Status),
                    Payload::Agent(AgentPayload::StatusResult {
                        status: "control_plane".to_string(),
                        run_id: None,
                    }),
                );
                reply.sender = "control".to_string();
                reply.receiver = message.sender;
                Ok(vec![reply])
            }
            Operation::Agent(AgentOperation::Submit) => {
                Err(ProtocolError::PayloadDecodeFailedOwned(
                    "agent.submit routing is not implemented in this behavior-completion slice"
                        .to_string(),
                ))
            }
            _ => Err(ProtocolError::PayloadDecodeFailedOwned(
                "unsupported client operation".to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use vol_llm_agent_protocol::agent_server_protocol::{
        AgentOperation, AgentPayload, AgentServerMessage, MessageKind, Operation, Payload,
    };
    use vol_llm_agent_protocol::DomainHandler;

    use crate::control_plane::handlers::client::ClientHandler;
    use crate::control_plane::state::ControlPlaneState;

    #[tokio::test]
    async fn agent_list_returns_empty_list_from_control_plane() {
        let state = Arc::new(ControlPlaneState::new());
        let handler = ClientHandler::new(state);
        let msg = AgentServerMessage {
            protocol: "agent-server/1".to_string(),
            message_id: "1".to_string(),
            sender: "client".to_string(),
            receiver: "control".to_string(),
            kind: MessageKind::Command,
            operation: Operation::Agent(AgentOperation::List),
            payload: Payload::Agent(AgentPayload::ListResult { agents: vec![] }),
            meta: Default::default(),
        };

        let replies = handler.handle(msg).await.unwrap();
        assert_eq!(replies.len(), 1);
        let json = replies[0].payload.data_json();
        assert!(json.get("agents").is_some());
    }

    #[tokio::test]
    async fn agent_status_returns_control_plane_status() {
        let state = Arc::new(ControlPlaneState::new());
        let handler = ClientHandler::new(state);
        let msg = AgentServerMessage {
            protocol: "agent-server/1".to_string(),
            message_id: "1".to_string(),
            sender: "client".to_string(),
            receiver: "control".to_string(),
            kind: MessageKind::Command,
            operation: Operation::Agent(AgentOperation::Status),
            payload: Payload::Agent(AgentPayload::StatusResult {
                status: String::new(),
                run_id: None,
            }),
            meta: Default::default(),
        };

        let replies = handler.handle(msg).await.unwrap();
        assert_eq!(replies.len(), 1);
        let json = replies[0].payload.data_json();
        assert_eq!(json["status"], "control_plane");
    }

    #[tokio::test]
    async fn agent_submit_returns_error() {
        let state = Arc::new(ControlPlaneState::new());
        let handler = ClientHandler::new(state);
        let msg = AgentServerMessage {
            protocol: "agent-server/1".to_string(),
            message_id: "1".to_string(),
            sender: "client".to_string(),
            receiver: "control".to_string(),
            kind: MessageKind::Command,
            operation: Operation::Agent(AgentOperation::Submit),
            payload: Payload::Agent(AgentPayload::Submit {
                input: vol_llm_agent::AgentInput::text("test"),
                target: None,
            }),
            meta: Default::default(),
        };

        let err = handler.handle(msg).await.unwrap_err();
        assert!(err.to_string().contains("agent.submit routing"));
    }

    #[tokio::test]
    async fn agent_list_returns_agents_from_capability_snapshots() {
        use vol_llm_agent_protocol::agent_server_protocol::{
            AgentCapability, CapabilitySnapshot,
        };

        let state = Arc::new(ControlPlaneState::new());
        state
            .capabilities
            .apply_snapshot(CapabilitySnapshot {
                node_id: "node-a".to_string(),
                revision: 1,
                generated_at_ms: Some(1000),
                agents: vec![
                    AgentCapability {
                        agent_id: "coding".to_string(),
                        name: "Coding Agent".to_string(),
                        description: Some("A coding agent".to_string()),
                        status: Some("idle".to_string()),
                    },
                ],
                tools: vec![],
                mcp_servers: vec![],
                skills: vec![],
            })
            .unwrap();

        let handler = ClientHandler::new(state);
        let msg = AgentServerMessage {
            protocol: "agent-server/1".to_string(),
            message_id: "1".to_string(),
            sender: "client".to_string(),
            receiver: "control".to_string(),
            kind: MessageKind::Command,
            operation: Operation::Agent(AgentOperation::List),
            payload: Payload::Agent(AgentPayload::ListResult { agents: vec![] }),
            meta: Default::default(),
        };

        let replies = handler.handle(msg).await.unwrap();
        let json = replies[0].payload.data_json();
        let agents = json["agents"].as_array().unwrap();
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0]["id"], "coding");
        assert_eq!(agents[0]["name"], "Coding Agent");
        assert_eq!(agents[0]["description"], "A coding agent");
        assert_eq!(agents[0]["status"], "idle");
        assert_eq!(agents[0]["node_id"], "node-a");
    }
}
