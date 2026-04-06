//! Alert history tool.

use async_trait::async_trait;
use serde::Deserialize;
use std::error::Error;
use crate::tool::{Tool, ToolResult, ToolContext};

/// Alert history tool
pub struct AlertHistoryTool {
    window_hours: u32,
}

impl AlertHistoryTool {
    pub fn new(window_hours: u32) -> Self {
        Self { window_hours }
    }
}

#[derive(Debug, Deserialize)]
struct AlertHistoryArgs {
    symbol: String,
    tenor: Option<String>,
    alert_type: Option<String>,
}

#[async_trait]
impl Tool for AlertHistoryTool {
    fn name(&self) -> &str {
        "alert_history"
    }

    fn description(&self) -> &str {
        "查询指定 symbol 的历史告警记录，用于分析告警频率和模式"
    }

    fn parameters(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({
            "type": "object",
            "properties": {
                "symbol": {
                    "type": "string",
                    "description": "标的符号，如 'BTC', 'ETH'"
                },
                "tenor": {
                    "type": "string",
                    "enum": ["short", "medium", "long"],
                    "description": "期限类型"
                },
                "alert_type": {
                    "type": "string",
                    "description": "告警类型（可选）"
                }
            },
            "required": ["symbol"]
        }))
    }

    async fn execute(&self, args: &str, _context: &ToolContext)
        -> Result<ToolResult, Box<dyn Error + Send>> {

        let args: AlertHistoryArgs = serde_json::from_str(args)
            .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

        // Placeholder response - TODO: integrate with TDengine
        let content = format!(
            "查询 {} 历史告警 (窗口：{} 小时)",
            args.symbol, self.window_hours
        );

        Ok(ToolResult {
            call_id: String::new(),
            success: true,
            content,
            error: None,
            data: Some(serde_json::json!({
                "symbol": args.symbol,
                "count": 0,
                "alerts": []
            })),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_alert_history_tool() {
        let tool = AlertHistoryTool::new(24);
        assert_eq!(tool.name(), "alert_history");
    }
}
