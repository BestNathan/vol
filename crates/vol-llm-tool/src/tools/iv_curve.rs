//! IV curve tool.

use async_trait::async_trait;
use serde::Deserialize;
use std::error::Error;
use crate::tool::{Tool, ToolResult, ToolContext};

#[derive(Debug, Deserialize)]
struct IvCurveArgs {
    symbol: String,
    tenor: Option<String>,
}

/// IV curve tool
pub struct IvCurveTool;

#[async_trait]
impl Tool for IvCurveTool {
    fn name(&self) -> &str {
        "iv_curve"
    }

    fn description(&self) -> &str {
        "获取标的的隐含波动率曲面数据，包括不同行权价和期限的 IV"
    }

    fn parameters(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({
            "type": "object",
            "properties": {
                "symbol": {
                    "type": "string",
                    "description": "标的符号"
                },
                "tenor": {
                    "type": "string",
                    "enum": ["short", "medium", "long"],
                    "description": "期限"
                }
            },
            "required": ["symbol"]
        }))
    }

    async fn execute(&self, args: &str, _context: &ToolContext)
        -> Result<ToolResult, Box<dyn Error + Send>> {

        let args: IvCurveArgs = serde_json::from_str(args)
            .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

        // Placeholder - TODO: integrate with TDengine
        let content = format!("获取 {} IV 曲线数据", args.symbol);

        Ok(ToolResult {
            call_id: String::new(),
            success: true,
            content,
            error: None,
            data: Some(serde_json::json!({
                "symbol": args.symbol,
                "iv_data": []
            })),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iv_curve_tool() {
        let tool = IvCurveTool;
        assert_eq!(tool.name(), "iv_curve");
    }
}
