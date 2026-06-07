//! Tool registry.

use crate::tool::{ExecutableTool, Tool, ToolContext, ToolResult, ToolSensitivity};
use std::collections::HashMap;
use std::sync::Arc;
use vol_llm_core::{ToolCall, ToolDefinition};

/// Tool registry
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn ExecutableTool>>,
    tool_sandboxes: HashMap<String, String>,  // tool_name → sandbox_name
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            tool_sandboxes: HashMap::new(),
        }
    }

    pub fn set_tool_sandbox(&mut self, tool_name: &str, sandbox_name: &str) {
        self.tool_sandboxes.insert(tool_name.to_string(), sandbox_name.to_string());
    }

    pub fn get_tool_sandbox(&self, tool_name: &str) -> Option<&str> {
        self.tool_sandboxes.get(tool_name).map(|s| s.as_str())
    }

    pub fn register<T: ExecutableTool + 'static>(&mut self, tool: T) {
        self.tools.insert(tool.name().to_string(), Arc::new(tool));
    }

    /// Register a boxed tool
    pub fn register_boxed(&mut self, tool: Box<dyn ExecutableTool>) {
        self.tools.insert(tool.name().to_string(), Arc::from(tool));
    }

    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|t| t.to_definition()).collect()
    }

    pub async fn execute(
        &self,
        call: &ToolCall,
        context: &ToolContext,
    ) -> Result<ToolResult, String> {
        let tool = self
            .tools
            .get(&call.name)
            .ok_or_else(|| format!("Unknown tool: {}", call.name))?;

        let args: serde_json::Value =
            serde_json::from_str(&call.arguments).map_err(|e| {
                format!("Invalid JSON arguments for {}: {}", call.name, e)
            })?;

        let result = tool
            .execute(&args, context)
            .await
            .map_err(|e| e.to_string())?;

        Ok(ToolResult {
            call_id: call.id.clone(),
            ..result
        })
    }

    pub fn tool_names(&self) -> Vec<&str> {
        self.tools.keys().map(|s| s.as_str()).collect()
    }

    /// Get the sensitivity level of a tool for the given arguments.
    /// Returns Safe if the tool is not found (fails open for registry lookup).
    pub fn tool_sensitivity(&self, name: &str, args: &serde_json::Value) -> ToolSensitivity {
        self.tools
            .get(name)
            .map(|t| t.sensitivity(args))
            .unwrap_or(ToolSensitivity::Safe)
    }

    /// Create a filtered registry containing only the allowed tools,
    /// minus any disallowed tools.
    pub fn filter(
        &self,
        allowed: Option<&[&str]>,
        disallowed: Option<&[&str]>,
    ) -> Arc<Self> {
        let disallowed_set: std::collections::HashSet<&str> = disallowed
            .map(|d| d.iter().copied().collect())
            .unwrap_or_default();

        let tools = match allowed {
            None => self
                .tools
                .iter()
                .filter(|(name, _)| !disallowed_set.contains(name.as_str()))
                .map(|(name, tool)| (name.clone(), Arc::clone(tool)))
                .collect(),
            Some(allow_list) => {
                let allowed_set: std::collections::HashSet<&str> = allow_list.iter().copied().collect();
                self.tools
                    .iter()
                    .filter(|(name, _)| {
                        allowed_set.contains(name.as_str())
                            && !disallowed_set.contains(name.as_str())
                    })
                    .map(|(name, tool)| (name.clone(), Arc::clone(tool)))
                    .collect()
            }
        };

        Arc::new(Self { tools, tool_sandboxes: self.tool_sandboxes.clone() })
    }

    /// Discover and register all MCP tools from an McpManager.
    ///
    /// Queries the manager for tools from all connected servers,
    /// creates McpTool wrappers, and registers them.
    /// Returns the number of tools registered.
    pub async fn register_from_mcp(&mut self, manager: Arc<vol_llm_mcp::McpManager>) -> usize {
        use crate::mcp_tool::McpTool;

        let tools = manager.list_all_tools().await;
        let mut count = 0;
        for (server, tool_info) in tools {
            let description = tool_info.description.as_deref().unwrap_or_else(|| {
                // Derive a minimal description from the tool name when absent.
                &tool_info.name
            });
            let mcp_tool = McpTool::new(
                manager.clone(),
                &server,
                &tool_info.name,
                description,
                tool_info.input_schema.unwrap_or_else(|| {
                    serde_json::json!({ "type": "object", "properties": {} })
                }),
            );
            self.register_boxed(Box::new(mcp_tool));
            count += 1;
        }
        count
    }
}

