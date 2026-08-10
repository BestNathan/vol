use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use vol_session::{Session, SessionManager};

use crate::data_plane::connection_holder::ConnectionHolder;
use crate::data_plane::router::AgentRouter;
use vol_llm_agent_protocol::agent_server_protocol::{
    AgentOperation, AgentPayload, AgentServerMessage, Operation, Payload, ProtocolError,
    ProviderInfo,
};
use vol_llm_agent_protocol::request::AgentRequest;
use vol_llm_agent_protocol::DomainHandler;

/// Handler for agent-domain operations.
pub struct AgentHandler {
    router: AgentRouter,
    holders: Arc<std::sync::Mutex<HashMap<String, Arc<ConnectionHolder>>>>,
    agent_defs: Arc<std::sync::RwLock<HashMap<String, vol_llm_core::AgentDef>>>,
    agent_status: Arc<std::sync::RwLock<HashMap<String, crate::data_plane::core::AgentStatus>>>,
    session_manager: Arc<dyn SessionManager>,
    /// Tracks the last session_id used per agent so we can detect "New Session".
    agent_last_session: std::sync::Mutex<HashMap<String, String>>,
}

impl AgentHandler {
    pub fn new(
        router: AgentRouter,
        holders: Arc<std::sync::Mutex<HashMap<String, Arc<ConnectionHolder>>>>,
        agent_defs: Arc<std::sync::RwLock<HashMap<String, vol_llm_core::AgentDef>>>,
        agent_status: Arc<std::sync::RwLock<HashMap<String, crate::data_plane::core::AgentStatus>>>,
        session_manager: Arc<dyn SessionManager>,
    ) -> Self {
        Self {
            router,
            holders,
            agent_defs,
            agent_status,
            session_manager,
            agent_last_session: std::sync::Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl DomainHandler for AgentHandler {
    fn name(&self) -> &str {
        "agent"
    }

    fn operations(&self) -> Vec<Operation> {
        vec![
            Operation::Agent(AgentOperation::Submit),
            Operation::Agent(AgentOperation::Cancel),
            Operation::Agent(AgentOperation::Subscribe),
            Operation::Agent(AgentOperation::Unsubscribe),
            Operation::Agent(AgentOperation::Approve),
            Operation::Agent(AgentOperation::List),
            Operation::Agent(AgentOperation::Event),
            Operation::Agent(AgentOperation::Status),
            Operation::Agent(AgentOperation::ContextConfig),
            Operation::Agent(AgentOperation::ContextSnapshot),
        ]
    }

    #[allow(clippy::unwrap_used)]
    async fn handle(
        &self,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        let op = match &message.operation {
            Operation::Agent(op) => op.clone(),
            _ => return Err(ProtocolError::PayloadDecodeFailed("agent")),
        };
        match (op, message.payload) {
            (AgentOperation::Submit, Payload::Agent(AgentPayload::Submit { input, target })) => {
                let target_id = {
                    let holders = self.holders.lock().unwrap();
                    target
                        .filter(|t| holders.contains_key(t))
                        .or_else(|| holders.keys().next().cloned())
                        .unwrap_or_else(|| "agent".to_string())
                };

                // Extract session_id from metadata — if it changed, create a new
                // session and swap it so the agent starts with fresh context.
                let new_session_id = input
                    .metadata
                    .get("session_id")
                    .and_then(|v| v.as_str())
                    .map(ToString::to_string);
                let session_changed = new_session_id.as_ref().is_some_and(|sid| {
                    let last = self.agent_last_session.lock().unwrap();
                    last.get(&target_id) != Some(sid)
                });
                if session_changed {
                    let store = self.session_manager.entry_store_for_agent(&target_id);
                    // Use the frontend's session_id so session.list/entries can find it.
                    let session = Arc::new(Session::with_id(
                        new_session_id.clone().unwrap_or_default(),
                        store,
                    ));
                    self.router
                        .swap_session(&target_id, session)
                        .await
                        .unwrap_or_else(|e| {
                            tracing::warn!(
                                agent = %target_id,
                                error = %e,
                                "Failed to swap session for agent"
                            );
                        });
                    if let Some(ref sid) = new_session_id {
                        self.agent_last_session
                            .lock()
                            .unwrap()
                            .insert(target_id.clone(), sid.clone());
                    }
                }

                let run_id = input
                    .run_id
                    .clone()
                    .unwrap_or_else(|| uuid::Uuid::new_v4().simple().to_string());
                let run_id_clone = run_id.clone();
                let request = AgentRequest::new(&target_id, input);

                // Read resolved config from agent instance
                let agent = self.router.get_agent(&target_id).await;
                let (provider, tools, mcps, skills) = match agent {
                    Some(ref a) => {
                        let tools_obj = a.tools();
                        let skills_obj = a.skills();
                        let mcps_obj = a.mcps();
                        let llm = a.llm();
                        (
                            ProviderInfo {
                                name: llm.provider().to_string(),
                                model: llm.model().to_string(),
                            },
                            tools_obj
                                .definitions()
                                .iter()
                                .map(|d| d.name.clone())
                                .collect(),
                            mcps_obj.server_status().keys().cloned().collect(),
                            skills_obj.skill_names().await,
                        )
                    }
                    None => (
                        ProviderInfo {
                            name: "unknown".into(),
                            model: "unknown".into(),
                        },
                        vec![],
                        vec![],
                        vec![],
                    ),
                };

                match self.router.send(&target_id, request).await {
                    Ok(rx) => {
                        let router = self.router.clone();
                        tokio::spawn(async move {
                            Self::process_run_result(rx, &run_id_clone, &router).await;
                        });

                        // Single response — no more Ack + Result pair
                        Ok(vec![AgentServerMessage::new_result(
                            message.message_id,
                            Operation::Agent(AgentOperation::Submit),
                            Payload::Agent(AgentPayload::SubmitResult {
                                run_id,
                                accepted: true,
                                provider,
                                tools,
                                mcps,
                                skills,
                            }),
                        )])
                    }
                    Err(e) => Ok(vec![AgentServerMessage::new_error(
                        message.message_id,
                        Operation::Agent(AgentOperation::Submit),
                        vol_llm_agent_protocol::agent_server_protocol::ErrorPayload {
                            code: "agent_submit_failed".to_string(),
                            message: e.to_string(),
                            detail: None,
                            terminal: true,
                        },
                    )]),
                }
            }
            (AgentOperation::Cancel, Payload::Agent(AgentPayload::Cancel { run_id })) => {
                let cancelled = self.router.cancel(&run_id).await;
                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Agent(AgentOperation::Cancel),
                    Payload::Agent(AgentPayload::CancelResult { run_id, cancelled }),
                )])
            }
            (AgentOperation::Subscribe, Payload::Agent(AgentPayload::Subscribe { .. })) => {
                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Agent(AgentOperation::Subscribe),
                    Payload::Agent(AgentPayload::SubscribeResult {
                        subscription_id: uuid::Uuid::new_v4().to_string(),
                    }),
                )])
            }
            (
                AgentOperation::Unsubscribe,
                Payload::Agent(AgentPayload::Unsubscribe { subscription_id }),
            ) => Ok(vec![AgentServerMessage::new_result(
                message.message_id,
                Operation::Agent(AgentOperation::Unsubscribe),
                Payload::Agent(AgentPayload::UnsubscribeResult {
                    subscription_id,
                    removed: true,
                }),
            )]),
            (AgentOperation::Approve, Payload::Agent(AgentPayload::Approve { run_id, .. })) => {
                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Agent(AgentOperation::Approve),
                    Payload::Agent(AgentPayload::ApproveResult {
                        run_id,
                        accepted: true,
                    }),
                )])
            }
            (AgentOperation::List, _) => {
                let defs = self.agent_defs.read().unwrap();
                let holder_keys: Vec<String> =
                    self.holders.lock().unwrap().keys().cloned().collect();
                let mut agents: Vec<serde_json::Value> = holder_keys
                    .iter()
                    .map(|k| {
                        let def = defs.get(k);
                        serde_json::json!({
                            "id": k,
                            "name": k,
                            "type": def.map_or("unknown", |d| &d.r#type),
                            "description": def.and_then(|d| if d.description.is_empty() { None } else { Some(d.description.as_str()) }).unwrap_or(""),
                            "scope": def.map_or("repo", |d| match d.scope {
                                vol_llm_core::AgentScope::Repo => "repo",
                                vol_llm_core::AgentScope::User => "user",
                            }),
                            "status": "idle",
                            "current_input": None::<String>,
                        })
                    })
                    .collect();

                // Stable sort: repo first, user second; alphabetical by name within group
                fn scope_rank(scope: &str) -> u8 {
                    match scope {
                        "repo" => 0,
                        _ => 1,
                    }
                }
                agents.sort_by(|a, b| {
                    let sa = a.get("scope").and_then(|v| v.as_str()).unwrap_or("");
                    let sb = b.get("scope").and_then(|v| v.as_str()).unwrap_or("");
                    let na = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let nb = b.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    scope_rank(sa).cmp(&scope_rank(sb)).then_with(|| na.cmp(nb))
                });

                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Agent(AgentOperation::List),
                    Payload::Agent(AgentPayload::ListResult { agents }),
                )])
            }
            (AgentOperation::Event, Payload::Agent(AgentPayload::Event { run_id, event })) => {
                Ok(vec![AgentServerMessage::new_event(
                    message.message_id,
                    Operation::Agent(AgentOperation::Event),
                    Payload::Agent(AgentPayload::Event { run_id, event }),
                )])
            }
            (AgentOperation::Submit, _) => Err(ProtocolError::PayloadDecodeFailed("agent.submit")),
            (AgentOperation::Cancel, _) => Err(ProtocolError::PayloadDecodeFailed("agent.cancel")),
            (AgentOperation::Subscribe, _) => {
                Err(ProtocolError::PayloadDecodeFailed("agent.subscribe"))
            }
            (AgentOperation::Unsubscribe, _) => {
                Err(ProtocolError::PayloadDecodeFailed("agent.unsubscribe"))
            }
            (AgentOperation::Approve, _) => {
                Err(ProtocolError::PayloadDecodeFailed("agent.approve"))
            }
            (AgentOperation::Status, Payload::Agent(AgentPayload::Status { agent_id })) => {
                let status_entry = self.agent_status.read().unwrap().get(&agent_id).cloned();
                let (status, run_id) = match status_entry {
                    Some(s) if s.status == "running" => (s.status, s.run_id),
                    _ => {
                        let is_busy = self.router.is_agent_running(&agent_id).await;
                        let status = if is_busy {
                            "running".to_string()
                        } else {
                            "idle".to_string()
                        };
                        (status, None)
                    }
                };
                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Agent(AgentOperation::Status),
                    Payload::Agent(AgentPayload::StatusResult { status, run_id }),
                )])
            }
            (AgentOperation::Status, _) => Err(ProtocolError::PayloadDecodeFailed("agent.status")),
            (AgentOperation::Event, _) => Err(ProtocolError::PayloadDecodeFailed("agent.event")),
            (
                AgentOperation::ContextConfig,
                Payload::Agent(AgentPayload::ContextConfig { agent_id }),
            ) => {
                let agent = match self.router.get_agent(&agent_id).await {
                    Some(a) => a,
                    None => {
                        return Ok(vec![AgentServerMessage::new_error(
                            message.message_id,
                            Operation::Agent(AgentOperation::ContextConfig),
                            vol_llm_agent_protocol::agent_server_protocol::ErrorPayload {
                                code: "agent_not_found".to_string(),
                                message: format!("agent '{agent_id}' not found"),
                                detail: None,
                                terminal: true,
                            },
                        )])
                    }
                };

                let contributors = agent
                    .contributors()
                    .await
                    .map(|infos| {
                        infos
                            .into_iter()
                            .map(|info| {
                                serde_json::json!({
                                    "name": info.name,
                                    "anchor_zone": info.anchor_zone,
                                    "estimated_tokens": info.estimated_tokens,
                                    "message_count": info.message_count,
                                })
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();

                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Agent(AgentOperation::ContextConfig),
                    Payload::Agent(AgentPayload::ContextConfigResult { contributors }),
                )])
            }
            (
                AgentOperation::ContextSnapshot,
                Payload::Agent(AgentPayload::ContextSnapshot {
                    agent_id,
                    contributor_name,
                }),
            ) => {
                let agent = match self.router.get_agent(&agent_id).await {
                    Some(a) => a,
                    None => {
                        return Ok(vec![AgentServerMessage::new_error(
                            message.message_id,
                            Operation::Agent(AgentOperation::ContextSnapshot),
                            vol_llm_agent_protocol::agent_server_protocol::ErrorPayload {
                                code: "agent_not_found".to_string(),
                                message: format!("agent '{agent_id}' not found"),
                                detail: None,
                                terminal: true,
                            },
                        )])
                    }
                };

                let messages = agent
                    .snapshot_by_name(&contributor_name)
                    .await
                    .map(|msgs| {
                        msgs.into_iter()
                            .map(|m| serde_json::json!({"role": m.role, "content": m.content}))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();

                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Agent(AgentOperation::ContextSnapshot),
                    Payload::Agent(AgentPayload::ContextSnapshotResult { messages }),
                )])
            }
            (AgentOperation::ContextConfig, _) => {
                Err(ProtocolError::PayloadDecodeFailed("agent.context_config"))
            }
            (AgentOperation::ContextSnapshot, _) => {
                Err(ProtocolError::PayloadDecodeFailed("agent.context_snapshot"))
            }
            // Delegate to CapabilityHandler — shouldn't reach here via dispatch
            (AgentOperation::GetCapabilities, _) => {
                Err(ProtocolError::PayloadDecodeFailed("agent.get_capabilities"))
            }
            (AgentOperation::UpdateCapabilities, _) => Err(ProtocolError::PayloadDecodeFailed(
                "agent.update_capabilities",
            )),
        }
    }
}

impl AgentHandler {
    async fn process_run_result(
        rx: tokio::sync::oneshot::Receiver<vol_llm_agent_protocol::request::RunResult>,
        run_id: &str,
        _router: &AgentRouter,
    ) {
        match rx.await {
            Ok(result) => match &result.response {
                Ok(response) => {
                    tracing::info!(%run_id, iterations = response.iterations, "agent run completed");
                }
                Err(e) => {
                    tracing::error!(%run_id, %e, "agent run failed");
                }
            },
            Err(_) => {
                tracing::warn!(%run_id, "agent run receiver dropped (possibly cancelled)");
            }
        }
    }
}
