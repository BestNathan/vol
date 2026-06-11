use std::path::PathBuf;

use async_trait::async_trait;

use vol_llm_agent_protocol::agent_server_protocol::{
    AgentServerMessage, FileOperation, FilePayload, Operation, Payload, ProtocolError,
};
use vol_llm_agent_protocol::DomainHandler;

/// Handler for file-domain operations.
pub struct FileHandler {
    working_dir: PathBuf,
}

impl FileHandler {
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }

    fn resolve_path(&self, path: &str) -> PathBuf {
        let p = PathBuf::from(path);
        if p.is_absolute() {
            p
        } else {
            self.working_dir.join(p)
        }
    }
}

#[async_trait]
impl DomainHandler for FileHandler {
    fn name(&self) -> &str {
        "file"
    }

    fn operations(&self) -> Vec<Operation> {
        vec![
            Operation::File(FileOperation::List),
            Operation::File(FileOperation::Read),
        ]
    }

    async fn handle(
        &self,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        let op = match &message.operation {
            Operation::File(op) => op.clone(),
            _ => return Err(ProtocolError::PayloadDecodeFailed("file")),
        };
        match (op, message.payload) {
            (FileOperation::List, Payload::File(FilePayload::List { path })) => {
                let resolved = self.resolve_path(&path);
                match std::fs::read_dir(&resolved) {
                    Ok(entries) => {
                        let mut list: Vec<serde_json::Value> = Vec::new();
                        for entry in entries.flatten() {
                            let name = entry.file_name().to_string_lossy().to_string();
                            let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
                            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                            list.push(serde_json::json!({
                                "name": name,
                                "is_dir": is_dir,
                                "size": size,
                            }));
                        }
                        list.sort_by(|a, b| {
                            let a_dir = a["is_dir"].as_bool().unwrap_or(false);
                            let b_dir = b["is_dir"].as_bool().unwrap_or(false);
                            b_dir
                                .cmp(&a_dir)
                                .then_with(|| a["name"].as_str().cmp(&b["name"].as_str()))
                        });
                        Ok(vec![AgentServerMessage::new_result(
                            message.message_id,
                            Operation::File(FileOperation::List),
                            Payload::File(FilePayload::ListResult { entries: list }),
                        )])
                    }
                    Err(e) => Ok(vec![AgentServerMessage::new_error(
                        message.message_id,
                        Operation::File(FileOperation::List),
                        vol_llm_agent_protocol::agent_server_protocol::ErrorPayload {
                            code: "file_list_failed".to_string(),
                            message: format!("Failed to read directory: {e}"),
                            detail: None,
                            terminal: true,
                        },
                    )]),
                }
            }
            (FileOperation::Read, Payload::File(FilePayload::Read { path })) => {
                let resolved = self.resolve_path(&path);
                match std::fs::read_to_string(&resolved) {
                    Ok(content) => Ok(vec![AgentServerMessage::new_result(
                        message.message_id,
                        Operation::File(FileOperation::Read),
                        Payload::File(FilePayload::ReadResult {
                            content,
                            metadata: serde_json::json!({}),
                        }),
                    )]),
                    Err(e) => Ok(vec![AgentServerMessage::new_error(
                        message.message_id,
                        Operation::File(FileOperation::Read),
                        vol_llm_agent_protocol::agent_server_protocol::ErrorPayload {
                            code: "file_read_failed".to_string(),
                            message: format!("Failed to read file: {e}"),
                            detail: None,
                            terminal: true,
                        },
                    )]),
                }
            }
            (FileOperation::List, _) => Err(ProtocolError::PayloadDecodeFailed("file.list")),
            (FileOperation::Read, _) => Err(ProtocolError::PayloadDecodeFailed("file.read")),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use vol_llm_agent_protocol::agent_server_protocol::{
        AgentServerMessage, FileOperation, FilePayload, MessageKind, Operation, Payload,
    };
    use vol_llm_agent_protocol::DomainHandler;

    use super::FileHandler;

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
    async fn file_list_nonexistent_dir_returns_error() {
        let handler = FileHandler::new(PathBuf::from("/tmp"));
        let replies = handler
            .handle(msg(
                "1",
                Operation::File(FileOperation::List),
                Payload::File(FilePayload::List {
                    path: "/nonexistent_dir_12345".to_string(),
                }),
            ))
            .await
            .unwrap();
        let json = replies[0].payload.data_json();
        assert_eq!(json["code"], "file_list_failed");
    }

    #[tokio::test]
    async fn file_read_nonexistent_file_returns_error() {
        let handler = FileHandler::new(PathBuf::from("/tmp"));
        let replies = handler
            .handle(msg(
                "1",
                Operation::File(FileOperation::Read),
                Payload::File(FilePayload::Read {
                    path: "/nonexistent_file_12345.txt".to_string(),
                }),
            ))
            .await
            .unwrap();
        let json = replies[0].payload.data_json();
        assert_eq!(json["code"], "file_read_failed");
    }

    #[tokio::test]
    async fn file_rejects_non_file_operation() {
        let handler = FileHandler::new(PathBuf::from("/tmp"));
        let err = handler
            .handle(msg(
                "1",
                Operation::Log(vol_llm_agent_protocol::agent_server_protocol::LogOperation::List),
                Payload::Log(vol_llm_agent_protocol::agent_server_protocol::LogPayload::List),
            ))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("file"));
    }
}