impl Clone for ToolRegistry {
    fn clone(&self) -> Self {
        Self {
            tools: self.tools.clone(),
            tool_sandboxes: self.tool_sandboxes.clone(),
        }
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use crate::tool::{ExecutableTool, ToolResultType, ToolSensitivity};

    struct DummyTool {
        name: &'static str,
    }
    impl DummyTool {
        fn new(name: &'static str) -> Self {
            Self { name }
        }
    }
    #[async_trait]
    impl ExecutableTool for DummyTool {
        fn name(&self) -> &'static str { self.name }
        fn description(&self) -> &'static str { "dummy" }
        fn parameters(&self) -> serde_json::Value { serde_json::json!({}) }
        fn sensitivity(&self, _args: &serde_json::Value) -> ToolSensitivity { ToolSensitivity::Safe }
        async fn execute(&self, _args: &serde_json::Value, _context: &ToolContext) -> ToolResultType<ToolResult> {
            Ok(ToolResult::success("ok"))
        }
    }

    #[test]
    fn test_filter_no_filters_keeps_all() {
        let mut registry = ToolRegistry::new();
        registry.register(DummyTool::new("tool_a"));
        registry.register(DummyTool::new("tool_b"));
        let filtered = registry.filter(None, None);
        assert_eq!(filtered.tool_names().len(), 2);
    }

    #[test]
    fn test_filter_allowed_keeps_only_allowed() {
        let mut registry = ToolRegistry::new();
        registry.register(DummyTool::new("tool_a"));
        registry.register(DummyTool::new("tool_b"));
        let filtered = registry.filter(Some(&["tool_a"]), None);
        assert_eq!(filtered.tool_names().len(), 1);
        assert!(filtered.tool_names().contains(&"tool_a"));
    }

    #[test]
    fn test_filter_disallowed_removes() {
        let mut registry = ToolRegistry::new();
        registry.register(DummyTool::new("tool_a"));
        registry.register(DummyTool::new("tool_b"));
        let filtered = registry.filter(None, Some(&["tool_a"]));
        assert_eq!(filtered.tool_names().len(), 1);
        assert!(filtered.tool_names().contains(&"tool_b"));
    }

    #[test]
    fn test_filter_allowed_and_disallowed() {
        let mut registry = ToolRegistry::new();
        registry.register(DummyTool::new("tool_a"));
        registry.register(DummyTool::new("tool_b"));
        registry.register(DummyTool::new("tool_c"));
        let filtered = registry.filter(Some(&["tool_a", "tool_b"]), Some(&["tool_b"]));
        assert_eq!(filtered.tool_names().len(), 1);
        assert!(filtered.tool_names().contains(&"tool_a"));
    }

    #[test]
    fn test_filter_unknown_tool_silently_ignored() {
        let mut registry = ToolRegistry::new();
        registry.register(DummyTool::new("tool_a"));
        let filtered = registry.filter(Some(&["tool_a", "nonexistent"]), None);
        assert_eq!(filtered.tool_names().len(), 1);
        assert!(filtered.tool_names().contains(&"tool_a"));
    }

    #[tokio::test]
    async fn test_register_from_mcp_empty_manager() {
        use vol_llm_mcp::McpManager;

        let manager = Arc::new(McpManager::new(vec![]));
        manager.connect().await.unwrap();
        let mut registry = ToolRegistry::new();
        let count = registry.register_from_mcp(manager).await;
        assert_eq!(count, 0);
        assert!(registry.tool_names().is_empty());
    }
}
