use async_trait::async_trait;

use vol_llm_agent_protocol::agent_server_protocol::{
    AgentServerMessage, Operation, Payload, ProtocolError, SystemOperation, SystemPayload,
};
use vol_llm_agent_protocol::DomainHandler;

/// Placeholder handler for system-domain operations.
pub struct SystemHandler;

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
        Ok(vec![AgentServerMessage::new_result(
            message.message_id,
            Operation::System(op),
            Payload::System(SystemPayload::Empty),
        )])
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
            receiver: "data-plane".to_string(),
            kind: MessageKind::Command,
            operation: op,
            payload,
            meta: Default::default(),
        }
    }

    #[tokio::test]
    async fn system_connected_returns_empty_payload() {
        let handler = SystemHandler;
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
        // Empty variant serializes as a string "Empty"
        assert_eq!(json, serde_json::Value::String("Empty".to_string()));
    }

    #[tokio::test]
    async fn system_handler_rejects_non_system_operation() {
        let handler = SystemHandler;
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
