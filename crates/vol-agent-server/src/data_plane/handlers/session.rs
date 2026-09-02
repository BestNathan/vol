use std::sync::Arc;

use async_trait::async_trait;
use vol_session::{Session, SessionManager, StoreError};

use crate::data_plane::router::AgentRouter;
use vol_llm_agent_protocol::agent_server_protocol::{
    AgentServerMessage, ErrorPayload, Operation, Payload, ProtocolError, SessionOperation,
    SessionPayload,
};
use vol_llm_agent_protocol::DomainHandler;

/// Handler for session-domain operations.
///
/// Uses the runtime-owned session manager so file and database backed session
/// stores share the same JSON-RPC behavior.
pub struct SessionHandler {
    session_manager: Arc<dyn SessionManager>,
    router: AgentRouter,
}

impl SessionHandler {
    pub fn new(session_manager: Arc<dyn SessionManager>, router: AgentRouter) -> Self {
        Self {
            session_manager,
            router,
        }
    }
}

fn session_store_error_payload(error: StoreError, fallback_code: &'static str) -> ErrorPayload {
    let message = error.to_string();
    let code = match &error {
        StoreError::NotFound(_) => "session_not_found",
        StoreError::InvalidInput(_) => "invalid_request",
        StoreError::SessionAgentScopeConflict { .. } => "session_scope_conflict",
        StoreError::Internal(detail) if detail.contains("ambiguous session") => "ambiguous_session",
        StoreError::Database(_) => "session_store_failed",
        _ => fallback_code,
    };

    ErrorPayload {
        code: code.to_string(),
        message,
        detail: None,
        terminal: true,
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
                let sessions = match self
                    .session_manager
                    .list_sessions(agent_id.as_deref())
                    .await
                {
                    Ok(sessions) => sessions,
                    Err(e) => {
                        return Ok(vec![AgentServerMessage::new_error(
                            message.message_id,
                            Operation::Session(SessionOperation::List),
                            session_store_error_payload(e, "session_list_failed"),
                        )]);
                    }
                }
                .into_iter()
                .map(|s| {
                    serde_json::json!({
                        "id": s.id,
                        "agent_id": s.agent_id,
                        "session_id": s.session_id,
                        "entry_count": s.entry_count,
                        "created_at": s.created_at,
                        "metadata": s.metadata,
                    })
                })
                .collect();

                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Session(SessionOperation::List),
                    Payload::Session(SessionPayload::ListResult { sessions }),
                )])
            }
            (
                SessionOperation::Resume,
                Payload::Session(SessionPayload::Resume {
                    session_id,
                    agent_id,
                }),
            ) => {
                let resolved_agent_id = match self
                    .session_manager
                    .resolve_session_agent(agent_id.as_deref(), &session_id)
                    .await
                {
                    Ok(agent_id) => agent_id,
                    Err(e) => {
                        return Ok(vec![AgentServerMessage::new_error(
                            message.message_id,
                            Operation::Session(SessionOperation::Resume),
                            session_store_error_payload(e, "session_resume_failed"),
                        )]);
                    }
                };

                let session_store = match self
                    .session_manager
                    .entry_store_for_session(Some(&resolved_agent_id), &session_id)
                    .await
                {
                    Ok(store) => store,
                    Err(e) => {
                        return Ok(vec![AgentServerMessage::new_error(
                            message.message_id,
                            Operation::Session(SessionOperation::Resume),
                            session_store_error_payload(e, "session_resume_failed"),
                        )]);
                    }
                };

                match session_store.get_entries(&session_id).await {
                    Ok(entries) => {
                        // Swap the agent's active session so subsequent messages go here.
                        match Session::resume(session_id.clone(), session_store.clone()).await {
                            Ok(session) => {
                                match self
                                    .router
                                    .swap_session(&resolved_agent_id, Arc::new(session))
                                    .await
                                {
                                    Ok(()) => {}
                                    Err(e) => {
                                        tracing::warn!(%session_id, %resolved_agent_id, %e, "session entries loaded but swap failed");
                                        return Ok(vec![AgentServerMessage::new_error(
                                            message.message_id,
                                            Operation::Session(SessionOperation::Resume),
                                            vol_llm_agent_protocol::agent_server_protocol::ErrorPayload {
                                                code: "agent_busy".to_string(),
                                                message: format!(
                                                    "Session loaded but agent is running — try again when idle: {e}"
                                                ),
                                                detail: None,
                                                terminal: false,
                                            },
                                        )]);
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!(%session_id, %resolved_agent_id, %e, "session entries loaded but resume failed");
                                return Ok(vec![AgentServerMessage::new_error(
                                    message.message_id,
                                    Operation::Session(SessionOperation::Resume),
                                    ErrorPayload {
                                        code: "session_resume_failed".to_string(),
                                        message: format!("Failed to resume session: {e}"),
                                        detail: None,
                                        terminal: true,
                                    },
                                )]);
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
                        session_store_error_payload(e, "session_resume_failed"),
                    )]),
                }
            }
            (
                SessionOperation::Entries,
                Payload::Session(SessionPayload::Entries {
                    session_id,
                    agent_id,
                }),
            ) => {
                let session_store = match self
                    .session_manager
                    .entry_store_for_session(agent_id.as_deref(), &session_id)
                    .await
                {
                    Ok(store) => store,
                    Err(e) => {
                        return Ok(vec![AgentServerMessage::new_error(
                            message.message_id,
                            Operation::Session(SessionOperation::Entries),
                            session_store_error_payload(e, "session_entries_failed"),
                        )]);
                    }
                };

                match session_store.get_entries(&session_id).await {
                    Ok(entries) => {
                        let json_entries: Vec<serde_json::Value> = entries
                            .into_iter()
                            .filter_map(|e| serde_json::to_value(e).ok())
                            .collect();
                        Ok(vec![AgentServerMessage::new_result(
                            message.message_id,
                            Operation::Session(SessionOperation::Entries),
                            Payload::Session(SessionPayload::EntriesResult {
                                entries: json_entries,
                            }),
                        )])
                    }
                    Err(e) => Ok(vec![AgentServerMessage::new_error(
                        message.message_id,
                        Operation::Session(SessionOperation::Entries),
                        session_store_error_payload(e, "session_entries_failed"),
                    )]),
                }
            }
            (SessionOperation::List, _) => Err(ProtocolError::PayloadDecodeFailed("session.list")),
            (SessionOperation::Resume, _) => {
                Err(ProtocolError::PayloadDecodeFailed("session.resume"))
            }
            (SessionOperation::Entries, _) => {
                Err(ProtocolError::PayloadDecodeFailed("session.entries"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use vol_llm_agent_protocol::agent_server_protocol::{
        AgentServerMessage, MessageKind, Operation, Payload, SessionOperation, SessionPayload,
    };
    use vol_llm_agent_protocol::DomainHandler;
    use vol_session::{DatabaseSessionManager, SessionManager};

    use crate::data_plane::router::AgentRouter;

    use super::SessionHandler;

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

    /// Set up a handler over a database-backed manager with a single session
    /// that has a task_ids binding. The database backend is what the production
    /// path uses, and it is the one that carries metadata through the
    /// `session_model_to_info` mapping we changed in this task.
    async fn handler_with_bound_session() -> (SessionHandler, tempfile::TempDir) {
        let temp = tempfile::tempdir().unwrap();
        let url = format!("sqlite://{}", temp.path().join("sessions.db").display());
        let manager = DatabaseSessionManager::connect(&url).await.unwrap();
        let store = manager.entry_store_for_agent("agent-a");
        store
            .save(vol_session::entry::SessionEntry {
                id: "entry-1".into(),
                session_id: "s1".into(),
                created_at: 10,
                parent_id: None,
                r#type: vol_session::entry::SessionEntryType::Summary,
                data: vol_session::entry::SessionEntryData::Summary {
                    summary: "summary-entry-1".into(),
                },
            })
            .await
            .expect("save");
        store
            .append_session_metadata_values("s1", "task_ids", &["1".into()])
            .await
            .expect("bind task_ids");
        let manager: Arc<dyn vol_session::SessionManager> = Arc::new(manager);
        let handler = SessionHandler::new(manager, AgentRouter::new());
        (handler, temp)
    }

    async fn list_sessions(handler: &SessionHandler) -> serde_json::Value {
        let replies = handler
            .handle(msg(
                "1",
                Operation::Session(SessionOperation::List),
                Payload::Session(SessionPayload::List { agent_id: None }),
            ))
            .await
            .expect("list");
        replies[0].payload.data_json()
    }

    #[tokio::test]
    async fn test_session_list_includes_metadata() {
        let (handler, _temp) = handler_with_bound_session().await;
        let result = list_sessions(&handler).await;
        let sessions = result["sessions"].as_array().expect("array");
        assert_eq!(
            sessions[0]["metadata"]["task_ids"],
            serde_json::json!(["1"])
        );
    }
}
