//! Alert history tool.

use async_trait::async_trait;
use serde_json::json;
use tracing::info;
use crate::tool::{ExecutableTool, ToolResult, ToolContext, Result, ToolError};
use crate::tdengine::{TdengineClient, TdengineConfig};

/// Alert history tool
pub struct AlertHistoryTool {
    client: TdengineClient,
}

impl AlertHistoryTool {
    pub fn new(config: Option<TdengineConfig>) -> Self {
        Self {
            client: TdengineClient::new(config.unwrap_or_default()),
        }
    }
}

#[async_trait]
impl ExecutableTool for AlertHistoryTool {
    fn name(&self) -> &'static str {
        "alert_history"
    }

    fn description(&self) -> &'static str {
        "Get recent volatility index history from TDengine (deribit_volatility_index table)"
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "symbol": {
                    "type": "string",
                    "description": "Symbol to query (e.g., BTC-PERP)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Number of alerts to retrieve",
                    "default": 10
                },
                "hours": {
                    "type": "integer",
                    "description": "Time window in hours"
                }
            },
            "required": ["symbol"]
        })
    }

    async fn execute(&self, args: &serde_json::Value, context: &ToolContext) -> Result<ToolResult> {
        let symbol = args["symbol"]
            .as_str()
            .or_else(|| {
                let s = context.instrument.as_str();
                if s.is_empty() { None } else { Some(s) }
            })
            .ok_or_else(|| ToolError::InvalidArguments("symbol required".to_string()))?;

        let limit = args["limit"].as_u64().unwrap_or(10) as u32;
        let hours = args["hours"].as_u64().map(|h| h as u32);

        info!("Querying alert history for {} (limit={}, hours={:?})", symbol, limit, hours);

        match self.client.query_alert_history(symbol, limit, hours).await {
            Ok(response) => {
                if response.code == 0 {
                    let data = response.data.unwrap_or(json!([]));
                    let count = data.as_array().map(|a| a.len()).unwrap_or(0);

                    Ok(ToolResult::success(format!(
                        "Retrieved {} alerts for {} (time window: {:?} hours)",
                        count, symbol, hours
                    )))
                } else {
                    Err(ToolError::ExecutionFailed(
                        response.desc.unwrap_or_else(|| "Query failed".to_string())
                    ))
                }
            }
            Err(e) => Err(ToolError::ExecutionFailed(format!("TDengine error: {}", e))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alert_history_tool_creation() {
        let tool = AlertHistoryTool::new(None);
        assert_eq!(tool.name(), "alert_history");
    }
}
