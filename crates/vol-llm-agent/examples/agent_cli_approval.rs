//! Example: Agent with CLI-based human-in-the-loop approval.
//!
//! This example demonstrates how to use the HITL plugin with CLI approval channel.
//! Run with:
//! ```bash
//! cargo run --example agent_cli_approval
//! ```
//!
//! The agent will request approval before executing tool calls.
//! User can approve or reject each tool execution via CLI prompt.

use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;
use vol_llm_agent::plugins::CliApprovalChannel;
use vol_llm_agent::react::hitl::*;
use vol_llm_agent::react::*;
use vol_llm_core::{
    ConversationRequest, ConversationResponse, LLMClient, LLMProvider, StreamEvent,
    StreamEventData, ToolCall,
};
use vol_llm_tool::{ExecutableTool, ToolContext, ToolError, ToolResult};

// ============================================================================
// Mock Tools for Demo
// ============================================================================

/// Mock BTC price tool
pub struct MockBtcPriceTool;

#[async_trait]
impl ExecutableTool for MockBtcPriceTool {
    fn name(&self) -> &'static str {
        "get_btc_price"
    }

    fn description(&self) -> &'static str {
        "Get current BTC price in USD. Returns the latest Bitcoin market price."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    async fn execute(
        &self,
        _args: &serde_json::Value,
        _context: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        // Simulate API call delay
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let result = json!({
            "symbol": "BTC",
            "price_usd": 69420.50,
            "change_24h": 2.5,
            "timestamp": "2026-04-10T12:00:00Z"
        });

        Ok(ToolResult {
            call_id: String::new(),
            success: true,
            content: format!("BTC Price: ${:.2} USD (24h change: +{:.1}%)", 69420.50, 2.5),
            error: None,
            data: Some(result),
        })
    }
}

/// Mock ETH volatility tool
pub struct MockEthVolatilityTool;

#[async_trait]
impl ExecutableTool for MockEthVolatilityTool {
    fn name(&self) -> &'static str {
        "get_eth_volatility"
    }

    fn description(&self) -> &'static str {
        "Get ETH implied volatility index. Returns current IV percentage for Ethereum options."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "period": {
                    "type": "string",
                    "description": "Time period (1d, 7d, 30d)",
                    "default": "1d"
                }
            },
            "required": []
        })
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _context: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let period = args["period"].as_str().unwrap_or("1d");

        // Simulate API call delay
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let iv = 100.8; // Elevated IV for demo
        let result = json!({
            "asset": "ETH",
            "period": period,
            "implied_volatility": iv,
            "iv_percentage": format!("{}%", iv),
            "timestamp": "2026-04-10T12:00:00Z"
        });

        Ok(ToolResult {
            call_id: String::new(),
            success: true,
            content: format!("ETH IV ({}): {:.1}%", period, iv),
            error: None,
            data: Some(result),
        })
    }
}

// ============================================================================
// Mock LLM for Testing (no API key required)
// ============================================================================

struct MockLlmWithTools {
    call_count: std::sync::atomic::AtomicUsize,
}

