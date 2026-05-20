use crate::agent_server_protocol::{AgentServerMessage, FileOperation, FilePayload, Operation, Payload, ProtocolError};

/// Handler for file-domain operations.
pub struct FileHandler;

impl FileHandler {
    pub async fn handle(
        &self,
        operation: FileOperation,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        match (operation, message.payload) {
            (FileOperation::List, Payload::File(FilePayload::List { path })) => {
                match std::fs::read_dir(&path) {
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
                            b_dir.cmp(&a_dir).then_with(|| a["name"].as_str().cmp(&b["name"].as_str()))
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
                        crate::agent_server_protocol::ErrorPayload {
                            code: "file_list_failed".to_string(),
                            message: format!("Failed to read directory: {e}"),
                            detail: None,
                            terminal: true,
                        },
                    )]),
                }
            }
            (FileOperation::Read, Payload::File(FilePayload::Read { path })) => {
                match std::fs::read_to_string(&path) {
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
                        crate::agent_server_protocol::ErrorPayload {
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
