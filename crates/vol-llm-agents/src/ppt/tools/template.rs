//! 模板匹配工具。

use vol_llm_tool::{ExecutableTool, ToolContext, ToolResult, ToolError};
use serde_json::Value;
use async_trait::async_trait;

/// 模板匹配工具
pub struct TemplateMatcherTool;

impl TemplateMatcherTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TemplateMatcherTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExecutableTool for TemplateMatcherTool {
    fn name(&self) -> &'static str {
        "match_template"
    }

    fn description(&self) -> &'static str {
        "Match the best template based on presentation topic."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "topic": {
                    "type": "string",
                    "description": "Presentation topic"
                },
                "style_preference": {
                    "type": "string",
                    "description": "Preferred style"
                }
            },
            "required": ["topic"]
        })
    }

    async fn execute(&self, _args: &Value, _context: &ToolContext) -> Result<ToolResult, ToolError> {
        // TODO: Implement template matching
        Ok(ToolResult {
            call_id: String::new(),
            success: true,
            content: "Template matching not yet implemented".to_string(),
            error: None,
            data: None,
        })
    }
}
