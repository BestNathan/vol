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
        let display_name = format!("mcp__{sanitized}__{sanitized_tool}");

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

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_manager() -> Arc<McpManager> {
        Arc::new(McpManager::new(vec![]))
    }

    #[test]
    fn test_mcp_tool_name_format() {
        let tool = McpTool::new(
            empty_manager(),
            "docs.rs",
            "search_crates",
            "Search crates on crates.io",
            serde_json::json!({"type": "object"}),
        );
        // Name should follow the mcp__{server}__{tool} convention with sanitization
        assert_eq!(tool.name(), "mcp__docs_rs__search_crates");
    }

    #[test]
    fn test_mcp_tool_description_is_preserved() {
        let tool = McpTool::new(
            empty_manager(),
            "weather",
            "forecast",
            "Get weather forecast",
            serde_json::json!({"type": "object"}),
        );
        assert_eq!(tool.description(), "Get weather forecast");
    }

    #[test]
    fn test_mcp_tool_parameters_are_preserved() {
        let params = serde_json::json!({
            "type": "object",
            "properties": {
                "city": {"type": "string"}
            },
            "required": ["city"]
        });
        let tool = McpTool::new(
            empty_manager(),
            "weather",
            "forecast",
            "desc",
            params.clone(),
        );
        assert_eq!(tool.parameters(), params);
    }

    #[test]
    fn test_mcp_tool_sensitivity_is_safe() {
        let tool = McpTool::new(empty_manager(), "srv", "t", "d", serde_json::json!({}));
        match tool.sensitivity(&serde_json::json!({})) {
            ToolSensitivity::Safe => {}
            _ => panic!("McpTool should be Safe"),
        }
    }

    #[tokio::test]
    async fn test_mcp_tool_execute_propagates_manager_error() {
        let manager = empty_manager();
        // Don't connect — call_tool on a non-existent server will fail
        let tool = McpTool::new(
            manager,
            "nonexistent_server",
            "nonexistent_tool",
            "desc",
            serde_json::json!({}),
        );
        let result = tool
            .execute(&serde_json::json!({}), &ToolContext::for_test())
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Execution failed")
                || err.contains("not found")
                || err.contains("not connected")
        );
    }

    #[test]
    fn test_mcp_tool_name_sanitizes_special_chars() {
        // Names with dots/special chars (not alphanumeric, underscore, or hyphen)
        // are sanitized: special chars become underscores.
        let tool = McpTool::new(
            empty_manager(),
            "docs.rs",  // '.' → '_'
            "get item", // ' ' → '_'
            "desc",
            serde_json::json!({}),
        );
        let name = tool.name();
        // Dots are replaced with underscores
        assert!(!name.contains('.'));
        // Spaces are replaced with underscores
        assert!(!name.contains(' '));
        // Should match mcp__<sanitized_server>__<sanitized_tool>
        assert!(name.starts_with("mcp__"));
    }
}
