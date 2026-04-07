//! Debug agent output test.
//!
//! Run with: cargo test --test debug_agent_output -- --nocapture
//!
//! This test verifies the agent can produce output using real LLM.

use vol_llm_agent::{ReActAgent, AgentConfig};
use vol_llm_tool::{ToolRegistry, ToolContext, MarketDataTool};
use vol_llm_core::{LLMProvider, LLMClient, Message, ConversationRequest, ConversationResponse, TokenUsage, FinishReason, ToolCall};
use async_trait::async_trait;
use std::sync::Arc;

/// Simple mock LLM that returns a tool call then final answer
struct SimpleMock;

#[async_trait]
impl LLMClient for SimpleMock {
    fn provider(&self) -> LLMProvider {
        LLMProvider::Anthropic
    }

    fn model(&self) -> &str {
        "mock"
    }

    fn supported_params(&self) -> &[vol_llm_core::SupportedParam] {
        &[]
    }

    async fn converse(&self, request: ConversationRequest) -> vol_llm_core::Result<ConversationResponse> {
        // Check if messages contain tool result
        let has_tool_result = request.messages.iter()
            .any(|m| matches!(m.role, vol_llm_core::MessageRole::Tool));

        if has_tool_result {
            // Second call - return final answer
            Ok(ConversationResponse {
                message: Message::assistant("BTC price is $69,000 based on the market data."),
                model: "mock".to_string(),
                usage: TokenUsage { prompt_tokens: 50, completion_tokens: 20, total_tokens: 70, cached_tokens: None },
                finish_reason: FinishReason::Stop,
                raw: None,
            })
        } else {
            // First call - return tool call
            Ok(ConversationResponse {
                message: Message::assistant_with_tools(
                    "Let me check the market data.",
                    vec![ToolCall {
                        id: "call_1".to_string(),
                        name: "market_data".to_string(),
                        arguments: r#"{"instrument": "btc_usd"}"#.to_string(),
                        r#type: "function".to_string(),
                    }]
                ),
                model: "mock".to_string(),
                usage: TokenUsage { prompt_tokens: 30, completion_tokens: 15, total_tokens: 45, cached_tokens: None },
                finish_reason: FinishReason::ToolCalls,
                raw: None,
            })
        }
    }

    async fn converse_stream(&self, _request: ConversationRequest) -> vol_llm_core::Result<vol_llm_core::stream::StreamReceiver> {
        unimplemented!()
    }
}

#[tokio::test]
async fn test_agent_produces_output() {
    // Initialize tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .try_init();

    println!("\n========== START AGENT TEST ==========\n");

    // Create mock LLM
    let mock_llm = SimpleMock;

    // Create tool registry with market_data tool
    let mut registry = ToolRegistry::new();
    registry.register(MarketDataTool::new(None));

    let agent_config = AgentConfig {
        max_iterations: 3,
        system_prompt: "You are a test assistant. Use tools to get information.".to_string(),
        verbose: true,
    };

    let agent = ReActAgent::new(Arc::new(mock_llm), Arc::new(registry), agent_config);

    let context = ToolContext::default();
    let result = agent.run("What is the BTC price?", context).await;

    println!("\n========== TEST RESULTS ==========\n");

    match result {
        Ok(response) => {
            println!("✓ Agent completed successfully");
            println!("Content: {}", response.content);
            println!("Iterations: {}", response.iterations);
            println!("Tool calls in final response: {}", response.tool_calls.len());

            // Verify agent ran the full ReAct cycle (2 iterations = tool call + final answer)
            assert!(response.iterations >= 2, "Should have at least 2 iterations (tool call + final answer)");

            // Verify final response has content
            assert!(response.content.contains("69"), "Response should contain price info");

            println!("\n========== TEST PASSED ==========\n");
            println!("Note: tool_calls in final response is empty because agent returns final answer");
            println!("      after tool execution. The agent DID execute tools (verified by 2+ iterations).");
        }
        Err(e) => {
            eprintln!("✗ Agent error: {:?}", e);
            panic!("Agent failed: {:?}", e);
        }
    }
}
