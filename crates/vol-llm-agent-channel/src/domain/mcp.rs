use std::sync::Arc;

use async_trait::async_trait;
use vol_llm_mcp::manager::McpManager;

use crate::agent_server_protocol::{
    AgentServerMessage, McpOperation, McpPayload, Operation, Payload, ProtocolError,
};
use crate::domain::handler::DomainHandler;

/// Handler for MCP-domain operations.
pub struct McpHandler {
    mcp_manager: Option<Arc<McpManager>>,
}

impl McpHandler {
    pub fn new(mcp_manager: Option<Arc<McpManager>>) -> Self {
        Self { mcp_manager }
    }

    fn mgr(&self) -> Result<&Arc<McpManager>, ProtocolError> {
        self.mcp_manager
            .as_ref()
            .ok_or(ProtocolError::PayloadDecodeFailed("mcp not configured"))
    }

    fn server_status_to_str(status: &vol_llm_mcp::manager::ServerStatus) -> String {
        match status {
            vol_llm_mcp::manager::ServerStatus::Connected => "connected".into(),
            vol_llm_mcp::manager::ServerStatus::Disconnected => "disconnected".into(),
            vol_llm_mcp::manager::ServerStatus::Connecting => "connecting".into(),
            vol_llm_mcp::manager::ServerStatus::Error(e) => format!("error: {e}"),
        }
    }
}

#[async_trait]
impl DomainHandler for McpHandler {
    fn name(&self) -> &str {
        "mcp"
    }

    fn operations(&self) -> Vec<Operation> {
        vec![
            Operation::Mcp(McpOperation::ListServers),
            Operation::Mcp(McpOperation::ListTools),
            Operation::Mcp(McpOperation::CallTool),
            Operation::Mcp(McpOperation::ListResources),
            Operation::Mcp(McpOperation::ListResourceTemplates),
            Operation::Mcp(McpOperation::ReadResource),
            Operation::Mcp(McpOperation::ListPrompts),
            Operation::Mcp(McpOperation::GetPrompt),
            Operation::Mcp(McpOperation::Reconnect),
            Operation::Mcp(McpOperation::ServerStatus),
        ]
    }

