//! Volatility index tool - queries deribit_volatility_index table.

use async_trait::async_trait;
use serde_json::json;
use tracing::info;
use vol_llm_tool::{ExecutableTool, ToolResult, ToolContext, ToolError};
use vol_tdengine::{TdengineClient, TdengineConfig, TdengineResponse};

/// Volatility index tool for querying deribit_volatility_index table
pub struct VolatilityIndexTool {
    client: TdengineClient,
}

impl VolatilityIndexTool {
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

        // Format: [[timestamp, volatility, index_name], ...]
        let first_row = data[0].as_array();
        if let Some(row) = first_row {
            if row.len() >= 3 {
                let timestamp = row[0].to_string();
                let volatility = row[1].as_f64().unwrap_or(0.0);
                let name = row[2].as_str().unwrap_or(index_name);
                return format!(
                    "Index: {} | Volatility: {:.2}% | Timestamp: {} | Rows: {}",
                    name, volatility, timestamp, rows
                );
            }
        }

        format!("Retrieved {} rows for {} (volatility index)", rows, index_name)
    }
}

#[async_trait]
impl ExecutableTool for VolatilityIndexTool {
    fn name(&self) -> &'static str {
        "volatility_index"
    }

    fn description(&self) -> &'static str {
        "Query volatility index data from deribit_volatility_index table. Returns historical volatility index values."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "symbol": {
                    "type": "string",
                    "description": "Symbol to query (e.g., BTC, ETH, btc_usd)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Number of rows to retrieve",
                    "default": 10
                },
                "hours": {
                    "type": "integer",
                    "description": "Time window in hours (optional)"
                }
            },
            "required": ["symbol"]
        })
    }

    async fn execute(&self, args: &serde_json::Value, _context: &ToolContext) -> Result<ToolResult, ToolError> {
        let symbol = args["symbol"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidArguments("symbol required".to_string()))?;

        let limit = args["limit"].as_u64().unwrap_or(10) as u32;
        let hours = args["hours"].as_u64().map(|h| h as u32);

        info!("Querying volatility index for {} (limit={}, hours={:?})", symbol, limit, hours);

        let time_filter = match hours {
            Some(h) => format!("AND _ts >= NOW - {}h", h),
            None => String::new(),
        };

        let index_name = if symbol.contains('_') {
            symbol.to_lowercase()
        } else {
            format!("{}_usd", symbol.to_lowercase())
        };

        let sql = format!(
            "SELECT _ts, volatility, index_name \
             FROM deribit_volatility_index \
             WHERE index_name = '{}' {} \
             ORDER BY _ts DESC \
             LIMIT {}",
            index_name, time_filter, limit
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
    fn test_volatility_index_tool_creation() {
        let tool = VolatilityIndexTool::new(None);
        assert_eq!(tool.name(), "volatility_index");
    }
}
