use std::path::PathBuf;

use async_trait::async_trait;
use vol_session::file_store::FileSessionEntryStore;
use vol_session::SessionEntryStore;

use crate::agent_server_protocol::{
    AgentServerMessage, Operation, Payload, ProtocolError, SessionOperation, SessionPayload,
};
use crate::domain::handler::DomainHandler;

/// Handler for session-domain operations.
///
/// Scans all agent directories under agents_root to aggregate sessions.
pub struct SessionHandler {
    agents_root: PathBuf,
}

impl SessionHandler {
    pub fn new(agents_root: PathBuf) -> Self {
        Self { agents_root }
    }

    /// Get a session store for a specific agent.
    fn agent_store(&self, agent_id: &str) -> FileSessionEntryStore {
        FileSessionEntryStore::new(self.agents_root.join(agent_id).join("sessions"))
    }

    /// Find which agent owns a session by scanning all agent dirs.
    fn find_store_for_session(&self, session_id: &str) -> Result<FileSessionEntryStore, ProtocolError> {
        if self.agents_root.is_dir() {
            for entry in std::fs::read_dir(&self.agents_root).into_iter().flatten().flatten() {
                if entry.path().is_dir() {
                    if let Some(agent_id) = entry.file_name().to_str() {
                        let store = self.agent_store(agent_id);
                        if let Ok(summaries) = store.list_sessions() {
                            if summaries.iter().any(|s| s.session_id == session_id) {
                                return Ok(store);
                            }
                        }
                    }
                }
            }
        }
        Err(ProtocolError::PayloadDecodeFailed("session not found in any agent"))
    }
}

#[async_trait]
impl DomainHandler for SessionHandler {
    fn name(&self) -> &str {
        "session"
    }

    fn operations(&self) -> Vec<Operation> {
        vec![
            Operation::Session(SessionOperation::List),
            Operation::Session(SessionOperation::Resume),
            Operation::Session(SessionOperation::Entries),
        ]
    }

    async fn handle(
        &self,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        let op = match &message.operation {
            Operation::Session(op) => op.clone(),
            _ => return Err(ProtocolError::PayloadDecodeFailed("session")),
        };
        match (op, message.payload) {
            (SessionOperation::List, Payload::Session(SessionPayload::List)) => {
                let mut all_sessions: Vec<serde_json::Value> = Vec::new();

                if self.agents_root.is_dir() {
                    for entry in std::fs::read_dir(&self.agents_root).into_iter().flatten().flatten() {
                        if entry.path().is_dir() {
                            if let Some(agent_id) = entry.file_name().to_str() {
                                let store = self.agent_store(agent_id);
                                if let Ok(summaries) = store.list_sessions() {
                                    for s in summaries {
                                        all_sessions.push(serde_json::json!({
                                            "agent_id": agent_id,
                                            "session_id": s.session_id,
                                            "entry_count": s.entry_count,
                                            "created_at": s.created_at,
                                        }));
                                    }
                                }
                            }
                        }
                    }
                }

                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Session(SessionOperation::List),
                    Payload::Session(SessionPayload::ListResult { sessions: all_sessions }),
                )])
            }
            (SessionOperation::Resume, Payload::Session(SessionPayload::Resume { session_id, agent_id })) => {
                let store = match agent_id {
                    Some(id) => self.agent_store(&id),
                    None => self.find_store_for_session(&session_id)?,
                };
                match store.get_entries(&session_id).await {
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
            (SessionOperation::Entries, Payload::Session(SessionPayload::Entries { session_id, agent_id })) => {
                let store = match agent_id {
                    Some(id) => self.agent_store(&id),
                    None => self.find_store_for_session(&session_id)?,
                };
                match store.get_entries(&session_id).await {
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
