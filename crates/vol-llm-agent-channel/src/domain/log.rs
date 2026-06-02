use async_trait::async_trait;

use crate::agent_server_protocol::{
    AgentServerMessage, LogOperation, LogPayload, Operation, Payload, ProtocolError,
};
use crate::domain::handler::DomainHandler;

/// Handler for log-domain operations.
pub struct LogHandler;

#[async_trait]
impl DomainHandler for LogHandler {
    fn name(&self) -> &str {
        "log"
    }

    fn operations(&self) -> Vec<Operation> {
        vec![
            Operation::Log(LogOperation::List),
            Operation::Log(LogOperation::Read),
        ]
    }

    async fn handle(
        &self,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        let op = match &message.operation {
            Operation::Log(op) => op.clone(),
            _ => return Err(ProtocolError::PayloadDecodeFailed("log")),
        };
        match (op, message.payload) {
            (LogOperation::List, Payload::Log(LogPayload::List)) => Ok(vec![
                AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Log(LogOperation::List),
                    Payload::Log(LogPayload::ListResult { runs: vec![] }),
                ),
            ]),
            (LogOperation::Read, Payload::Log(LogPayload::Read { .. })) => Ok(vec![
                AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Log(LogOperation::Read),
                    Payload::Log(LogPayload::ReadResult { entries: vec![] }),
                ),
            ]),
            (LogOperation::List, _) => Err(ProtocolError::PayloadDecodeFailed("log.list")),
            (LogOperation::Read, _) => Err(ProtocolError::PayloadDecodeFailed("log.read")),
        }
    }
}
