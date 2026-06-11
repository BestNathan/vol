use std::sync::Arc;

use async_trait::async_trait;
use vol_llm_skill::SkillLoader;

use vol_llm_agent_protocol::agent_server_protocol::{
    AgentServerMessage, Operation, Payload, ProtocolError, SkillOperation, SkillPayload,
};
use vol_llm_agent_protocol::DomainHandler;

/// Handler for skill-domain operations.
pub struct SkillHandler {
    skill_loader: Option<Arc<SkillLoader>>,
}

impl SkillHandler {
    pub fn new(skill_loader: Option<Arc<SkillLoader>>) -> Self {
        Self { skill_loader }
    }
}

#[async_trait]
impl DomainHandler for SkillHandler {
    fn name(&self) -> &str {
        "skill"
    }

    fn operations(&self) -> Vec<Operation> {
        vec![
            Operation::Skill(SkillOperation::List),
            Operation::Skill(SkillOperation::Get),
            Operation::Skill(SkillOperation::Refresh),
        ]
    }

    async fn handle(
        &self,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        let op = match &message.operation {
            Operation::Skill(op) => op.clone(),
            _ => return Err(ProtocolError::PayloadDecodeFailed("skill")),
        };
        match (op, message.payload) {
            (SkillOperation::List, Payload::Skill(SkillPayload::List)) => {
                let skills = match &self.skill_loader {
                    Some(loader) => {
                        let metadata = loader.list_metadata().await;
                        metadata
                            .iter()
                            .map(|m| {
                                serde_json::json!({
                                    "id": m.id,
                                    "name": m.name,
                                    "version": m.version,
                                    "scope": m.scope.to_string(),
                                    "description": m.description,
                                    "triggers": m.triggers,
                                })
                            })
                            .collect()
                    }
                    None => vec![],
                };
                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Skill(SkillOperation::List),
                    Payload::Skill(SkillPayload::ListResult { skills }),
                )])
            }
            (SkillOperation::Get, Payload::Skill(SkillPayload::Get { name })) => {
                let skill = match &self.skill_loader {
                    Some(loader) => loader.get(&name).await.map(|s| {
                        serde_json::json!({
                            "name": s.name,
                            "version": s.version,
                            "scope": s.scope.to_string(),
                            "description": s.description,
                            "triggers": s.triggers,
                            "content": s.content,
                            "file_listing": s.file_listing,
                            "directory": s.directory,
                        })
                    }),
                    None => None,
                };
                match skill {
                    Some(skill) => Ok(vec![AgentServerMessage::new_result(
                        message.message_id,
                        Operation::Skill(SkillOperation::Get),
                        Payload::Skill(SkillPayload::GetResult { skill, name }),
                    )]),
                    None => Ok(vec![AgentServerMessage::new_error(
                        message.message_id,
                        Operation::Skill(SkillOperation::Get),
                        vol_llm_agent_protocol::agent_server_protocol::ErrorPayload {
                            code: "skill_not_found".to_string(),
                            message: format!("Skill '{name}' not found"),
                            detail: None,
                            terminal: false,
                        },
                    )]),
                }
            }
            (SkillOperation::Refresh, Payload::Skill(SkillPayload::Refresh)) => {
                let discovered = match &self.skill_loader {
                    Some(loader) => match loader.discover_all().await {
                        Ok(()) => loader.list_metadata().await.len(),
                        Err(e) => {
                            return Ok(vec![AgentServerMessage::new_error(
                                message.message_id,
                                Operation::Skill(SkillOperation::Refresh),
                                vol_llm_agent_protocol::agent_server_protocol::ErrorPayload {
                                    code: "skill_refresh_failed".to_string(),
                                    message: e.to_string(),
                                    detail: None,
                                    terminal: false,
                                },
                            )]);
                        }
                    },
                    None => 0,
                };
                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Skill(SkillOperation::Refresh),
                    Payload::Skill(SkillPayload::RefreshResult { discovered }),
                )])
            }
            (SkillOperation::List, _) => Err(ProtocolError::PayloadDecodeFailed("skill.list")),
            (SkillOperation::Get, _) => Err(ProtocolError::PayloadDecodeFailed("skill.get")),
            (SkillOperation::Refresh, _) => {
                Err(ProtocolError::PayloadDecodeFailed("skill.refresh"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use vol_llm_agent_protocol::agent_server_protocol::{
        AgentServerMessage, MessageKind, Operation, Payload, SkillOperation, SkillPayload,
    };
    use vol_llm_agent_protocol::DomainHandler;

    use super::SkillHandler;

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
    async fn skill_list_returns_empty_without_loader() {
        let handler = SkillHandler::new(None);
        let replies = handler
            .handle(msg(
                "1",
                Operation::Skill(SkillOperation::List),
                Payload::Skill(SkillPayload::List),
            ))
            .await
            .unwrap();
        let json = replies[0].payload.data_json();
        let skills = json["skills"].as_array().unwrap();
        assert!(skills.is_empty());
    }

    #[tokio::test]
    async fn skill_get_returns_not_found_without_loader() {
        let handler = SkillHandler::new(None);
        let replies = handler
            .handle(msg(
                "1",
                Operation::Skill(SkillOperation::Get),
                Payload::Skill(SkillPayload::Get {
                    name: "nonexistent".to_string(),
                }),
            ))
            .await
            .unwrap();
        let json = replies[0].payload.data_json();
        assert_eq!(json["code"], "skill_not_found");
    }

    #[tokio::test]
    async fn skill_refresh_returns_zero_without_loader() {
        let handler = SkillHandler::new(None);
        let replies = handler
            .handle(msg(
                "1",
                Operation::Skill(SkillOperation::Refresh),
                Payload::Skill(SkillPayload::Refresh),
            ))
            .await
            .unwrap();
        let json = replies[0].payload.data_json();
        assert_eq!(json["discovered"], 0);
    }

    #[tokio::test]
    async fn skill_handler_rejects_non_skill_operation() {
        let handler = SkillHandler::new(None);
        let err = handler
            .handle(msg(
                "1",
                Operation::Log(vol_llm_agent_protocol::agent_server_protocol::LogOperation::List),
                Payload::Log(vol_llm_agent_protocol::agent_server_protocol::LogPayload::List),
            ))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("skill"));
    }

    #[tokio::test]
    async fn skill_list_with_wrong_payload_returns_error() {
        let handler = SkillHandler::new(None);
        let err = handler
            .handle(msg(
                "1",
                Operation::Skill(SkillOperation::List),
                Payload::Skill(SkillPayload::Get {
                    name: "x".to_string(),
                }),
            ))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("skill.list"));
    }
}
