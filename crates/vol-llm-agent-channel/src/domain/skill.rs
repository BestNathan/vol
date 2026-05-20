use crate::agent_server_protocol::{
    AgentServerMessage, Operation, Payload, ProtocolError, SkillOperation, SkillPayload,
};

/// Handler for skill-domain operations.
pub struct SkillHandler;

impl SkillHandler {
    pub async fn handle(
        &self,
        operation: SkillOperation,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        match (operation, message.payload) {
            (SkillOperation::List, Payload::Skill(SkillPayload::List)) => Ok(vec![AgentServerMessage::new_result(
                message.message_id,
                Operation::Skill(SkillOperation::List),
                Payload::Skill(SkillPayload::ListResult { skills: vec![] }),
            )]),
            (SkillOperation::Get, Payload::Skill(SkillPayload::Get { name })) => Ok(vec![AgentServerMessage::new_result(
                message.message_id,
                Operation::Skill(SkillOperation::Get),
                Payload::Skill(SkillPayload::GetResult {
                    skill: serde_json::Value::Null,
                    name,
                }),
            )]),
            (SkillOperation::List, _) => Err(ProtocolError::PayloadDecodeFailed("skill.list")),
            (SkillOperation::Get, _) => Err(ProtocolError::PayloadDecodeFailed("skill.get")),
        }
    }
}
