use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use crate::agent_server_protocol::{
    AgentOperation, AgentPayload, AgentServerMessage, Operation, Payload, ProtocolError,
};
use crate::connection::ConnectionHolder;
use crate::domain::handler::DomainHandler;
use crate::request::AgentRequest;
use crate::router::AgentRouter;

/// Handler for agent-domain operations.
pub struct AgentHandler {
    router: AgentRouter,
    holders: Arc<std::sync::Mutex<HashMap<String, Arc<ConnectionHolder>>>>,
    agent_defs: Arc<std::sync::RwLock<HashMap<String, vol_llm_agent::AgentDef>>>,
}

impl AgentHandler {
    pub fn new(
        router: AgentRouter,
        holders: Arc<std::sync::Mutex<HashMap<String, Arc<ConnectionHolder>>>>,
        agent_defs: Arc<std::sync::RwLock<HashMap<String, vol_llm_agent::AgentDef>>>,
    ) -> Self {
        Self { router, holders, agent_defs }
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
        ]
    }

    async fn handle(
        &self,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        let op = match &message.operation {
            Operation::Agent(op) => op.clone(),
            _ => return Err(ProtocolError::PayloadDecodeFailed("agent")),
        };
        match (op, message.payload) {
            (
                AgentOperation::Submit,
                Payload::Agent(AgentPayload::Submit {
                    input,
                    target,
                }),
            ) => {
                let target_id = {
                    let holders = self.holders.lock().unwrap();
                    target
                        .filter(|t| holders.contains_key(t))
                        .or_else(|| holders.keys().next().cloned())
                        .unwrap_or_else(|| "agent".to_string())
                };

                let run_id = input
                    .run_id
                    .clone()
                    .unwrap_or_else(|| uuid::Uuid::new_v4().simple().to_string());
                let run_id_clone = run_id.clone();
                let request = AgentRequest::new(&target_id, input);

                match self.router.send(&target_id, request).await {
                    Ok(rx) => {
                        let router = self.router.clone();
                        tokio::spawn(async move {
                            Self::process_run_result(rx, &run_id_clone, &router).await;
                        });

                        Ok(vec![
                            AgentServerMessage::new_ack(
                                message.message_id.clone(),
                                Operation::Agent(AgentOperation::Submit),
                                Payload::Agent(AgentPayload::SubmitAck {
                                    run_id: run_id.clone(),
                                    accepted: true,
                                }),
                            ),
                            AgentServerMessage::new_result(
                                message.message_id,
                                Operation::Agent(AgentOperation::Submit),
                                Payload::Agent(AgentPayload::SubmitResult {
                                    run_id: run_id.clone(),
                                    response: serde_json::json!({"run_id": run_id}),
                                }),
                            ),
                        ])
                    }
                    Err(e) => Ok(vec![AgentServerMessage::new_error(
                        message.message_id,
                        Operation::Agent(AgentOperation::Submit),
                        crate::agent_server_protocol::ErrorPayload {
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
                let agents: Vec<serde_json::Value> = self
                    .holders
                    .lock()
                    .unwrap()
                    .keys()
                    .map(|k| {
                        let def = defs.get(k);
                        serde_json::json!({
                            "id": k,
                            "name": k,
                            "type": def.map_or("unknown", |d| &d.r#type),
                            "description": def.and_then(|d| if d.description.is_empty() { None } else { Some(d.description.as_str()) }).unwrap_or(""),
                            "scope": def.map_or("repo", |d| match d.scope {
                                vol_llm_agent::AgentScope::User => "user",
                                vol_llm_agent::AgentScope::Repo => "repo",
                            }),
                        })
                    })
                    .collect();
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
            (AgentOperation::Event, _) => Err(ProtocolError::PayloadDecodeFailed("agent.event")),
        }
    }
}

impl AgentHandler {
    async fn process_run_result(
        rx: tokio::sync::oneshot::Receiver<crate::request::RunResult>,
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
