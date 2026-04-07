//! Tool registry.

use std::collections::HashMap;
use vol_llm_core::{ToolDefinition, ToolCall};
use crate::tool::{Tool, ToolResult, ToolContext};
use crate::tools::{AlertHistoryTool, IvCurveTool, MarketDataTool, RuleInfoTool};

/// Tool registry
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self { tools: HashMap::new() }
    }

    pub fn register<T: Tool + 'static>(&mut self, tool: T) {
        self.tools.insert(tool.name().to_string(), Box::new(tool));
    }

    /// Register a boxed tool
    pub fn register_boxed(&mut self, tool: Box<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    /// Register all default tools
    pub fn register_default_tools(&mut self) {
        self.register(AlertHistoryTool::new(None));
        self.register(IvCurveTool::new(None));
        self.register(MarketDataTool::new(None));
        self.register(RuleInfoTool::new(None));
    }

    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|t| t.to_definition()).collect()
    }

    pub async fn execute(
        &self,
        call: &ToolCall,
        context: &ToolContext,
    ) -> Result<ToolResult, String> {
        let tool = self.tools.get(&call.name)
            .ok_or_else(|| format!("Unknown tool: {}", call.name))?;

        let result = tool.execute(&call.arguments, context).await
            .map_err(|e| e.to_string())?;

        Ok(ToolResult {
            call_id: call.id.clone(),
            ..result
        })
    }

    pub fn tool_names(&self) -> Vec<&str> {
        self.tools.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
