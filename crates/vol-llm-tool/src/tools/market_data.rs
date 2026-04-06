//! Market data tool.

use async_trait::async_trait;
use serde_json::json;
use tracing::info;
use crate::tool::{ExecutableTool, ToolResult, ToolContext, Result, ToolError};
use crate::tdengine::{TdengineClient, TdengineConfig};

/// Market data tool
pub struct MarketDataTool {
    client: TdengineClient,
}

impl MarketDataTool {
    pub fn new(config: Option<TdengineConfig>) -> Self {
        Self {
            client: TdengineClient::new(config.unwrap_or_default()),
        }
    }
}

#[async_trait]
impl ExecutableTool for MarketDataTool {
    fn name(&self) -> &'static str {
        "market_data"
    }

    fn description(&self) -> &'static str {
        "Get current market price data from TDengine (deribit_index_price table)"
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "instrument": {
                    "type": "string",
                    "description": "Index name (e.g., btc_usd, eth_usd)"
                },
                "data_type": {
                    "type": "string",
                    "description": "Type of data to retrieve",
                    "enum": ["price", "all"]
                }
            },
            "required": ["instrument"]
        })
    }

    async fn execute(&self, args: &serde_json::Value, context: &ToolContext) -> Result<ToolResult> {
        let instrument = args["instrument"]
            .as_str()
            .or_else(|| {
                let s = context.instrument.as_str();
                if s.is_empty() { None } else { Some(s) }
            })
            .ok_or_else(|| ToolError::InvalidArguments("instrument required".to_string()))?;

        let data_type = args["data_type"].as_str().unwrap_or("all");

        info!("Querying market data for {} (type={})", instrument, data_type);

        match self.client.query_market_data(instrument).await {
            Ok(response) => {
                if response.code == 0 {
                    let data = response.data.unwrap_or(json!([]));
                    let row = data.as_array().and_then(|a| a.first());

                    match row {
                        Some(r) => {
                            // TDengine response: [_ts, price, index_name]
                            let price = r.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0);

                            let content = match data_type {
                                "price" => format!("{} price: {}", instrument, price),
                                _ => format!(
                                    "{} market data - Price: {}",
                                    instrument, price
                                ),
                            };

                            Ok(ToolResult::success(content))
                        }
                        None => Ok(ToolResult::success(format!(
                            "No market data found for {}",
                            instrument
                        ))),
                    }
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
    fn test_market_data_tool_creation() {
        let tool = MarketDataTool::new(None);
        assert_eq!(tool.name(), "market_data");
    }
}
