//! Example: Agent with real Anthropic API and real TDengine tools for observability testing.
//!
//! This example demonstrates the observability plugin with:
//! - Real Anthropic/DashScope LLM API calls
//! - Real TDengine-based tools for market data queries
//! - JSONL file logging for all agent events
//!
//! Run with:
//! ```bash
//! export ANTHROPIC_AUTH_TOKEN=your_token_here
//! cargo run --example agent_observability_test
//! ```
//!
//! Log files will be written to: logs/agents/market_analyst_agent/

use std::path::PathBuf;
use std::sync::Arc;
use vol_llm_agent::react::{AgentConfig, ReActAgent};
use vol_llm_core::LLMProvider;
use vol_llm_provider::LLMConfig;
use vol_llm_tdengine::{IndexPriceTool, OptionsTool, RvTool, VolatilityIndexTool};
use vol_tdengine::TdengineConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("═══════════════════════════════════════════════════════════");
    println!("  ReAct Agent with Real API and Observability");
    println!("═══════════════════════════════════════════════════════════");
    println!();

    // Check for required environment variables
    let api_key = std::env::var("ANTHROPIC_AUTH_TOKEN").map_err(|_| {
        eprintln!("Error: ANTHROPIC_AUTH_TOKEN environment variable not set");
        eprintln!("Please set it: export ANTHROPIC_AUTH_TOKEN=your_token_here");
        "Missing API token"
    })?;

    println!("Configuration:");
    println!("  ✓ ANTHROPIC_AUTH_TOKEN is set");
    println!();

    // Create LLM configuration for DashScope Anthropic endpoint
    let llm_config = LLMConfig::with_literal_key(
        LLMProvider::Anthropic,
        "qwen3.5-plus",
        api_key,
        "https://coding.dashscope.aliyuncs.com/apps/anthropic",
    );

    // Create Anthropic provider
    let llm = vol_llm_provider::anthropic::AnthropicProvider::new(&llm_config)
        .map_err(|e| format!("Failed to create Anthropic provider: {e}"))?;

    println!("  ✓ Anthropic provider initialized (qwen3.5-plus via DashScope)");
    println!();

    // Create TDengine tools
    let tdengine_config = TdengineConfig::default();
    let volatility_tool = VolatilityIndexTool::new(Some(tdengine_config.clone()));
    let price_tool = IndexPriceTool::new(Some(tdengine_config.clone()));
    let options_tool = OptionsTool::new(Some(tdengine_config.clone()));
    let rv_tool = RvTool::new(Some(tdengine_config.clone()));

    println!("  ✓ TDengine tools initialized:");
    println!("    - volatility_index");
    println!("    - index_price");
    println!("    - options");
    println!("    - rv");
    println!();

    // Configure observability
    let agent_id = "market_analyst_agent".to_string();
    let working_dir = PathBuf::from(".");

    // Build agent with observability plugin
    let config = AgentConfig::builder()
        .with_llm(Arc::new(llm))
        .with_tool(volatility_tool)
        .with_tool(price_tool)
        .with_tool(options_tool)
        .with_tool(rv_tool)
        .with_system_prompt(
            "你是一个专业的加密货币市场分析师。你有访问 Deribit 市场数据的工具，包括：
            - volatility_index: 查询波动率指数数据
            - index_price: 查询标的价格指数
            - options: 查询期权数据
            - rv: 查询已实现波动率

            当用户询问市场状况时，请使用工具查询相关数据并提供分析建议。
            请使用中文回复。"
                .to_string(),
        )
        .build()?;
    let agent = ReActAgent::new(config);

    println!("  ✓ Agent built with observability plugin");
    println!();

    // Test query
    let query = "请查询 BTC 当前的波动率水平和 ETH 的价格，并分析当前市场状况";

    println!("═══════════════════════════════════════════════════════════");
    println!("  Running Agent");
    println!("═══════════════════════════════════════════════════════════");
    println!("Query: {query}");
    println!();

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
            println!("Events were logged to observability plugin.");
        }
        Err(e) => {
            eprintln!("Agent run failed: {e:?}");
        }
    }

    println!();
    println!("═══════════════════════════════════════════════════════════");
    println!("  Observability Logs");
    println!("═══════════════════════════════════════════════════════════");
    println!();
    println!("Log files written to:");
    println!("  Session logs: logs/agents/{agent_id}/sessions/");
    println!("  Run logs:     logs/agents/{agent_id}/runs/");
    println!();
    println!("Log format: JSONL (one JSON object per line)");
    println!();

    // Show log file locations
    let agent_path = working_dir.join("logs/agents").join(&agent_id);
    if agent_path.exists() {
        println!("Checking log directories...");

        let sessions_path = agent_path.join("sessions");
        let runs_path = agent_path.join("runs");

        if sessions_path.exists() {
            if let Ok(entries) = std::fs::read_dir(&sessions_path) {
                let count = entries.count();
                println!(
                    "  ✓ Sessions: {} files in {}",
                    count,
                    sessions_path.display()
                );
            }
        }

        if runs_path.exists() {
            if let Ok(entries) = std::fs::read_dir(&runs_path) {
                let count = entries.count();
                println!("  ✓ Runs: {} files in {}", count, runs_path.display());
            }
        }
    }
    println!();

    println!("═══════════════════════════════════════════════════════════");
    println!("  Example Complete");
    println!("═══════════════════════════════════════════════════════════");
    println!();
    println!("Features demonstrated:");
    println!("  ✓ Real Anthropic API calls via DashScope");
    println!("  ✓ Real TDengine tools for market data");
    println!("  ✓ Observability plugin logging all events");
    println!("  ✓ JSONL format for structured logs");
    println!("  ✓ Agent-centric log organization");
    println!();

    Ok(())
}
