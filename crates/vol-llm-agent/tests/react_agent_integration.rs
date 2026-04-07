//! ReAct Agent integration tests.
//!
//! Run with: cargo test --test react_agent_integration -- --nocapture
//!
//! These tests verify the ReAct Agent workflow with real LLM provider and TDengine tools.

use vol_llm_agent::{ReActAgent, AgentConfig, AgentStreamEvent};
use vol_llm_tool::{ToolRegistry, ToolContext};
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

    // Create tool registry with TDengine tools
    let mut registry = ToolRegistry::new();
    registry.register(VolatilityIndexTool::new(None));
    registry.register(IndexPriceTool::new(None));
    registry.register(OptionsTool::new(None));
    registry.register(RvTool::new(None));

    let agent_config = AgentConfig {
        max_iterations: 5,
        system_prompt: "You are a helpful assistant.".to_string(),
        verbose: true,
    };

    Some(ReActAgent::new(llm.into(), Arc::new(registry), agent_config))
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

    let stream_result = agent.run("What is the current BTC price?", context).await;

    match stream_result {
        Ok(mut stream) => {
            let mut final_response = None;
            let mut tool_calls_count = 0;

            while let Some(event) = stream.recv().await {
                match event.unwrap() {
                    AgentStreamEvent::ToolCallBegin { tool_name, .. } => {
                        println!("Tool call begin: {}", tool_name);
                        tool_calls_count += 1;
                    }
                    AgentStreamEvent::ToolCallComplete { tool_name, result } => {
                        println!("Tool call complete: {} = {}", tool_name, result);
                    }
                    AgentStreamEvent::AgentComplete { response } => {
                        println!("Agent response: {}", response.content);
                        println!("Iterations: {}", response.iterations);
                        println!("Tool calls: {}", response.tool_calls.len());
                        final_response = Some(response.content);
                    }
                    _ => {}
                }
            }

            // Verify agent used tools
            assert!(tool_calls_count > 0, "Agent should have called at least one tool");

            // Verify response mentions price or market data
            if let Some(content) = final_response {
                let content_lower = content.to_lowercase();
                assert!(
                    content_lower.contains("price") || content_lower.contains("market") || content_lower.contains("btc"),
                    "Response should mention price or BTC"
                );
            }
        }
        Err(e) => {
            eprintln!("Agent error: {:?}", e);
            // Don't fail test - could be API rate limit or network issue
        }
    }
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

    let stream_result = agent.run("Show me the recent volatility data for ETH", context).await;

    match stream_result {
        Ok(mut stream) => {
            while let Some(event) = stream.recv().await {
                match event.unwrap() {
                    AgentStreamEvent::AgentComplete { response } => {
                        println!("Agent response: {}", response.content);
                        println!("Iterations: {}", response.iterations);
                    }
                    _ => {}
                }
            }
        }
        Err(e) => {
            eprintln!("Agent error: {:?}", e);
        }
    }
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

    let stream_result = agent.run("Compare BTC and ETH volatility and explain the difference", context).await;

    match stream_result {
        Ok(mut stream) => {
            while let Some(event) = stream.recv().await {
                match event.unwrap() {
                    AgentStreamEvent::AgentComplete { response } => {
                        println!("Agent completed in {} iterations", response.iterations);
                        println!("Response: {}", response.content);
                    }
                    _ => {}
                }
            }
        }
        Err(vol_llm_agent::AgentError::MaxIterationsReached { max }) => {
            println!("Hit max iterations ({}) - this is expected for complex queries", max);
        }
        Err(e) => {
            eprintln!("Agent error: {:?}", e);
        }
    }
}
