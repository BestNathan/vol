//! Index price tool - queries deribit_index_price table.

use async_trait::async_trait;
use serde_json::json;
use tracing::info;
use vol_llm_tool::{ExecutableTool, ToolContext, ToolError, ToolResult};
use vol_tdengine::{TdengineClient, TdengineConfig, TdengineResponse};

/// Index price tool for querying deribit_index_price table
pub struct IndexPriceTool {
    client: TdengineClient,
}

impl IndexPriceTool {
    pub fn new(config: Option<TdengineConfig>) -> Self {
        Self {
            client: TdengineClient::new(config.unwrap_or_default()),
        }
    }

    /// Process TDengine response and format result
    fn format_response(&self, response: TdengineResponse, index_name: &str) -> String {
        if response.code != 0 {
            return format!(
                "Error: {}",
                response.desc.unwrap_or_else(|| "Unknown error".to_string())
            );
        }

        let data = response.data.unwrap_or(json!([]));
        let rows = data.as_array().map(std::vec::Vec::len).unwrap_or(0);

        if rows == 0 {
            return format!("No data found for {index_name}");
        }

        // Format: [[timestamp, price, index_name], ...]
        let first_row = data.get(0).and_then(|v| v.as_array());
        if let Some(row) = first_row {
            if row.len() >= 3 {
                let timestamp = row.first().map(ToString::to_string).unwrap_or_default();
                let price = row
                    .get(1)
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(0.0);
                let name = row
                    .get(2)
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or(index_name);
                return format!(
                    "Index: {name} | Price: ${price:.2} | Timestamp: {timestamp} | Rows: {rows}"
                );
            }
        }

        format!("Retrieved {rows} rows for {index_name}")
    }
}

#[async_trait]
impl ExecutableTool for IndexPriceTool {
    fn name(&self) -> &'static str {
        "index_price"
    }

    fn description(&self) -> &'static str {
        "Query index price data from deribit_index_price table. Returns latest price for a given index (e.g., BTC, ETH)."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "instrument": {
                    "type": "string",
                    "description": "Instrument name (e.g., BTC, ETH, btc_usd)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Number of rows to retrieve",
                    "default": 1
                }
            },
            "required": ["instrument"]
        })
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _context: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let instrument = args
            .get("instrument")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("instrument required".to_string()))?;

        #[allow(clippy::cast_possible_truncation)]
        let limit = args
            .get("limit")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(1) as u32;

        info!("Querying index price for {} (limit={})", instrument, limit);

        // Build SQL query
        let index_name = instrument
            .to_uppercase()
            .split('_')
            .next()
            .unwrap_or(instrument)
            .to_string();

        let sql = format!(
            "SELECT _ts, price, index_name \
             FROM deribit_index_price \
             WHERE index_name = '{index_name}' \
             ORDER BY _ts DESC \
             LIMIT {limit}"
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
            Err(e) => Err(ToolError::ExecutionFailed(format!("TDengine error: {e}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_price_tool_creation() {
        let tool = IndexPriceTool::new(None);
        assert_eq!(tool.name(), "index_price");
    }
}
