//! Realized volatility (RV) tool - queries deribit_rv table.

use async_trait::async_trait;
use serde_json::json;
use tracing::info;
use vol_llm_tool::{ExecutableTool, ToolResult, ToolContext, ToolError};
use vol_tdengine::{TdengineClient, TdengineConfig, TdengineResponse};

/// RV tool for querying deribit_rv table
pub struct RvTool {
    client: TdengineClient,
}

impl RvTool {
    pub fn new(config: Option<TdengineConfig>) -> Self {
        Self {
            client: TdengineClient::new(config.unwrap_or_default()),
        }
    }

    /// Process TDengine response and format result
    fn format_response(&self, response: TdengineResponse, index_name: &str) -> String {
        if response.code != 0 {
            return format!("Error: {}", response.desc.unwrap_or_else(|| "Unknown error".to_string()));
        }

        let data = response.data.unwrap_or(json!([]));
        let rows = data.as_array().map(|a| a.len()).unwrap_or(0);

        if rows == 0 {
            return format!("No data found for {}", index_name);
        }

        // Format: [[timestamp, rv, index_name], ...]
        let first_row = data[0].as_array();
        if let Some(row) = first_row {
            if row.len() >= 3 {
                let timestamp = row[0].to_string();
                let rv = row[1].as_f64().unwrap_or(0.0);
                let name = row[2].as_str().unwrap_or(index_name);
                return format!(
                    "Index: {} | RV: {:.2}% | Timestamp: {} | Rows: {}",
                    name, rv * 100.0, timestamp, rows
                );
            }
        }

        format!("Retrieved {} rows for {} (RV data)", rows, index_name)
    }
}

#[async_trait]
impl ExecutableTool for RvTool {
    fn name(&self) -> &'static str {
        "rv"
    }

    fn description(&self) -> &'static str {
        "Query realized volatility (RV) data from deribit_rv table. Returns RV for a given index."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "index": {
                    "type": "string",
                    "description": "Index name (e.g., BTC, ETH, btc_usd)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Number of rows to retrieve",
                    "default": 10
                }
            },
            "required": ["index"]
        })
    }

    async fn execute(&self, args: &serde_json::Value, _context: &ToolContext) -> Result<ToolResult, ToolError> {
        let index = args["index"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidArguments("index required".to_string()))?;

        let limit = args["limit"].as_u64().unwrap_or(10) as u32;

        info!("Querying RV data for {} (limit={})", index, limit);

        let index_name = index
            .to_uppercase()
            .split('_')
            .next()
            .unwrap_or(index)
            .to_string();

        let sql = format!(
            "SELECT _ts, rv, index_name \
             FROM deribit_rv \
             WHERE index_name = '{}' \
             ORDER BY _ts DESC \
             LIMIT {}",
            index_name, limit
        );

        match self.client.query_with_db(&sql).await {
            Ok(response) => {
                let result = self.format_response(response, &index_name);
                if result.starts_with("Error:") {
                    Err(ToolError::ExecutionFailed(result))
                } else {
                    Ok(ToolResult::success(result))
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
    fn test_rv_tool_creation() {
        let tool = RvTool::new(None);
        assert_eq!(tool.name(), "rv");
    }
}
