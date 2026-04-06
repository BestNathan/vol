//! Rule info tool.

use async_trait::async_trait;
use serde_json::json;
use tracing::info;
use crate::tool::{ExecutableTool, ToolResult, ToolContext, Result, ToolError};
use crate::tdengine::{TdengineClient, TdengineConfig};

/// Rule info tool
pub struct RuleInfoTool {
    client: TdengineClient,
}

impl RuleInfoTool {
    pub fn new(config: Option<TdengineConfig>) -> Self {
        Self {
            client: TdengineClient::new(config.unwrap_or_default()),
        }
    }
}

#[async_trait]
impl ExecutableTool for RuleInfoTool {
    fn name(&self) -> &'static str {
        "rule_info"
    }

    fn description(&self) -> &'static str {
        "Get realized volatility data from TDengine (deribit_rv table)"
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "index_name": {
                    "type": "string",
                    "description": "Index name (e.g., btc_usd, eth_usd)"
                },
                "list_all": {
                    "type": "boolean",
                    "description": "List all available data"
                }
            }
        })
    }

    async fn execute(&self, args: &serde_json::Value, _context: &ToolContext) -> Result<ToolResult> {
        let index_name = args["index_name"].as_str();
        let list_all = args["list_all"].as_bool().unwrap_or(false);

        let index_name_opt = if list_all { None } else { index_name };

        info!("Querying RV data (index={:?}, list_all={})", index_name_opt, list_all);

        match self.client.query_rules(index_name_opt).await {
            Ok(response) => {
                if response.code == 0 {
                    let data = response.data.unwrap_or(json!([]));
                    let count = data.as_array().map(|a| a.len()).unwrap_or(0);

                    let content = if list_all {
                        format!("Available RV data: {} records found", count)
                    } else if let Some(name) = index_name {
                        format!("RV data for {}: {} record(s) found", name, count)
                    } else {
                        "Please specify index_name or set list_all=true".to_string()
                    };

                    Ok(ToolResult::success(content))
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
    fn test_rule_info_tool_creation() {
        let tool = RuleInfoTool::new(None);
        assert_eq!(tool.name(), "rule_info");
    }
}
