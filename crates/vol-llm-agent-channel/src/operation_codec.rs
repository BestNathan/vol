use crate::agent_server_protocol::{
    AgentOperation, FileOperation, LogOperation, McpOperation, Operation, Payload, ProtocolError,
    SessionOperation, SkillOperation, SystemOperation, TaskOperation, ToolOperation,
};

pub fn method_to_operation(method: &str) -> Result<Operation, ProtocolError> {
    match method {
        "agent.submit" => Ok(Operation::Agent(AgentOperation::Submit)),
        "agent.cancel" => Ok(Operation::Agent(AgentOperation::Cancel)),
        "agent.subscribe" => Ok(Operation::Agent(AgentOperation::Subscribe)),
        "agent.unsubscribe" => Ok(Operation::Agent(AgentOperation::Unsubscribe)),
        "agent.approve" => Ok(Operation::Agent(AgentOperation::Approve)),
        "agent.list" => Ok(Operation::Agent(AgentOperation::List)),
        "agent.event" => Ok(Operation::Agent(AgentOperation::Event)),
        "agent.status" => Ok(Operation::Agent(AgentOperation::Status)),
        "agent.context_config" => Ok(Operation::Agent(AgentOperation::ContextConfig)),
        "agent.context_snapshot" => Ok(Operation::Agent(AgentOperation::ContextSnapshot)),
        "task.list" => Ok(Operation::Task(TaskOperation::List)),
        "task.get" => Ok(Operation::Task(TaskOperation::Get)),
        "file.list" => Ok(Operation::File(FileOperation::List)),
        "file.read" => Ok(Operation::File(FileOperation::Read)),
        "session.list" => Ok(Operation::Session(SessionOperation::List)),
        "session.resume" => Ok(Operation::Session(SessionOperation::Resume)),
        "session.entries" => Ok(Operation::Session(SessionOperation::Entries)),
        "mcp.list_servers" => Ok(Operation::Mcp(McpOperation::ListServers)),
        "mcp.list_tools" => Ok(Operation::Mcp(McpOperation::ListTools)),
        "mcp.call_tool" => Ok(Operation::Mcp(McpOperation::CallTool)),
        "mcp.list_resources" => Ok(Operation::Mcp(McpOperation::ListResources)),
        "mcp.list_resource_templates" => Ok(Operation::Mcp(McpOperation::ListResourceTemplates)),
        "mcp.read_resource" => Ok(Operation::Mcp(McpOperation::ReadResource)),
        "mcp.list_prompts" => Ok(Operation::Mcp(McpOperation::ListPrompts)),
        "mcp.get_prompt" => Ok(Operation::Mcp(McpOperation::GetPrompt)),
        "mcp.reconnect" => Ok(Operation::Mcp(McpOperation::Reconnect)),
        "mcp.server_status" => Ok(Operation::Mcp(McpOperation::ServerStatus)),
        "skill.list" => Ok(Operation::Skill(SkillOperation::List)),
        "skill.get" => Ok(Operation::Skill(SkillOperation::Get)),
        "skill.refresh" => Ok(Operation::Skill(SkillOperation::Refresh)),
        "tool.list" => Ok(Operation::Tool(ToolOperation::List)),
        "tool.call" => Ok(Operation::Tool(ToolOperation::Call)),
        "log.list" => Ok(Operation::Log(LogOperation::List)),
        "log.read" => Ok(Operation::Log(LogOperation::Read)),
        "system.connected" => Ok(Operation::System(SystemOperation::Connected)),
        _ => Err(ProtocolError::UnknownMethod(method.to_string())),
    }
}

pub fn decode_payload(
    operation: Operation,
    value: serde_json::Value,
) -> Result<Payload, ProtocolError> {
    Payload::from_operation(&operation, value)
}
