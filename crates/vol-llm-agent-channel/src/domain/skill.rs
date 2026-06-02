use std::sync::Arc;

use async_trait::async_trait;
use vol_llm_skill::SkillLoader;

use crate::agent_server_protocol::{
    AgentServerMessage, Operation, Payload, ProtocolError, SkillOperation, SkillPayload,
};
use crate::domain::handler::DomainHandler;

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
                        metadata.iter().map(|m| {
                            serde_json::json!({
                                "id": m.id,
                                "name": m.name,
                                "version": m.version,
                                "scope": m.scope.to_string(),
                                "description": m.description,
                                "triggers": m.triggers,
                            })
                        }).collect()
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
                    Some(loader) => loader.get(&name).await.map(|s| serde_json::json!({
                        "name": s.name,
                        "version": s.version,
                        "scope": s.scope.to_string(),
                        "description": s.description,
                        "triggers": s.triggers,
                        "content": s.content,
                        "file_listing": s.file_listing,
                        "directory": s.directory,
                    })),
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
                        crate::agent_server_protocol::ErrorPayload {
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
                    Some(loader) => {
                        match loader.discover_all().await {
                            Ok(()) => loader.list_metadata().await.len(),
                            Err(e) => {
                                return Ok(vec![AgentServerMessage::new_error(
                                    message.message_id,
                                    Operation::Skill(SkillOperation::Refresh),
                                    crate::agent_server_protocol::ErrorPayload {
                                        code: "skill_refresh_failed".to_string(),
                                        message: e.to_string(),
                                        detail: None,
                                        terminal: false,
                                    },
                                )]);
                            }
                        }
                    }
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
            (SkillOperation::Refresh, _) => Err(ProtocolError::PayloadDecodeFailed("skill.refresh")),
        }
    }
}
