use async_trait::async_trait;

use crate::agent_server_protocol::{
    AgentServerMessage, Operation, Payload, ProtocolError, SystemOperation, SystemPayload,
};
use crate::domain::handler::DomainHandler;

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
