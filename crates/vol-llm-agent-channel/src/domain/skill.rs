use std::sync::Arc;

use vol_llm_skill::SkillLoader;

use crate::agent_server_protocol::{
    AgentServerMessage, Operation, Payload, ProtocolError, SkillOperation, SkillPayload,
};

/// Handler for skill-domain operations.
pub struct SkillHandler {
    skill_loader: Option<Arc<SkillLoader>>,
}

impl SkillHandler {
    pub fn new(skill_loader: Option<Arc<SkillLoader>>) -> Self {
        Self { skill_loader }
    }

    pub async fn handle(
        &self,
        operation: SkillOperation,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        match (operation, message.payload) {
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
            (SkillOperation::List, _) => Err(ProtocolError::PayloadDecodeFailed("skill.list")),
            (SkillOperation::Get, _) => Err(ProtocolError::PayloadDecodeFailed("skill.get")),
        }
    }
}
