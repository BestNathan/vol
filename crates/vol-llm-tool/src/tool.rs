//! Tool trait and types.

use async_trait::async_trait;
use vol_llm_core::{ToolDefinition, Message};
use vol_core::Alert;
use std::error::Error;

/// Tool execution result
#[derive(Debug, Clone)]
pub struct ToolResult {
    pub call_id: String,
    pub success: bool,
    pub content: String,
    pub error: Option<String>,
    pub data: Option<serde_json::Value>,
}

/// Tool execution context
#[derive(Debug, Clone, Default)]
pub struct ToolContext {
    pub alert: Option<Alert>,
    pub messages: Vec<Message>,
    pub metadata: std::collections::HashMap<String, String>,
}

/// Tool trait
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Option<serde_json::Value>;

    fn to_definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: Some(self.description().to_string()),
            parameters: self.parameters(),
        }
    }

    async fn execute(&self, args: &str, context: &ToolContext)
        -> Result<ToolResult, Box<dyn Error + Send>>;
}
