//! Rule info tool.

use async_trait::async_trait;
use serde::Deserialize;
use std::error::Error;
use crate::tool::{Tool, ToolResult, ToolContext};

#[derive(Debug, Deserialize)]
struct RuleInfoArgs {
    alert_type: String,
}

/// Rule info tool
pub struct RuleInfoTool;

#[async_trait]
impl Tool for RuleInfoTool {
    fn name(&self) -> &str {
        "rule_info"
    }

    fn description(&self) -> &str {
        "查询告警规则的详细信息，包括触发条件和阈值"
    }

    fn parameters(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({
            "type": "object",
            "properties": {
                "alert_type": {
                    "type": "string",
                    "description": "告警类型，如 'absolute_iv', 'rate_change'"
                }
            },
            "required": ["alert_type"]
        }))
    }

    async fn execute(&self, args: &str, _context: &ToolContext)
        -> Result<ToolResult, Box<dyn Error + Send>> {

        let args: RuleInfoArgs = serde_json::from_str(args)
            .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

        // Placeholder - TODO: query rule configuration
        let content = format!("查询告警规则：{}", args.alert_type);

        Ok(ToolResult {
            call_id: String::new(),
            success: true,
            content,
            error: None,
            data: Some(serde_json::json!({
                "alert_type": args.alert_type,
                "rule": {}
            })),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rule_info_tool() {
        let tool = RuleInfoTool;
        assert_eq!(tool.name(), "rule_info");
    }
}
