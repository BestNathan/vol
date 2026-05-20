use std::sync::Arc;

use vol_session::SessionEntryStore;

use crate::agent_server_protocol::{
    AgentServerMessage, Operation, Payload, ProtocolError, SessionOperation, SessionPayload,
};

/// Handler for session-domain operations.
pub struct SessionHandler {
    session_store: Arc<vol_session::file_store::FileSessionEntryStore>,
}

impl SessionHandler {
    pub fn new(session_store: Arc<vol_session::file_store::FileSessionEntryStore>) -> Self {
        Self { session_store }
    }

    pub async fn handle(
        &self,
        operation: SessionOperation,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        match (operation, message.payload) {
            (SessionOperation::List, Payload::Session(SessionPayload::List)) => {
                match self.session_store.list_sessions() {
                    Ok(summaries) => {
                        let sessions: Vec<serde_json::Value> = summaries
                            .into_iter()
                            .map(|s| serde_json::json!({
                                "id": s.session_id,
                                "entry_count": s.entry_count,
                                "created_at": s.created_at,
                            }))
                            .collect();
                        Ok(vec![AgentServerMessage::new_result(
                            message.message_id,
                            Operation::Session(SessionOperation::List),
                            Payload::Session(SessionPayload::ListResult { sessions }),
                        )])
                    }
                    Err(e) => Ok(vec![AgentServerMessage::new_error(
                        message.message_id,
                        Operation::Session(SessionOperation::List),
                        crate::agent_server_protocol::ErrorPayload {
                            code: "session_list_failed".to_string(),
                            message: format!("Failed to list sessions: {e}"),
                            detail: None,
                            terminal: true,
                        },
                    )]),
                }
            }
            (SessionOperation::Resume, Payload::Session(SessionPayload::Resume { session_id })) => {
                match self.session_store.get_entries(&session_id).await {
                    Ok(entries) => {
                        let json_entries: Vec<serde_json::Value> = entries
                            .into_iter()
                            .filter_map(|e| serde_json::to_value(e).ok())
                            .collect();
                        Ok(vec![AgentServerMessage::new_result(
                            message.message_id,
                            Operation::Session(SessionOperation::Resume),
                            Payload::Session(SessionPayload::ResumeResult {
                                session_id,
                                restored: true,
                                entries: json_entries,
                            }),
                        )])
                    }
                    Err(e) => Ok(vec![AgentServerMessage::new_error(
                        message.message_id,
                        Operation::Session(SessionOperation::Resume),
                        crate::agent_server_protocol::ErrorPayload {
                            code: "session_resume_failed".to_string(),
                            message: format!("Failed to resume session: {e}"),
                            detail: None,
                            terminal: true,
                        },
                    )]),
                }
            }
            (SessionOperation::Entries, Payload::Session(SessionPayload::Entries { session_id })) => {
                match self.session_store.get_entries(&session_id).await {
                    Ok(entries) => {
                        let json_entries: Vec<serde_json::Value> = entries
                            .into_iter()
                            .filter_map(|e| serde_json::to_value(e).ok())
                            .collect();
                        Ok(vec![AgentServerMessage::new_result(
                            message.message_id,
                            Operation::Session(SessionOperation::Entries),
                            Payload::Session(SessionPayload::EntriesResult { entries: json_entries }),
                        )])
                    }
                    Err(e) => Ok(vec![AgentServerMessage::new_error(
                        message.message_id,
                        Operation::Session(SessionOperation::Entries),
                        crate::agent_server_protocol::ErrorPayload {
                            code: "session_entries_failed".to_string(),
                            message: format!("Failed to get entries: {e}"),
                            detail: None,
                            terminal: true,
                        },
                    )]),
                }
            }
            (SessionOperation::List, _) => Err(ProtocolError::PayloadDecodeFailed("session.list")),
            (SessionOperation::Resume, _) => Err(ProtocolError::PayloadDecodeFailed("session.resume")),
            (SessionOperation::Entries, _) => Err(ProtocolError::PayloadDecodeFailed("session.entries")),
        }
    }
}
