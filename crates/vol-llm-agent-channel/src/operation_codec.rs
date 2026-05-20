use crate::agent_server_protocol::{
    AgentOperation, AgentPayload, FileOperation, FilePayload, LogOperation, LogPayload,
    McpOperation, McpPayload, Operation, Payload, ProtocolError, SessionOperation,
    SessionPayload, SkillOperation, SkillPayload, SystemOperation,
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
        "log.list" => Ok(Operation::Log(LogOperation::List)),
        "log.read" => Ok(Operation::Log(LogOperation::Read)),
        "system.connected" => Ok(Operation::System(SystemOperation::Connected)),
        _ => Err(ProtocolError::UnknownMethod(method.to_string())),
    }
}

pub fn decode_payload(operation: Operation, value: serde_json::Value) -> Result<Payload, ProtocolError> {
    match operation {
        Operation::Agent(AgentOperation::Submit) => {
            let obj = value.as_object().ok_or(ProtocolError::PayloadDecodeFailed("agent.submit"))?;
            let input = obj
                .get("input")
                .and_then(|v| v.as_str())
                .ok_or(ProtocolError::PayloadDecodeFailed("agent.submit"))?
                .to_string();
            let target = obj.get("target").and_then(|v| v.as_str()).map(ToString::to_string);
            let metadata = obj.get("metadata").and_then(|v| v.as_object()).cloned();
            Ok(Payload::Agent(AgentPayload::Submit { input, target, metadata }))
        }
        Operation::Agent(AgentOperation::Cancel) => {
            let obj = value.as_object().ok_or(ProtocolError::PayloadDecodeFailed("agent.cancel"))?;
            let run_id = obj
                .get("run_id")
                .and_then(|v| v.as_str())
                .ok_or(ProtocolError::PayloadDecodeFailed("agent.cancel"))?
                .to_string();
            Ok(Payload::Agent(AgentPayload::Cancel { run_id }))
        }
        Operation::Agent(AgentOperation::Subscribe) => {
            let target = value.as_object().and_then(|o| o.get("target")).and_then(|v| v.as_str()).map(ToString::to_string);
            Ok(Payload::Agent(AgentPayload::Subscribe { target }))
        }
        Operation::Agent(AgentOperation::Unsubscribe) => {
            let obj = value.as_object().ok_or(ProtocolError::PayloadDecodeFailed("agent.unsubscribe"))?;
            let subscription_id = obj
                .get("subscription_id")
                .and_then(|v| v.as_str())
                .ok_or(ProtocolError::PayloadDecodeFailed("agent.unsubscribe"))?
                .to_string();
            Ok(Payload::Agent(AgentPayload::Unsubscribe { subscription_id }))
        }
        Operation::Agent(AgentOperation::Approve) => {
            let obj = value.as_object().ok_or(ProtocolError::PayloadDecodeFailed("agent.approve"))?;
            let run_id = obj
                .get("run_id")
                .and_then(|v| v.as_str())
                .ok_or(ProtocolError::PayloadDecodeFailed("agent.approve"))?
                .to_string();
            let approved = obj
                .get("approved")
                .and_then(|v| v.as_bool())
                .ok_or(ProtocolError::PayloadDecodeFailed("agent.approve"))?;
            let reason = obj.get("reason").and_then(|v| v.as_str()).map(ToString::to_string);
            Ok(Payload::Agent(AgentPayload::Approve { run_id, approved, reason }))
        }
        Operation::Agent(AgentOperation::List) => Ok(Payload::Agent(AgentPayload::ListResult { agents: vec![] })),
        Operation::Agent(AgentOperation::Event) => {
            let obj = value.as_object().ok_or(ProtocolError::PayloadDecodeFailed("agent.event"))?;
            let run_id = obj
                .get("run_id")
                .and_then(|v| v.as_str())
                .ok_or(ProtocolError::PayloadDecodeFailed("agent.event"))?
                .to_string();
            let event = obj
                .get("event")
                .cloned()
                .ok_or(ProtocolError::PayloadDecodeFailed("agent.event"))?;
            Ok(Payload::Agent(AgentPayload::Event { run_id, event }))
        }
        Operation::File(FileOperation::List) => {
            let obj = value.as_object().ok_or(ProtocolError::PayloadDecodeFailed("file.list"))?;
            let path = obj
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or(ProtocolError::PayloadDecodeFailed("file.list"))?
                .to_string();
            Ok(Payload::File(FilePayload::List { path }))
        }
        Operation::File(FileOperation::Read) => {
            let obj = value.as_object().ok_or(ProtocolError::PayloadDecodeFailed("file.read"))?;
            let path = obj
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or(ProtocolError::PayloadDecodeFailed("file.read"))?
                .to_string();
            Ok(Payload::File(FilePayload::Read { path }))
        }
        Operation::Session(SessionOperation::List) => Ok(Payload::Session(SessionPayload::List)),
        Operation::Session(SessionOperation::Resume) => {
            let obj = value.as_object().ok_or(ProtocolError::PayloadDecodeFailed("session.resume"))?;
            let session_id = obj
                .get("session_id")
                .and_then(|v| v.as_str())
                .ok_or(ProtocolError::PayloadDecodeFailed("session.resume"))?
                .to_string();
            let agent_id = obj.get("agent_id").and_then(|v| v.as_str()).map(ToString::to_string);
            Ok(Payload::Session(SessionPayload::Resume { session_id, agent_id }))
        }
        Operation::Session(SessionOperation::Entries) => {
            let obj = value.as_object().ok_or(ProtocolError::PayloadDecodeFailed("session.entries"))?;
            let session_id = obj
                .get("session_id")
                .and_then(|v| v.as_str())
                .ok_or(ProtocolError::PayloadDecodeFailed("session.entries"))?
                .to_string();
            let agent_id = obj.get("agent_id").and_then(|v| v.as_str()).map(ToString::to_string);
            Ok(Payload::Session(SessionPayload::Entries { session_id, agent_id }))
        }
        Operation::Mcp(McpOperation::ListServers) => Ok(Payload::Mcp(McpPayload::ListServers)),
        Operation::Mcp(McpOperation::ListTools) => {
            let server = value.as_object().and_then(|o| o.get("server")).and_then(|v| v.as_str()).map(ToString::to_string);
            Ok(Payload::Mcp(McpPayload::ListTools { server }))
        }
        Operation::Mcp(McpOperation::CallTool) => {
            let obj = value.as_object().ok_or(ProtocolError::PayloadDecodeFailed("mcp.call_tool"))?;
            let server = obj.get("server").and_then(|v| v.as_str()).ok_or(ProtocolError::PayloadDecodeFailed("mcp.call_tool"))?.to_string();
            let tool_name = obj.get("tool_name").and_then(|v| v.as_str()).ok_or(ProtocolError::PayloadDecodeFailed("mcp.call_tool"))?.to_string();
            let arguments = obj.get("arguments").cloned().unwrap_or(serde_json::json!({}));
            Ok(Payload::Mcp(McpPayload::CallTool { server, tool_name, arguments }))
        }
        Operation::Mcp(McpOperation::ListResources) => {
            let server = value.as_object().and_then(|o| o.get("server")).and_then(|v| v.as_str()).map(ToString::to_string);
            Ok(Payload::Mcp(McpPayload::ListResources { server }))
        }
        Operation::Mcp(McpOperation::ListResourceTemplates) => {
            let server = value.as_object().and_then(|o| o.get("server")).and_then(|v| v.as_str()).map(ToString::to_string);
            Ok(Payload::Mcp(McpPayload::ListResourceTemplates { server }))
        }
        Operation::Mcp(McpOperation::ReadResource) => {
            let obj = value.as_object().ok_or(ProtocolError::PayloadDecodeFailed("mcp.read_resource"))?;
            let uri = obj.get("uri").and_then(|v| v.as_str()).ok_or(ProtocolError::PayloadDecodeFailed("mcp.read_resource"))?.to_string();
            Ok(Payload::Mcp(McpPayload::ReadResource { uri }))
        }
        Operation::Mcp(McpOperation::ListPrompts) => {
            let server = value.as_object().and_then(|o| o.get("server")).and_then(|v| v.as_str()).map(ToString::to_string);
            Ok(Payload::Mcp(McpPayload::ListPrompts { server }))
        }
        Operation::Mcp(McpOperation::GetPrompt) => {
            let obj = value.as_object().ok_or(ProtocolError::PayloadDecodeFailed("mcp.get_prompt"))?;
            let name = obj.get("name").and_then(|v| v.as_str()).ok_or(ProtocolError::PayloadDecodeFailed("mcp.get_prompt"))?.to_string();
            let arguments = obj.get("arguments").and_then(|v| v.as_object()).cloned();
            Ok(Payload::Mcp(McpPayload::GetPrompt { name, arguments }))
        }
        Operation::Mcp(McpOperation::Reconnect) => {
            let obj = value.as_object().ok_or(ProtocolError::PayloadDecodeFailed("mcp.reconnect"))?;
            let server = obj.get("server").and_then(|v| v.as_str()).ok_or(ProtocolError::PayloadDecodeFailed("mcp.reconnect"))?.to_string();
            Ok(Payload::Mcp(McpPayload::Reconnect { server }))
        }
        Operation::Mcp(McpOperation::ServerStatus) => {
            let server = value.as_object().and_then(|o| o.get("server")).and_then(|v| v.as_str()).map(ToString::to_string);
            Ok(Payload::Mcp(McpPayload::ServerStatus { server }))
        }
        Operation::Skill(SkillOperation::List) => Ok(Payload::Skill(SkillPayload::List)),
        Operation::Skill(SkillOperation::Get) => {
            let obj = value.as_object().ok_or(ProtocolError::PayloadDecodeFailed("skill.get"))?;
            let name = obj.get("name").and_then(|v| v.as_str()).ok_or(ProtocolError::PayloadDecodeFailed("skill.get"))?.to_string();
            Ok(Payload::Skill(SkillPayload::Get { name }))
        }
        Operation::Log(LogOperation::List) => Ok(Payload::Log(LogPayload::List)),
        Operation::Log(LogOperation::Read) => {
            let obj = value.as_object().ok_or(ProtocolError::PayloadDecodeFailed("log.read"))?;
            let run_id = obj.get("run_id").and_then(|v| v.as_str()).ok_or(ProtocolError::PayloadDecodeFailed("log.read"))?.to_string();
            Ok(Payload::Log(LogPayload::Read { run_id }))
        }
        Operation::System(SystemOperation::Connected) => Ok(Payload::System(crate::agent_server_protocol::SystemPayload::Empty)),
    }
}
