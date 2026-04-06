//! Market data tool.

use async_trait::async_trait;
use serde::Deserialize;
use std::error::Error;
use crate::tool::{Tool, ToolResult, ToolContext};

#[derive(Debug, Deserialize)]
struct MarketDataArgs {
    symbol: String,
    data_type: Option<String>,
}

/// Market data tool
pub struct MarketDataTool;

#[async_trait]
impl Tool for MarketDataTool {
    fn name(&self) -> &str {
        "market_data"
    }

    fn description(&self) -> &str {
        "获取实时市场数据，包括价格、涨跌幅、成交量等"
    }

    fn parameters(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({
            "type": "object",
            "properties": {
                "symbol": {
                    "type": "string",
                    "description": "标的符号"
                },
                "data_type": {
                    "type": "string",
                    "enum": ["price", "volume", "funding_rate", "open_interest"],
                    "description": "数据类型"
                }
            },
            "required": ["symbol"]
        }))
    }

    async fn execute(&self, args: &str, _context: &ToolContext)
        -> Result<ToolResult, Box<dyn Error + Send>> {

        let args: MarketDataArgs = serde_json::from_str(args)
            .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

        // Placeholder - TODO: integrate with market data API
        let content = format!("获取 {} 市场数据", args.symbol);

        Ok(ToolResult {
            call_id: String::new(),
            success: true,
            content,
            error: None,
            data: Some(serde_json::json!({
                "symbol": args.symbol,
                "data": {}
            })),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_market_data_tool() {
        let tool = MarketDataTool;
        assert_eq!(tool.name(), "market_data");
    }
}
