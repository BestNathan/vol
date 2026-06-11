use async_trait::async_trait;

use vol_llm_agent_protocol::agent_server_protocol::{
    AgentServerMessage, LogOperation, LogPayload, Operation, Payload, ProtocolError,
};
use vol_llm_agent_protocol::DomainHandler;

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
            (LogOperation::List, Payload::Log(LogPayload::List)) => {
                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Log(LogOperation::List),
                    Payload::Log(LogPayload::ListResult { runs: vec![] }),
                )])
            }
            (LogOperation::Read, Payload::Log(LogPayload::Read { .. })) => {
                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Log(LogOperation::Read),
                    Payload::Log(LogPayload::ReadResult { entries: vec![] }),
                )])
            }
            (LogOperation::List, _) => Err(ProtocolError::PayloadDecodeFailed("log.list")),
            (LogOperation::Read, _) => Err(ProtocolError::PayloadDecodeFailed("log.read")),
        }
    }
}

#[cfg(test)]
mod tests {
    use vol_llm_agent_protocol::agent_server_protocol::{
        AgentServerMessage, LogOperation, LogPayload, MessageKind, Operation, Payload,
    };
    use vol_llm_agent_protocol::DomainHandler;

    use super::LogHandler;

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
    async fn log_list_returns_empty_runs() {
        let handler = LogHandler;
        let replies = handler
            .handle(msg(
                "1",
                Operation::Log(LogOperation::List),
                Payload::Log(LogPayload::List),
            ))
            .await
            .unwrap();
        let json = replies[0].payload.data_json();
        let runs = json["runs"].as_array().unwrap();
        assert!(runs.is_empty());
    }

    #[tokio::test]
    async fn log_read_returns_empty_entries() {
        let handler = LogHandler;
        let replies = handler
            .handle(msg(
                "1",
                Operation::Log(LogOperation::Read),
                Payload::Log(LogPayload::Read {
                    run_id: "run-1".to_string(),
                }),
            ))
            .await
            .unwrap();
        let json = replies[0].payload.data_json();
        let entries = json["entries"].as_array().unwrap();
        assert!(entries.is_empty());
    }

    #[tokio::test]
    async fn log_list_with_wrong_payload_returns_error() {
        let handler = LogHandler;
        let err = handler
            .handle(msg(
                "1",
                Operation::Log(LogOperation::List),
                Payload::Log(LogPayload::Read {
                    run_id: "run-1".to_string(),
                }),
            ))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("log.list"));
    }

    #[tokio::test]
    async fn log_read_with_wrong_payload_returns_error() {
        let handler = LogHandler;
        let err = handler
            .handle(msg(
                "1",
                Operation::Log(LogOperation::Read),
                Payload::Log(LogPayload::List),
            ))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("log.read"));
    }
}