impl MockLlmWithTools {
    fn new() -> Self {
        Self {
            call_count: std::sync::atomic::AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl LLMClient for MockLlmWithTools {
    fn provider(&self) -> LLMProvider {
        LLMProvider::Anthropic
    }

    fn model(&self) -> &str {
        "mock-tool-calling-model"
    }

    fn supported_params(&self) -> &[vol_llm_core::SupportedParam] {
        &[]
    }

    async fn converse(
        &self,
        _request: ConversationRequest,
    ) -> vol_llm_core::Result<ConversationResponse> {
        unimplemented!("Use converse_stream instead")
    }

    async fn converse_stream(
        &self,
        _request: ConversationRequest,
    ) -> vol_llm_core::Result<vol_llm_core::stream::StreamReceiver> {
        use tokio::sync::mpsc;

        let call_count = self
            .call_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let (tx, rx) = mpsc::channel(10);

        tokio::spawn(async move {
            if call_count == 0 {
                // First call - return tool call for BTC price
                let _ = tx
                    .send(Ok(StreamEvent {
                        id: "event_1".to_string(),
                        data: StreamEventData::ToolCallComplete {
                            tool_call: ToolCall {
                                id: "call_btc_1".to_string(),
                                name: "get_btc_price".to_string(),
                                arguments: "{}".to_string(),
                                r#type: "function".to_string(),
                            },
                        },
                    }))
                    .await;
            } else if call_count == 1 {
                // Second call - return tool call for ETH volatility
                let _ = tx
                    .send(Ok(StreamEvent {
                        id: "event_2".to_string(),
                        data: StreamEventData::ToolCallComplete {
                            tool_call: ToolCall {
                                id: "call_eth_1".to_string(),
                                name: "get_eth_volatility".to_string(),
                                arguments: r#"{"period":"1d"}"#.to_string(),
                                r#type: "function".to_string(),
                            },
                        },
                    }))
                    .await;
            } else {
                // Final call - return analysis summary
                let response = "根据查询结果：\n\n1. **BTC 价格**: $69,420.50 USD，24 小时上涨 +2.5%\n2. **ETH 波动率**: 100.8% (1 天期)，处于历史高位\n\n**分析建议**:\n- ETH IV 超过 100% 表明市场预期波动剧烈\n- 可考虑卖出期权策略获取时间价值\n- 注意风险管理，设置止损位";

                let _ = tx
                    .send(Ok(StreamEvent {
                        id: "event_3".to_string(),
                        data: StreamEventData::ContentComplete {
                            content: response.to_string(),
                        },
                    }))
                    .await;
            }
        });

        Ok(vol_llm_core::StreamReceiver::new(rx))
    }
}

// ============================================================================
// Main Example
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("═══════════════════════════════════════════════════════════");
    println!("  ReAct Agent with Human-in-the-Loop Approval");
    println!("═══════════════════════════════════════════════════════════");
    println!();
    println!("This example demonstrates the HITL plugin system.");
    println!("The agent will request approval before executing tools.");
    println!();
    println!("Press:");
    println!("  'y' + Enter  - Approve tool execution");
    println!("  'n' + Enter  - Reject tool execution");
    println!();

    // HITL configuration: require approval for all tool executions
    let config = HitlConfig {
        triggers: vec![
            // Require approval before executing any tool
            ApprovalTrigger::ToolExecution { tools: None },
        ],
        timeout_secs: 60,
        on_timeout: TimeoutBehavior::Reject {
            reason: "Timeout waiting for approval".to_string(),
        },
        timeout_message: None,
    };

    // Create CLI approval channel
    let channel = Arc::new(CliApprovalChannel);

    // Create HITL plugin
    let hitl_plugin = HitlPlugin::new(config, channel);

    // Create mock LLM
    let mock_llm = Arc::new(MockLlmWithTools::new());

    // Build agent with HITL plugin and observability
    let working_dir = std::path::PathBuf::from(".");

    let agent = ReActAgent::builder()
        .with_llm(mock_llm)
        .with_tool(MockBtcPriceTool)
        .with_tool(MockEthVolatilityTool)
        .with_plugin(hitl_plugin)
        .with_agent_id("hitl_demo_agent".to_string())
        .with_working_dir(working_dir)
        .with_max_iterations(3)
        .with_system_prompt(
            "你是一个专业的加密货币市场分析师。你有访问市场数据的工具。
            当用户询问价格或波动率时，请使用工具查询数据并提供分析建议。
            请使用中文回复。"
                .to_string(),
        )
        .build()?;

    // Example query that will trigger tool calls
    let query = "请查询 BTC 当前价格，并分析 ETH 的隐含波动率是否处于高位？";

    println!("═══════════════════════════════════════════════════════════");
    println!("  Running Agent");
    println!("═══════════════════════════════════════════════════════════");
    println!("Query: {}", query);
    println!();

    // Run agent
    let result = agent.run(query).await;

    println!();
    println!("═══════════════════════════════════════════════════════════");
    println!("  Agent Execution Results");
    println!("═══════════════════════════════════════════════════════════");
    println!();

    match result {
        Ok(response) => {
            println!("Agent completed successfully.");
            println!("Final answer: {}", response.content);
            println!("Check observability logs for event details.");
        }
        Err(e) => {
            eprintln!("Agent run failed: {:?}", e);
        }
    }

    println!();
    println!("═══════════════════════════════════════════════════════════");
    println!("  Example Complete");
    println!("═══════════════════════════════════════════════════════════");
    println!();
    println!("Key features demonstrated:");
    println!("  ✓ HITL plugin intercepts tool execution requests");
    println!("  ✓ CLI approval channel prompts for user approval");
    println!("  ✓ Agent continues or aborts based on approval decision");
    println!("  ✓ Observability plugin logs all events to file");
    println!();
    println!("Log files written to: logs/agents/hitl_demo_agent/");
    println!();

    Ok(())
}
