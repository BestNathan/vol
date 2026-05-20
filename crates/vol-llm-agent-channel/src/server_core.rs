use crate::agent_server_protocol::{AgentServerMessage, Operation, ProtocolError};
use crate::domain::{
    agent::AgentHandler,
    file::FileHandler,
    log::LogHandler,
    mcp::McpHandler,
    session::SessionHandler,
    skill::SkillHandler,
    system::SystemHandler,
};
use std::sync::Arc;

/// Core dispatch engine for the agent server protocol.
pub struct AgentServerCore {
    pub agent: AgentHandler,
    pub file: FileHandler,
    pub session: SessionHandler,
    pub mcp: McpHandler,
    pub skill: SkillHandler,
    pub log: LogHandler,
    pub system: SystemHandler,
}

impl AgentServerCore {
    pub fn new() -> Self {
        let session_store = Arc::new(vol_session::FileSessionEntryStore::new("/tmp/vol-llm-agent-channel-test-sessions"));
        Self {
            agent: AgentHandler,
            file: FileHandler,
            session: SessionHandler::new(session_store),
            mcp: McpHandler,
            skill: SkillHandler,
            log: LogHandler,
            system: SystemHandler,
        }
    }

    pub fn for_test() -> Self {
        Self::new()
    }

    pub async fn handle(
        &self,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        match message.operation.clone() {
            Operation::Agent(op) => self.agent.handle(op, message).await,
            Operation::File(op) => self.file.handle(op, message).await,
            Operation::Session(op) => self.session.handle(op, message).await,
            Operation::Mcp(op) => self.mcp.handle(op, message).await,
            Operation::Skill(op) => self.skill.handle(op, message).await,
            Operation::Log(op) => self.log.handle(op, message).await,
            Operation::System(op) => self.system.handle(op, message).await,
        }
    }
}
