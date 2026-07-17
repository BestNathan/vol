//! Options tool - queries deribit_options table.

use async_trait::async_trait;
use serde_json::json;
use tracing::info;
use vol_llm_tool::{ExecutableTool, ToolContext, ToolError, ToolResult};
use vol_tdengine::{TdengineClient, TdengineConfig, TdengineResponse};

/// Options tool for querying deribit_options table
pub struct OptionsTool {
    client: TdengineClient,
}

impl OptionsTool {
    pub fn new(config: Option<TdengineConfig>) -> Self {
        Self {
            client: TdengineClient::new(config.unwrap_or_default()),
        }
    }

    /// Process TDengine response and format result
    fn format_response(&self, response: TdengineResponse, instrument: &str) -> String {
        if response.code != 0 {
            return format!(
                "Error: {}",
                response.desc.unwrap_or_else(|| "Unknown error".to_string())
            );
        }

        let data = response.data.unwrap_or(json!([]));
        let rows = data.as_array().map(std::vec::Vec::len).unwrap_or(0);

        if rows == 0 {
            return format!("No data found for {instrument}");
        }

        // Format: [[timestamp, instrument_name, iv, mark_price, expiry_date, strike_price, type], ...]
        let first_row = data.get(0).and_then(|v| v.as_array());
        if let Some(row) = first_row {
            if row.len() >= 7 {
                let timestamp = row.first().map(ToString::to_string).unwrap_or_default();
                let name = row
                    .get(1)
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or(instrument);
                let iv = row
                    .get(2)
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(0.0);
                let mark_price = row
                    .get(3)
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(0.0);
                let expiry = row
                    .get(4)
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("unknown");
                let strike = row
                    .get(5)
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(0.0);
                let opt_type = row
                    .get(6)
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("unknown");

                return format!(
                    "Instrument: {} | Type: {} | IV: {:.2}% | Mark Price: ${:.4} | Strike: ${:.0} | Expiry: {} | Timestamp: {} | Rows: {}",
                    name, opt_type, iv * 100.0, mark_price, strike, expiry, timestamp, rows
                );
            }
        }

        format!("Retrieved {rows} rows for {instrument}")
    }
}

#[async_trait]
impl ExecutableTool for OptionsTool {
    fn name(&self) -> &'static str {
        "options"
    }

    fn description(&self) -> &'static str {
        "Query options data from deribit_options table. Returns IV, mark price, and other option Greeks for a specific instrument."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "instrument": {
                    "type": "string",
                    "description": "Option instrument name (e.g., BTC-29DEC23-3000-C)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Number of rows to retrieve",
                    "default": 10
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
            .unwrap_or(10) as u32;

        info!("Querying options data for {} (limit={})", instrument, limit);

        let sql = format!(
            "SELECT _ts, instrument_name, iv, mark_price, expiry_date, strike_price, type \
             FROM deribit_options \
             WHERE instrument_name = '{instrument}' \
             ORDER BY _ts DESC \
             LIMIT {limit}"
        );

        match self.client.query_with_db(&sql).await {
            Ok(response) => {
                let result = self.format_response(response, instrument);
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
    fn test_options_tool_creation() {
        let tool = OptionsTool::new(None);
        assert_eq!(tool.name(), "options");
    }
}
