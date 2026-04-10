//! ReAct Agent integration tests.
//!
//! Run with: cargo test --test react_agent_integration -- --nocapture
//!
//! These tests verify the ReAct Agent workflow with real LLM provider and TDengine tools.

use vol_llm_agent::ReActAgent;
use vol_llm_tool::ToolContext;
use vol_llm_provider::{create_provider, LLMConfig, Secret};
use vol_llm_core::LLMProvider;
use vol_llm_tdengine::{VolatilityIndexTool, IndexPriceTool, OptionsTool, RvTool};
use std::sync::Arc;

/// Create a test agent with TDengine tools
fn create_test_agent() -> Option<ReActAgent> {
    // Load LLM provider using Anthropic API key from environment
    let api_key = std::env::var("ANTHROPIC_AUTH_TOKEN").ok()?;
    let config = LLMConfig {
        provider: LLMProvider::Anthropic,
        model: "claude-sonnet-4-6".to_string(),
        api_key: Secret::Literal(api_key),
        base_url: "https://coding.dashscope.aliyuncs.com/apps/anthropic".to_string(),
    };

    let llm = create_provider(&config).ok()?;

    // Create agent with builder
    let agent = ReActAgent::builder()
        .with_llm(llm.into())
        .with_tool(VolatilityIndexTool::new(None))
        .with_tool(IndexPriceTool::new(None))
        .with_tool(OptionsTool::new(None))
        .with_tool(RvTool::new(None))
        .with_max_iterations(5)
        .with_system_prompt("You are a helpful assistant.".to_string())
        .with_verbose(true)
        .build()
        .ok()?;

    Some(agent)
}

#[tokio::test]
#[ignore = "requires ANTHROPIC_AUTH_TOKEN and correct model"]
async fn test_agent_with_market_data_query() {
    let agent = match create_test_agent() {
        Some(a) => a,
        None => {
            eprintln!("Skipping test - LLM provider not configured");
            return;
        }
    };

    let context = ToolContext::default();
    agent.run("What is the current BTC price?", context).await.unwrap();
}

#[tokio::test]
#[ignore = "requires ANTHROPIC_AUTH_TOKEN and correct model"]
async fn test_agent_with_volatility_query() {
    let agent = match create_test_agent() {
        Some(a) => a,
        None => {
            eprintln!("Skipping test - LLM provider not configured");
            return;
        }
    };

    let context = ToolContext::default();
    agent.run("Show me the recent volatility data for ETH", context).await.unwrap();
}

#[tokio::test]
#[ignore = "requires ANTHROPIC_AUTH_TOKEN and correct model"]
async fn test_agent_max_iterations() {
    let agent = match create_test_agent() {
        Some(a) => a,
        None => {
            eprintln!("Skipping test - LLM provider not configured");
            return;
        }
    };

    // This query should trigger multiple iterations
    let context = ToolContext::default();
    agent.run("Compare BTC and ETH volatility and explain the difference", context).await.unwrap();
}
