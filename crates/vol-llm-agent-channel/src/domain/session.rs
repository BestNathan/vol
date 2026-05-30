use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use vol_session::file_store::FileSessionEntryStore;
use vol_session::{Session, SessionEntryStore};

use crate::agent_server_protocol::{
    AgentServerMessage, Operation, Payload, ProtocolError, SessionOperation, SessionPayload,
};
use crate::domain::handler::DomainHandler;
use crate::router::AgentRouter;

/// Handler for session-domain operations.
///
/// Scans all agent directories under agents_root to aggregate sessions.
/// Holds a router reference to swap agent sessions on resume.
pub struct SessionHandler {
    agents_root: PathBuf,
    router: AgentRouter,
}

impl SessionHandler {
    pub fn new(agents_root: PathBuf, router: AgentRouter) -> Self {
        Self { agents_root, router }
    }

    /// Get a session store for a specific agent.
    fn agent_store(&self, agent_id: &str) -> FileSessionEntryStore {
        FileSessionEntryStore::new(self.agents_root.join(agent_id).join("sessions"))
    }

    /// Find which agent owns a session by scanning all agent dirs.
    fn find_store_for_session(&self, session_id: &str) -> Result<(FileSessionEntryStore, String), ProtocolError> {
        if self.agents_root.is_dir() {
            for entry in std::fs::read_dir(&self.agents_root).into_iter().flatten().flatten() {
                if entry.path().is_dir() {
                    if let Some(agent_id) = entry.file_name().to_str() {
                        let store = self.agent_store(agent_id);
                        if let Ok(summaries) = store.list_sessions() {
                            if summaries.iter().any(|s| s.session_id == session_id) {
                                return Ok((store, agent_id.to_string()));
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
            (SessionOperation::List, Payload::Session(SessionPayload::List { agent_id })) => {
                let mut all_sessions: Vec<serde_json::Value> = Vec::new();

                let agent_ids: Vec<String> = if let Some(ref aid) = agent_id {
                    vec![aid.clone()]
                } else if self.agents_root.is_dir() {
                    std::fs::read_dir(&self.agents_root)
                        .into_iter().flatten().flatten()
                        .filter(|e| e.path().is_dir())
                        .filter_map(|e| e.file_name().to_str().map(String::from))
                        .collect()
                } else {
                    vec![]
                };

                for aid in &agent_ids {
                    let store = self.agent_store(aid);
                    if let Ok(summaries) = store.list_sessions() {
                        for s in summaries {
                            all_sessions.push(serde_json::json!({
                                "id": s.session_id,
                                "agent_id": aid,
                                "session_id": s.session_id,
                                "entry_count": s.entry_count,
                                "created_at": s.created_at,
                            }));
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
                let (store, resolved_agent_id) = match agent_id {
                    Some(ref id) => (self.agent_store(id), id.clone()),
                    None => self.find_store_for_session(&session_id)?,
                };
                match store.get_entries(&session_id).await {
                    Ok(entries) => {
                        // Swap the agent's active session so subsequent messages go here.
                        let session_store: Arc<dyn SessionEntryStore> = Arc::new(store);
                        match Session::resume(session_id.clone(), session_store).await {
                            Ok(session) => {
                                if let Err(e) = self.router.swap_session(&resolved_agent_id, Arc::new(session)).await {
                                    tracing::warn!(%session_id, %resolved_agent_id, %e, "session entries loaded but swap failed");
                                }
                            }
                            Err(e) => {
                                tracing::warn!(%session_id, %resolved_agent_id, %e, "session entries loaded but resume failed");
                            }
                        }

                        let json_entries: Vec<serde_json::Value> = entries
                            .into_iter()
                            .filter_map(|e| serde_json::to_value(e).ok())
                            .collect();
                        let entry_count = json_entries.len();
                        Ok(vec![AgentServerMessage::new_result(
                            message.message_id,
                            Operation::Session(SessionOperation::Resume),
                            Payload::Session(SessionPayload::ResumeResult {
                                session_id,
                                restored: true,
                                entry_count,
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
                let (store, _agent_id) = match agent_id {
                    Some(id) => (self.agent_store(&id), id),
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
