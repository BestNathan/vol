//! Tool registry.

use crate::tool::{ExecutableTool, Tool, ToolContext, ToolResult, ToolSensitivity};
use std::collections::HashMap;
use vol_llm_core::{ToolCall, ToolDefinition};

/// Tool registry
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn ExecutableTool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register<T: ExecutableTool + 'static>(&mut self, tool: T) {
        self.tools.insert(tool.name().to_string(), Box::new(tool));
    }

    /// Register a boxed tool
    pub fn register_boxed(&mut self, tool: Box<dyn ExecutableTool>) {
        self.tools.insert(tool.name().to_string(), tool);
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
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
