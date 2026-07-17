//! McpTool — bridges MCP tools into the ExecutableTool trait.

use std::sync::Arc;

use async_trait::async_trait;
use vol_llm_mcp::McpManager;

use crate::tool::{
    ExecutableTool, ToolContext, ToolError, ToolResult, ToolResultType, ToolSensitivity,
};

/// A tool that proxies execution to an MCP server via McpManager.
pub struct McpTool {
    manager: Arc<McpManager>,
    server_name: String,
    tool_name: String,
    display_name: &'static str,
    description: &'static str,
    parameters: serde_json::Value,
}

impl McpTool {
    /// Create a new McpTool from a manager and tool info.
    pub fn new(
        manager: Arc<McpManager>,
        server_name: &str,
        tool_name: &str,
        description: &str,
        parameters: serde_json::Value,
    ) -> Self {
        let sanitized = vol_llm_mcp::session::sanitize_name(server_name);
        let sanitized_tool = vol_llm_mcp::session::sanitize_name(tool_name);
        let display_name = format!("mcp__{sanitized}_{sanitized_tool}");

        // Leak strings to satisfy ExecutableTool::name() -> &'static str
        // Acceptable because tools are registered once at startup.
        let display_name: &'static str = Box::leak(display_name.into_boxed_str());
        let description: &'static str = Box::leak(description.to_string().into_boxed_str());

        Self {
            manager,
            server_name: sanitized,
            tool_name: sanitized_tool,
            display_name,
            description,
            parameters,
        }
    }
}

#[async_trait]
impl ExecutableTool for McpTool {
    fn name(&self) -> &'static str {
        self.display_name
    }

    fn description(&self) -> &'static str {
        self.description
    }

    fn parameters(&self) -> serde_json::Value {
        self.parameters.clone()
    }

    fn sensitivity(&self, _args: &serde_json::Value) -> ToolSensitivity {
        ToolSensitivity::Safe
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _context: &ToolContext,
    ) -> ToolResultType<ToolResult> {
        let result = self
            .manager
            .call_tool(&self.server_name, &self.tool_name, args.clone())
            .await;

        match result {
            Ok(content) => Ok(ToolResult::success(content)),
            Err(e) => Err(ToolError::ExecutionFailed(e.to_string())),
        }
    }
}
