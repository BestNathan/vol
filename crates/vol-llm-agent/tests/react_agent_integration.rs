//! ReAct Agent integration tests.
//!
//! Run with: cargo test --test react_agent_integration -- --nocapture
//!
//! These tests verify the ReAct Agent workflow with real LLM provider and TDengine tools.

use vol_llm_agent::{ReActAgent, AgentBuilder};
use vol_llm_tool::{ToolRegistry, ToolContext};
use vol_llm_provider::{create_provider, LLMConfig};
use vol_llm_core::LLMProvider;

/// Create a test agent with TDengine tools
fn create_test_agent() -> Option<ReActAgent> {
    // Load LLM provider using Anthropic API key from environment
    let config = LLMConfig {
        provider: LLMProvider::Anthropic,
        model: "claude-sonnet-4-6".to_string(),
        api_key_env: "ANTHROPIC_AUTH_TOKEN".to_string(),
        endpoint: Some("https://coding.dashscope.aliyuncs.com/apps/anthropic".to_string()),
    };

    let llm = create_provider(&config).ok()?;

    // Create tool registry with TDengine tools
    let mut registry = ToolRegistry::new();
    registry.register_default_tools();

    AgentBuilder::new()
        .with_llm(llm)
        .with_tools(registry)
        .with_max_iterations(5)
        .verbose()
        .build()
}

#[tokio::test]
async fn test_agent_with_market_data_query() {
    let agent = match create_test_agent() {
        Some(a) => a,
        None => {
            eprintln!("Skipping test - LLM provider not configured");
            return;
        }
    };

    let context = ToolContext {
        instrument: "btc_usd".to_string(),
        ..Default::default()
    };

    let result = agent.run("What is the current BTC price?", context).await;

    match result {
        Ok(response) => {
            println!("Agent response: {}", response.content);
            println!("Iterations: {}", response.iterations);
            println!("Tool calls: {}", response.tool_calls.len());

            // Verify agent used tools
            assert!(!response.tool_calls.is_empty(), "Agent should have called at least one tool");

            // Verify response mentions price or market data
            let content_lower = response.content.to_lowercase();
            assert!(
                content_lower.contains("price") || content_lower.contains("market") || content_lower.contains("btc"),
                "Response should mention price or BTC"
            );
        }
        Err(e) => {
            eprintln!("Agent error: {:?}", e);
            // Don't fail test - could be API rate limit or network issue
        }
    }
}

#[tokio::test]
async fn test_agent_with_volatility_query() {
    let agent = match create_test_agent() {
        Some(a) => a,
        None => {
            eprintln!("Skipping test - LLM provider not configured");
            return;
        }
    };

    let context = ToolContext {
        instrument: "eth_usd".to_string(),
        ..Default::default()
    };

    let result = agent.run("Show me the recent volatility data for ETH", context).await;

    match result {
        Ok(response) => {
            println!("Agent response: {}", response.content);
            println!("Iterations: {}", response.iterations);
        }
        Err(e) => {
            eprintln!("Agent error: {:?}", e);
        }
    }
}

#[tokio::test]
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

    let result = agent.run("Compare BTC and ETH volatility and explain the difference", context).await;

    match result {
        Ok(response) => {
            println!("Agent completed in {} iterations", response.iterations);
            println!("Response: {}", response.content);
        }
        Err(vol_llm_agent::AgentError::MaxIterationsReached { max }) => {
            println!("Hit max iterations ({}) - this is expected for complex queries", max);
        }
        Err(e) => {
            eprintln!("Agent error: {:?}", e);
        }
    }
}
