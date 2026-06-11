use std::sync::Arc;

use vol_llm_agent_protocol::agent_server_protocol::{
    AgentOperation, ControlOperation, McpOperation, Operation, SessionOperation, SkillOperation,
    TaskOperation, ToolOperation,
};
use vol_llm_agent_protocol::{Connection, JsonRpcMessageService};

use crate::control_plane::core::ControlPlaneServerCore;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlConnectionRole {
    Client,
    DataPlaneNode,
}

impl ControlConnectionRole {
    pub fn allows(&self, operation: &Operation) -> bool {
        match self {
            ControlConnectionRole::Client => matches!(
                operation,
                Operation::Agent(AgentOperation::List)
                    | Operation::Agent(AgentOperation::Status)
                    | Operation::Agent(AgentOperation::Submit)
                    | Operation::Agent(AgentOperation::Cancel)
                    | Operation::Tool(ToolOperation::List)
                    | Operation::Tool(ToolOperation::Call)
                    | Operation::Mcp(McpOperation::ListServers)
                    | Operation::Mcp(McpOperation::ListTools)
                    | Operation::Mcp(McpOperation::CallTool)
                    | Operation::Mcp(McpOperation::ServerStatus)
                    | Operation::Skill(SkillOperation::List)
                    | Operation::Task(TaskOperation::List)
                    | Operation::Task(TaskOperation::Get)
                    | Operation::Session(SessionOperation::List)
                    | Operation::Session(SessionOperation::Entries)
                    | Operation::Control(ControlOperation::NodeList)
                    | Operation::Control(ControlOperation::NodeGet)
                    | Operation::Control(ControlOperation::CapabilityList)
                    | Operation::Control(ControlOperation::RunStatus)
            ),
            ControlConnectionRole::DataPlaneNode => matches!(
                operation,
                Operation::Control(ControlOperation::Register)
                    | Operation::Control(ControlOperation::Heartbeat)
                    | Operation::Control(ControlOperation::CapabilitySnapshot)
                    | Operation::Control(ControlOperation::CapabilityDelta)
                    | Operation::Control(ControlOperation::Event)
                    | Operation::Control(ControlOperation::CommandResult)
            ),
        }
    }
}

pub struct ControlPlaneEndpoint {
    core: Arc<ControlPlaneServerCore>,
    role: ControlConnectionRole,
}

impl ControlPlaneEndpoint {
    pub fn new(core: Arc<ControlPlaneServerCore>, role: ControlConnectionRole) -> Self {
        Self { core, role }
    }
}

#[async_trait::async_trait]
impl JsonRpcMessageService for ControlPlaneEndpoint {
    async fn serve_connection(&self, conn: Arc<dyn Connection>) {
        self.core.serve_connection_with_role(self.role, conn).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_agent_protocol::agent_server_protocol::{
        AgentOperation, ControlOperation, Operation,
    };

    #[test]
    fn client_endpoint_allows_agent_submit_and_denies_register() {
        assert!(ControlConnectionRole::Client.allows(&Operation::Agent(AgentOperation::Submit)));
        assert!(
            !ControlConnectionRole::Client.allows(&Operation::Control(ControlOperation::Register))
        );
    }

    #[test]
    fn node_endpoint_allows_register_and_denies_agent_submit() {
        assert!(ControlConnectionRole::DataPlaneNode
            .allows(&Operation::Control(ControlOperation::Register)));
        assert!(
            !ControlConnectionRole::DataPlaneNode.allows(&Operation::Agent(AgentOperation::Submit))
        );
    }

    #[test]
    fn client_allows_agent_list() {
        assert!(ControlConnectionRole::Client.allows(&Operation::Agent(AgentOperation::List)));
    }

    #[test]
    fn client_allows_node_list() {
        assert!(
            ControlConnectionRole::Client.allows(&Operation::Control(ControlOperation::NodeList))
        );
    }

    #[test]
    fn node_denies_agent_list() {
        assert!(
            !ControlConnectionRole::DataPlaneNode.allows(&Operation::Agent(AgentOperation::List))
        );
    }

    #[test]
    fn node_denies_node_list() {
        assert!(!ControlConnectionRole::DataPlaneNode
            .allows(&Operation::Control(ControlOperation::NodeList)));
    }
}
