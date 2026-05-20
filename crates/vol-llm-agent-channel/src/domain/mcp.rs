use crate::agent_server_protocol::{
    AgentServerMessage, McpOperation, McpPayload, Operation, Payload, ProtocolError,
};

/// Handler for MCP-domain operations.
pub struct McpHandler;

impl McpHandler {
    pub async fn handle(
        &self,
        operation: McpOperation,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        match (operation, message.payload) {
            (McpOperation::ListServers, Payload::Mcp(McpPayload::ListServers)) => Ok(vec![
                AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Mcp(McpOperation::ListServers),
                    Payload::Mcp(McpPayload::ListServersResult { servers: vec![] }),
                ),
            ]),
            (McpOperation::ListTools, Payload::Mcp(McpPayload::ListTools { .. })) => Ok(vec![
                AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Mcp(McpOperation::ListTools),
                    Payload::Mcp(McpPayload::ListToolsResult { tools: vec![] }),
                ),
            ]),
            (McpOperation::CallTool, Payload::Mcp(McpPayload::CallTool { tool_name, .. })) => Ok(vec![
                AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Mcp(McpOperation::CallTool),
                    Payload::Mcp(McpPayload::CallToolResult {
                        tool_name,
                        result: serde_json::Value::Null,
                    }),
                ),
            ]),
            (McpOperation::ListResources, Payload::Mcp(McpPayload::ListResources { .. })) => Ok(vec![
                AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Mcp(McpOperation::ListResources),
                    Payload::Mcp(McpPayload::ListResourcesResult { resources: vec![] }),
                ),
            ]),
            (McpOperation::ListResourceTemplates, Payload::Mcp(McpPayload::ListResourceTemplates { .. })) => Ok(vec![
                AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Mcp(McpOperation::ListResourceTemplates),
                    Payload::Mcp(McpPayload::ListResourceTemplatesResult { templates: vec![] }),
                ),
            ]),
            (McpOperation::ReadResource, Payload::Mcp(McpPayload::ReadResource { uri })) => Ok(vec![
                AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Mcp(McpOperation::ReadResource),
                    Payload::Mcp(McpPayload::ReadResourceResult {
                        uri,
                        content: String::new(),
                    }),
                ),
            ]),
            (McpOperation::ListPrompts, Payload::Mcp(McpPayload::ListPrompts { .. })) => Ok(vec![
                AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Mcp(McpOperation::ListPrompts),
                    Payload::Mcp(McpPayload::ListPromptsResult { prompts: vec![] }),
                ),
            ]),
            (McpOperation::GetPrompt, Payload::Mcp(McpPayload::GetPrompt { name, .. })) => Ok(vec![
                AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Mcp(McpOperation::GetPrompt),
                    Payload::Mcp(McpPayload::GetPromptResult {
                        name,
                        prompt: serde_json::Value::Null,
                    }),
                ),
            ]),
            (McpOperation::Reconnect, Payload::Mcp(McpPayload::Reconnect { .. })) => Ok(vec![
                AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Mcp(McpOperation::Reconnect),
                    Payload::Mcp(McpPayload::ReconnectResult { reconnected: true }),
                ),
            ]),
            (McpOperation::ServerStatus, Payload::Mcp(McpPayload::ServerStatus { server })) => Ok(vec![
                AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Mcp(McpOperation::ServerStatus),
                    Payload::Mcp(McpPayload::ServerStatusResult {
                        server: server.unwrap_or_else(|| "unknown".to_string()),
                        status: "unknown".to_string(),
                    }),
                ),
            ]),
            (McpOperation::ListServers, _) => Err(ProtocolError::PayloadDecodeFailed("mcp.list_servers")),
            (McpOperation::ListTools, _) => Err(ProtocolError::PayloadDecodeFailed("mcp.list_tools")),
            (McpOperation::CallTool, _) => Err(ProtocolError::PayloadDecodeFailed("mcp.call_tool")),
            (McpOperation::ListResources, _) => Err(ProtocolError::PayloadDecodeFailed("mcp.list_resources")),
            (McpOperation::ListResourceTemplates, _) => Err(ProtocolError::PayloadDecodeFailed("mcp.list_resource_templates")),
            (McpOperation::ReadResource, _) => Err(ProtocolError::PayloadDecodeFailed("mcp.read_resource")),
            (McpOperation::ListPrompts, _) => Err(ProtocolError::PayloadDecodeFailed("mcp.list_prompts")),
            (McpOperation::GetPrompt, _) => Err(ProtocolError::PayloadDecodeFailed("mcp.get_prompt")),
            (McpOperation::Reconnect, _) => Err(ProtocolError::PayloadDecodeFailed("mcp.reconnect")),
            (McpOperation::ServerStatus, _) => Err(ProtocolError::PayloadDecodeFailed("mcp.server_status")),
        }
    }
}
