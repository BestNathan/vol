use crate::agent_server_protocol::{
    AgentServerMessage, Operation, Payload, ProtocolError, SystemOperation, SystemPayload,
};

/// Placeholder handler for system-domain operations.
pub struct SystemHandler;

impl SystemHandler {
    pub async fn handle(
        &self,
        operation: SystemOperation,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        Ok(vec![AgentServerMessage::new_result(
            message.message_id,
            Operation::System(operation),
            Payload::System(SystemPayload::Empty),
        )])
    }
}
