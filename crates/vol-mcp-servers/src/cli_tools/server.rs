use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content, ListToolsResult, PaginatedRequestParams,
    Tool,
};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{ErrorData, ServerHandler};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use vol_llm_cli_tool::CliTool;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandParams {
    /// CLI command to run. First token must match one of the tool's declared binaries.
    pub command: String,
}

#[derive(Clone)]
struct ToolEntry {
    config_name: String,
    description: String,
    cli_tool: Arc<CliTool>,
}

#[derive(Clone)]
pub struct CliToolsMcpServer {
    tools: Vec<ToolEntry>,
}

impl CliToolsMcpServer {
    pub async fn load(
        cli_tools_dir: &std::path::Path,
        sandbox_registry: &vol_llm_sandbox::registry::SandboxRegistry,
    ) -> Result<Self, String> {
        let raw_tools = vol_llm_cli_tool::load_dir(cli_tools_dir, sandbox_registry)
            .await
            .map_err(|e| e.to_string())?;

        let tools = raw_tools
            .into_iter()
            .map(|t| {
                let (cfg, sandbox) = t.into_parts();
                ToolEntry {
                    description: cfg.description.clone(),
                    config_name: cfg.name.clone(),
                    cli_tool: Arc::new(CliTool::new(cfg, sandbox)),
                }
            })
            .collect();

        Ok(Self { tools })
    }
}

impl ServerHandler for CliToolsMcpServer {
    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, ErrorData>> + Send + '_ {
        async move {
            let tools: Vec<Tool> = self
                .tools
                .iter()
                .map(|t| {
                    Tool::new(
                        t.config_name.clone(),
                        t.description.clone(),
                        serde_json::json!({
                            "type": "object",
                            "properties": {
                                "command": {
                                    "type": "string",
                                    "description": "CLI command to run. First token must match one of the tool's declared binaries."
                                }
                            },
                            "required": ["command"]
                        })
                        .as_object()
                        .cloned()
                        .unwrap_or_default(),
                    )
                })
                .collect();
            Ok(ListToolsResult::with_all_items(tools))
        }
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, ErrorData>> + Send + '_ {
        async move {
            let tool_name = request.name.to_string();
            let params: CommandParams = serde_json::from_value(
                serde_json::to_value(&request.arguments).unwrap_or_default(),
            )
            .unwrap_or(CommandParams {
                command: String::new(),
            });

            let entry = self
                .tools
                .iter()
                .find(|t| t.config_name == tool_name)
                .ok_or_else(|| {
                    ErrorData::invalid_request(format!("unknown cli-tool: {tool_name}"), None)
                })?;

            let output = entry
                .cli_tool
                .run(&params.command)
                .await
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

            if output.success {
                Ok(CallToolResult::success(vec![Content::text(output.content)]))
            } else {
                Ok(CallToolResult::error(vec![Content::text(output.content)]))
            }
        }
    }
}
