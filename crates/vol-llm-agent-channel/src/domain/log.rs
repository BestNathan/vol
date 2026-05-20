use crate::agent_server_protocol::{
    AgentServerMessage, LogOperation, LogPayload, Operation, Payload, ProtocolError,
};

/// Handler for log-domain operations.
pub struct LogHandler;

impl LogHandler {
    pub async fn handle(
        &self,
        operation: LogOperation,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        match (operation, message.payload) {
            (LogOperation::List, Payload::Log(LogPayload::List)) => Ok(vec![AgentServerMessage::new_result(
                message.message_id,
                Operation::Log(LogOperation::List),
                Payload::Log(LogPayload::ListResult { runs: vec![] }),
            )]),
            (LogOperation::Read, Payload::Log(LogPayload::Read { .. })) => Ok(vec![AgentServerMessage::new_result(
                message.message_id,
                Operation::Log(LogOperation::Read),
                Payload::Log(LogPayload::ReadResult { entries: vec![] }),
            )]),
            (LogOperation::List, _) => Err(ProtocolError::PayloadDecodeFailed("log.list")),
            (LogOperation::Read, _) => Err(ProtocolError::PayloadDecodeFailed("log.read")),
        }
    }
}
