//! IV curve tool.

use async_trait::async_trait;
use serde_json::json;
use tracing::info;
use crate::tool::{ExecutableTool, ToolResult, ToolContext, Result, ToolError};
use crate::tdengine::{TdengineClient, TdengineConfig};

/// IV curve tool
pub struct IvCurveTool {
    client: TdengineClient,
}

impl IvCurveTool {
    pub fn new(config: Option<TdengineConfig>) -> Self {
        Self {
            client: TdengineClient::new(config.unwrap_or_default()),
        }
    }
}

#[async_trait]
impl ExecutableTool for IvCurveTool {
    fn name(&self) -> &'static str {
        "iv_curve"
    }

    fn description(&self) -> &'static str {
        "Get implied volatility curve data for an instrument from TDengine"
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "instrument": {
                    "type": "string",
                    "description": "Instrument name (e.g., BTC-29DEC23)"
                },
                "delta": {
                    "type": "array",
                    "items": {"type": "number"},
                    "description": "Delta values to query"
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

        let delta = args["delta"].as_array().map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_f64())
                .collect::<Vec<f64>>()
        });

        info!("Querying IV curve for {} (deltas: {:?})", instrument, delta);

        let delta_slice: Option<&[f64]> = delta.as_deref();
        match self.client.query_iv_curve(instrument, delta_slice).await {
            Ok(response) => {
                if response.code == 0 {
                    let data = response.data.unwrap_or(json!([]));
                    let count = data.as_array().map(|a| a.len()).unwrap_or(0);

                    let delta_info = match delta_slice {
                        Some(d) => format!(" at {} delta points", d.len()),
                        None => String::new(),
                    };

                    Ok(ToolResult::success(format!(
                        "Retrieved {} IV curve data points for {}{}",
                        count, instrument, delta_info
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
    fn test_iv_curve_tool_creation() {
        let tool = IvCurveTool::new(None);
        assert_eq!(tool.name(), "iv_curve");
    }
}