    async fn handle(
        &self,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        let op = match &message.operation {
            Operation::Mcp(op) => op.clone(),
            _ => return Err(ProtocolError::PayloadDecodeFailed("mcp")),
        };
        match (op, message.payload) {
            (McpOperation::ListServers, Payload::Mcp(McpPayload::ListServers)) => {
                let mgr = self.mgr()?;
                let status = mgr.server_status_async().await;
                let servers: Vec<serde_json::Value> = status.iter().map(|(name, s)| {
                    serde_json::json!({ "name": name, "status": Self::server_status_to_str(s) })
                }).collect();
                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Mcp(McpOperation::ListServers),
                    Payload::Mcp(McpPayload::ListServersResult { servers }),
                )])
            }
            (McpOperation::ListTools, Payload::Mcp(McpPayload::ListTools { server })) => {
                let mgr = self.mgr()?;
                let tools = mgr.list_all_tools().await;
                let tools_json: Vec<serde_json::Value> = tools.iter()
                    .filter(|(s, _)| server.as_ref().map_or(true, |f| s == f))
                    .map(|(s, t)| {
                        serde_json::json!({
                            "server": s, "name": t.name, "description": t.description, "input_schema": t.input_schema,
                        })
                    }).collect();
                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Mcp(McpOperation::ListTools),
                    Payload::Mcp(McpPayload::ListToolsResult { tools: tools_json }),
                )])
            }
            (
                McpOperation::CallTool,
                Payload::Mcp(McpPayload::CallTool {
                    server,
                    tool_name,
                    arguments,
                }),
            ) => {
                let mgr = self.mgr()?;
                match mgr.call_tool(&server, &tool_name, arguments).await {
                    Ok(result) => Ok(vec![AgentServerMessage::new_result(
                        message.message_id,
                        Operation::Mcp(McpOperation::CallTool),
                        Payload::Mcp(McpPayload::CallToolResult {
                            tool_name,
                            result: serde_json::json!(result),
                        }),
                    )]),
                    Err(e) => Ok(vec![AgentServerMessage::new_error(
                        message.message_id,
                        Operation::Mcp(McpOperation::CallTool),
                        crate::agent_server_protocol::ErrorPayload {
                            code: "mcp_call_failed".to_string(),
                            message: e.to_string(),
                            detail: None,
                            terminal: true,
                        },
                    )]),
                }
            }
            (McpOperation::ListResources, Payload::Mcp(McpPayload::ListResources { server })) => {
                let mgr = self.mgr()?;
                let resources = mgr.list_all_resources().await;
                let r_json: Vec<serde_json::Value> = resources.iter()
                    .filter(|(s, _)| server.as_ref().map_or(true, |f| s == f))
                    .map(|(s, r)| {
                        serde_json::json!({ "server": s, "name": r.name, "uri": r.uri, "mime_type": r.mime_type, "description": r.description })
                    }).collect();
                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Mcp(McpOperation::ListResources),
                    Payload::Mcp(McpPayload::ListResourcesResult { resources: r_json }),
                )])
            }
            (
                McpOperation::ListResourceTemplates,
                Payload::Mcp(McpPayload::ListResourceTemplates { server }),
            ) => {
                let mgr = self.mgr()?;
                let templates = mgr.list_all_resource_templates().await;
                let t_json: Vec<serde_json::Value> = templates.iter()
                    .filter(|(s, _)| server.as_ref().map_or(true, |f| s == f))
                    .map(|(s, t)| {
                        serde_json::json!({ "server": s, "name": t.name, "uri_template": t.uri_template, "description": t.description })
                    }).collect();
                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Mcp(McpOperation::ListResourceTemplates),
                    Payload::Mcp(McpPayload::ListResourceTemplatesResult { templates: t_json }),
                )])
            }
            (McpOperation::ReadResource, Payload::Mcp(McpPayload::ReadResource { uri })) => {
                let mgr = self.mgr()?;
                match mgr.read_resource(&uri).await {
                    Ok(content) => Ok(vec![AgentServerMessage::new_result(
                        message.message_id,
                        Operation::Mcp(McpOperation::ReadResource),
                        Payload::Mcp(McpPayload::ReadResourceResult { uri, content }),
                    )]),
                    Err(e) => Ok(vec![AgentServerMessage::new_error(
                        message.message_id,
                        Operation::Mcp(McpOperation::ReadResource),
                        crate::agent_server_protocol::ErrorPayload {
                            code: "mcp_read_resource_failed".to_string(),
                            message: e.to_string(),
                            detail: None,
                            terminal: true,
                        },
                    )]),
                }
            }
            (McpOperation::ListPrompts, Payload::Mcp(McpPayload::ListPrompts { server })) => {
                let mgr = self.mgr()?;
                let prompts = mgr.list_all_prompts().await;
                let p_json: Vec<serde_json::Value> = prompts.iter()
                    .filter(|(s, _)| server.as_ref().map_or(true, |f| s == f))
                    .map(|(s, p)| {
                        let args = p.arguments.as_ref().map(|args| {
                            args.iter().map(|a| {
                                serde_json::json!({ "name": a.name, "description": a.description, "required": a.required })
                            }).collect::<Vec<_>>()
                        });
                        serde_json::json!({ "server": s, "name": p.name, "description": p.description, "arguments": args })
                    }).collect();
                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Mcp(McpOperation::ListPrompts),
                    Payload::Mcp(McpPayload::ListPromptsResult { prompts: p_json }),
                )])
            }
            (McpOperation::GetPrompt, Payload::Mcp(McpPayload::GetPrompt { name, arguments })) => {
                let mgr = self.mgr()?;
                match mgr
                    .get_prompt(&name, arguments.map(|m| m.into_iter().collect()))
                    .await
                {
                    Ok((desc, messages)) => {
                        let msgs = messages
                            .iter()
                            .map(|m| {
                                let content = serde_json::to_string(&m.content).unwrap_or_default();
                                let role = format!("{:?}", m.role);
                                serde_json::json!({ "role": role, "content": content })
                            })
                            .collect::<Vec<_>>();
                        Ok(vec![AgentServerMessage::new_result(
                            message.message_id,
                            Operation::Mcp(McpOperation::GetPrompt),
                            Payload::Mcp(McpPayload::GetPromptResult {
                                name,
                                prompt: serde_json::json!({ "description": desc, "messages": msgs }),
                            }),
                        )])
                    }
                    Err(e) => Ok(vec![AgentServerMessage::new_error(
                        message.message_id,
                        Operation::Mcp(McpOperation::GetPrompt),
                        crate::agent_server_protocol::ErrorPayload {
                            code: "mcp_get_prompt_failed".to_string(),
                            message: e.to_string(),
                            detail: None,
                            terminal: true,
                        },
                    )]),
                }
            }
            (McpOperation::Reconnect, Payload::Mcp(McpPayload::Reconnect { server })) => {
                let mgr = self.mgr()?;
                match mgr.reconnect(&server).await {
                    Ok(()) => {
                        let status = mgr.server_status_async().await;
                        let status_str = status
                            .get(&server)
                            .map(Self::server_status_to_str)
                            .unwrap_or_else(|| "unknown".into());
                        Ok(vec![AgentServerMessage::new_result(
                            message.message_id,
                            Operation::Mcp(McpOperation::Reconnect),
                            Payload::Mcp(McpPayload::ReconnectResult { reconnected: true }),
                        )])
                    }
                    Err(e) => Ok(vec![AgentServerMessage::new_error(
                        message.message_id,
                        Operation::Mcp(McpOperation::Reconnect),
                        crate::agent_server_protocol::ErrorPayload {
                            code: "mcp_reconnect_failed".to_string(),
                            message: e.to_string(),
                            detail: None,
                            terminal: true,
                        },
                    )]),
                }
            }
            (McpOperation::ServerStatus, Payload::Mcp(McpPayload::ServerStatus { server: _ })) => {
                let mgr = self.mgr()?;
                let status = mgr.server_status_async().await;
                let servers: Vec<serde_json::Value> = status.iter().map(|(name, s)| {
                    serde_json::json!({ "name": name, "status": Self::server_status_to_str(s) })
                }).collect();
                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Mcp(McpOperation::ServerStatus),
                    Payload::Mcp(McpPayload::ServerStatusResult {
                        server: "all".to_string(),
                        status: format!("{} servers", servers.len()),
                    }),
                )])
            }
            (McpOperation::ListServers, _) => {
                Err(ProtocolError::PayloadDecodeFailed("mcp.list_servers"))
            }
            (McpOperation::ListTools, _) => {
                Err(ProtocolError::PayloadDecodeFailed("mcp.list_tools"))
            }
            (McpOperation::CallTool, _) => Err(ProtocolError::PayloadDecodeFailed("mcp.call_tool")),
            (McpOperation::ListResources, _) => {
                Err(ProtocolError::PayloadDecodeFailed("mcp.list_resources"))
            }
            (McpOperation::ListResourceTemplates, _) => Err(ProtocolError::PayloadDecodeFailed(
                "mcp.list_resource_templates",
            )),
            (McpOperation::ReadResource, _) => {
                Err(ProtocolError::PayloadDecodeFailed("mcp.read_resource"))
            }
            (McpOperation::ListPrompts, _) => {
                Err(ProtocolError::PayloadDecodeFailed("mcp.list_prompts"))
            }
            (McpOperation::GetPrompt, _) => {
                Err(ProtocolError::PayloadDecodeFailed("mcp.get_prompt"))
            }
            (McpOperation::Reconnect, _) => {
                Err(ProtocolError::PayloadDecodeFailed("mcp.reconnect"))
            }
            (McpOperation::ServerStatus, _) => {
                Err(ProtocolError::PayloadDecodeFailed("mcp.server_status"))
            }
        }
    }
}
