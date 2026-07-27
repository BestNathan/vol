use async_trait::async_trait;
use vol_llm_agent_protocol::agent_server_protocol::{
    AgentServerMessage, ConnectedInfo, Operation, Payload, ProtocolError, ServerType,
    SystemOperation, SystemPayload,
};
use vol_llm_agent_protocol::DomainHandler;

/// Handler for system-domain operations on the control plane.
pub struct SystemHandler;

impl SystemHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SystemHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DomainHandler for SystemHandler {
    fn name(&self) -> &str {
        "system"
    }

    fn operations(&self) -> Vec<Operation> {
        vec![Operation::System(SystemOperation::Connected)]
    }

    async fn handle(
        &self,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        let op = match &message.operation {
            Operation::System(op) => op.clone(),
            _ => return Err(ProtocolError::PayloadDecodeFailed("system")),
        };

        match op {
            SystemOperation::Connected => {
                let info = ConnectedInfo {
                    server_type: ServerType::ControlPlane,
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    capabilities: vec![
                        "control.node_list".to_string(),
                        "control.node_get".to_string(),
                        "control.capability_list".to_string(),
                    ],
                };
                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::System(SystemOperation::Connected),
                    Payload::System(SystemPayload::Connected(info)),
                )])
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use vol_llm_agent_protocol::agent_server_protocol::{
        AgentServerMessage, MessageKind, Operation, Payload, SystemOperation, SystemPayload,
    };
    use vol_llm_agent_protocol::DomainHandler;

    use super::SystemHandler;

    fn msg(id: &str, op: Operation, payload: Payload) -> AgentServerMessage {
        AgentServerMessage {
            protocol: "agent-server/1".to_string(),
            message_id: id.to_string(),
            sender: "client".to_string(),
            receiver: "control-plane".to_string(),
            kind: MessageKind::Command,
            operation: op,
            payload,
            meta: Default::default(),
        }
    }

    #[tokio::test]
    async fn system_connected_returns_control_plane_info() {
        let handler = SystemHandler::new();
        let replies = handler
            .handle(msg(
                "1",
                Operation::System(SystemOperation::Connected),
                Payload::System(SystemPayload::Empty),
            ))
            .await
            .unwrap();
        assert_eq!(replies.len(), 1);
        let json = replies[0].payload.data_json();
        // data_json() strips the variant wrapper, so we get ConnectedInfo directly
        let info: vol_llm_agent_protocol::agent_server_protocol::ConnectedInfo =
            serde_json::from_value(json).unwrap();
        assert_eq!(
            info.server_type,
            vol_llm_agent_protocol::agent_server_protocol::ServerType::ControlPlane
        );
        assert!(!info.version.is_empty());
        assert!(info.capabilities.contains(&"control.node_list".to_string()));
    }

    #[tokio::test]
    async fn system_handler_rejects_non_system_operation() {
        let handler = SystemHandler::new();
        let err = handler
            .handle(msg(
                "1",
                Operation::Log(vol_llm_agent_protocol::agent_server_protocol::LogOperation::List),
                Payload::Log(vol_llm_agent_protocol::agent_server_protocol::LogPayload::List),
            ))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("system"));
    }
}
