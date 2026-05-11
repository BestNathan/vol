//! Tool registry.

use crate::tool::{ExecutableTool, Tool, ToolContext, ToolResult, ToolSensitivity};
use std::collections::HashMap;
use std::sync::Arc;
use vol_llm_core::{ToolCall, ToolDefinition};

/// Tool registry
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn ExecutableTool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
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

        Arc::new(Self { tools })
    }

    /// Discover and register all MCP tools from an McpSession.
    ///
    /// Iterates all connected servers, discovers their tools,
    /// creates McpTool wrappers, and registers them.
    pub async fn register_from_mcp(&mut self, session: Arc<vol_llm_mcp::McpSession>) {
        use crate::mcp_tool::McpTool;

        let tools = session.list_all_tools();
        for (server, tool_info) in tools {
            let mcp_tool = McpTool::new(
                session.clone(),
                &server,
                &tool_info.name,
                tool_info.description.as_deref().unwrap_or("MCP tool"),
                tool_info.input_schema.unwrap_or_else(|| {
                    serde_json::json!({ "type": "object", "properties": {} })
                }),
            );
            self.register_boxed(Box::new(mcp_tool));
        }
    }
}

impl Clone for ToolRegistry {
    fn clone(&self) -> Self {
        Self {
            tools: self.tools.clone(),
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
}
