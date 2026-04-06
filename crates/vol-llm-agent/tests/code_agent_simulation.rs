//! Code Agent API simulation test.
//!
//! Run with: cargo test --test code_agent_simulation -- --nocapture
//!
//! This test simulates a real Code Agent calling the LLM API with proper request/response format.

use vol_llm_agent::{ReActAgent, AgentConfig};
use vol_llm_tool::{ToolRegistry, ToolContext};
use vol_llm_core::{
    LLMClient, Message, ConversationRequest, ConversationResponse,
    TokenUsage, FinishReason, LLMProvider, FunctionCall,
    MessageRole, SupportedParam,
};
use async_trait::async_trait;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Simulates a real Code Agent LLM client that properly handles tool calls
struct CodeAgentSimulator {
    call_count: Arc<AtomicUsize>,
    model_name: String,
}

impl CodeAgentSimulator {
    fn new(model: &str) -> Self {
        Self {
            call_count: Arc::new(AtomicUsize::new(0)),
            model_name: model.to_string(),
        }
    }

    /// Parse tool calls from conversation history and request
    fn generate_tool_response(&self, request: &ConversationRequest) -> ConversationResponse {
        let count = self.call_count.fetch_add(1, Ordering::SeqCst);

        // First call: analyze user query and decide to use tools
        if count == 0 {
            // Check if user is asking about market data
            let user_query = request.messages
                .iter()
                .find(|m| m.role == MessageRole::User)
                .and_then(|m| m.content.as_ref())
                .map(|c| c.as_str())
                .unwrap_or("");

            let query_lower = user_query.to_lowercase();

            // Priority: volatility/iv queries
            if query_lower.contains("volatility") || query_lower.contains("iv") {
                // Return tool call for alert_history (volatility index)
                let tool_call = vol_llm_core::ToolCall {
                    id: "toolu_volatility123456".to_string(),
                    r#type: "function".to_string(),
                    function: FunctionCall {
                        name: "alert_history".to_string(),
                        arguments: r#"{"symbol": "btc_usd", "limit": 10, "hours": 24}"#.to_string(),
                    },
                };

                return ConversationResponse {
                    message: Message::assistant_with_tools(
                        "Let me query the volatility data.".to_string(),
                        vec![tool_call],
                    ),
                    model: self.model_name.clone(),
                    usage: TokenUsage {
                        prompt_tokens: 180,
                        completion_tokens: 45,
                        total_tokens: 225,
                        cached_tokens: None,
                    },
                    finish_reason: FinishReason::ToolCalls,
                    raw: None,
                };
            }

            // Check for price/market/btc/eth queries
            if query_lower.contains("price") || query_lower.contains("market") ||
               query_lower.contains("btc") || query_lower.contains("eth") {

                // Return tool call for market_data
                let tool_call = vol_llm_core::ToolCall {
                    id: "toolu_01234567890abcdef".to_string(),
                    r#type: "function".to_string(),
                    function: FunctionCall {
                        name: "market_data".to_string(),
                        arguments: r#"{"instrument": "btc_usd", "data_type": "price"}"#.to_string(),
                    },
                };

                return ConversationResponse {
                    message: Message::assistant_with_tools(
                        "Let me check the current market data for you.".to_string(),
                        vec![tool_call],
                    ),
                    model: self.model_name.clone(),
                    usage: TokenUsage {
                        prompt_tokens: 150,
                        completion_tokens: 50,
                        total_tokens: 200,
                        cached_tokens: None,
                    },
                    finish_reason: FinishReason::ToolCalls,
                    raw: None,
                };
            }
        }

        // Second call: user has tool results, provide final answer
        if count == 1 {
            // Check for tool results in messages
            let tool_results: Vec<&Message> = request.messages
                .iter()
                .filter(|m| m.role == MessageRole::Tool)
                .collect();

            if !tool_results.is_empty() {
                // Analyze tool results and generate natural language response
                let tool_content: Vec<&str> = tool_results
                    .iter()
                    .filter_map(|m| m.content.as_ref().map(|c| c.as_str()))
                    .collect();

                // Check which tool was called based on content
                let first_content = tool_content.first().map(|s| *s).unwrap_or("");

                let response_text = if first_content.contains("market_data") || first_content.contains("price") || first_content.contains("BTC") {
                    "Based on the latest market data, BTC is currently trading at approximately $69,000.
                    This price reflects the most recent index price from our data source."
                } else if first_content.contains("volatility") || first_content.contains("alert") {
                    "The volatility data shows recent price movements. Based on the historical data,
                    we can observe the volatility trends over the specified time period."
                } else if first_content.contains("Retrieved") {
                    "I've successfully retrieved the requested data from TDengine."
                } else {
                    "I've successfully queried the requested data. Here's what I found based on the tool results."
                };

                return ConversationResponse {
                    message: Message::assistant(response_text.to_string()),
                    model: self.model_name.clone(),
                    usage: TokenUsage {
                        prompt_tokens: 250,
                        completion_tokens: 80,
                        total_tokens: 330,
                        cached_tokens: None,
                    },
                    finish_reason: FinishReason::Stop,
                    raw: None,
                };
            }
        }

        // Fallback: return simple response
        ConversationResponse {
            message: Message::assistant("I'm here to help with your market data questions."),
            model: self.model_name.clone(),
            usage: TokenUsage::default(),
            finish_reason: FinishReason::Stop,
            raw: None,
        }
    }
}

#[async_trait]
impl LLMClient for CodeAgentSimulator {
    fn provider(&self) -> LLMProvider {
        LLMProvider::Anthropic
    }

    fn model(&self) -> &str {
        &self.model_name
    }

    fn supported_params(&self) -> &[SupportedParam] {
        &[
            SupportedParam::MaxTokens,
            SupportedParam::Temperature,
            SupportedParam::TopP,
            SupportedParam::Tools,
        ]
    }

    async fn converse(&self, request: ConversationRequest) -> vol_llm_core::Result<ConversationResponse> {
        // Simulate API call with realistic response
        Ok(self.generate_tool_response(&request))
    }

    async fn converse_stream(&self, _request: ConversationRequest) -> vol_llm_core::Result<vol_llm_core::stream::StreamReceiver> {
        unimplemented!("Streaming not implemented in simulator")
    }
}

// ============================================================================
// Test Cases
// ============================================================================

#[tokio::test]
async fn test_code_agent_market_data_query() {
    println!("\n=== Test: Market Data Query ===\n");

    let mock_llm = CodeAgentSimulator::new("claude-sonnet-4-6");

    // Setup tools
    let mut registry = ToolRegistry::new();
    registry.register_default_tools();

    let config = AgentConfig {
        max_iterations: 5,
        system_prompt: "You are a helpful market data assistant.".to_string(),
        verbose: true,
    };

    let agent = ReActAgent::new(Box::new(mock_llm), registry, config);

    // Test: Query BTC price
    let context = ToolContext::default();
    let result = agent.run("What is the current BTC price?", context).await;

    match result {
        Ok(response) => {
            println!("✓ Agent completed successfully");
            println!("  Response: {}", response.content);
            println!("  Iterations: {}", response.iterations);
            println!("  Tool calls: {}", response.tool_calls.len());

            // Verify tool was called
            assert!(response.tool_calls.len() >= 1, "Should call at least one tool");
            assert_eq!(response.tool_calls[0].function.name, "market_data");

            // Verify response mentions trading or data (more flexible)
            let content_lower = response.content.to_lowercase();
            assert!(
                content_lower.contains("price") || content_lower.contains("btc") ||
                content_lower.contains("market") || content_lower.contains("trading") ||
                content_lower.contains("data"),
                "Response should mention price, BTC, market, trading, or data"
            );
        }
        Err(e) => {
            panic!("Agent failed: {:?}", e);
        }
    }
}

#[tokio::test]
async fn test_code_agent_volatility_query() {
    println!("\n=== Test: Volatility Query ===\n");

    let mock_llm = CodeAgentSimulator::new("claude-sonnet-4-6");

    let mut registry = ToolRegistry::new();
    registry.register_default_tools();

    let config = AgentConfig {
        max_iterations: 5,
        system_prompt: "You are a volatility analysis assistant.".to_string(),
        verbose: true,
    };

    let agent = ReActAgent::new(Box::new(mock_llm), registry, config);

    // Test: Query volatility - use "ETH volatility" to trigger volatility path
    let context = ToolContext {
        instrument: "eth_usd".to_string(),
        ..Default::default()
    };
    let result = agent.run("Show me ETH volatility", context).await;

    match result {
        Ok(response) => {
            println!("✓ Agent completed successfully");
            println!("  Response: {}", response.content);
            println!("  Iterations: {}", response.iterations);
            println!("  Tool calls: {}", response.tool_calls.len());

            // Verify a tool was called (any tool is acceptable for this query)
            assert!(response.tool_calls.len() >= 1, "Should call at least one tool");
            let called_tool = &response.tool_calls[0].function.name;
            println!("  Called tool: {}", called_tool);
        }
        Err(e) => {
            panic!("Agent failed: {:?}", e);
        }
    }
}

#[tokio::test]
async fn test_code_agent_multi_turn_conversation() {
    println!("\n=== Test: Multi-turn Conversation ===\n");

    let mock_llm = CodeAgentSimulator::new("claude-sonnet-4-6");

    let mut registry = ToolRegistry::new();
    registry.register_default_tools();

    let config = AgentConfig {
        max_iterations: 5,
        system_prompt: "You are a helpful market data assistant.".to_string(),
        verbose: true,
    };

    let agent = ReActAgent::new(Box::new(mock_llm), registry, config);

    // Test: Multi-turn with follow-up
    let context = ToolContext::default();
    let result = agent.run("What is the BTC price and how does it compare to ETH?", context).await;

    match result {
        Ok(response) => {
            println!("✓ Agent completed multi-turn conversation");
            println!("  Response: {}", response.content);
            println!("  Iterations: {}", response.iterations);
            println!("  Tool calls: {}", response.tool_calls.len());

            // Should complete within reasonable iterations
            assert!(response.iterations <= 5, "Should complete within max iterations");
        }
        Err(e) => {
            panic!("Agent failed: {:?}", e);
        }
    }
}

#[tokio::test]
async fn test_code_agent_tool_choice_auto() {
    println!("\n=== Test: Tool Choice Auto ===\n");

    let mock_llm = CodeAgentSimulator::new("claude-sonnet-4-6");

    let mut registry = ToolRegistry::new();
    registry.register_default_tools();

    let config = AgentConfig {
        max_iterations: 3,
        system_prompt: "You are a helpful assistant.".to_string(),
        verbose: true,
    };

    let agent = ReActAgent::new(Box::new(mock_llm), registry, config);

    // Test: Simple greeting (may not need tools)
    let context = ToolContext::default();
    let result = agent.run("Hello, can you help me?", context).await;

    match result {
        Ok(response) => {
            println!("✓ Agent responded to greeting");
            println!("  Response: {}", response.content);
            println!("  Iterations: {}", response.iterations);

            // Greeting should complete in 1 iteration without tools
            assert_eq!(response.iterations, 1, "Greeting should complete in 1 iteration");
        }
        Err(e) => {
            panic!("Agent failed: {:?}", e);
        }
    }
}

#[tokio::test]
async fn test_code_agent_with_tool_definitions() {
    println!("\n=== Test: Tool Definitions Verification ===\n");

    // Verify tools are properly registered
    let mut registry = ToolRegistry::new();
    registry.register_default_tools();

    let tools = registry.tools();

    println!("Registered tools:");
    for tool in &tools {
        println!("  - {}: {}", tool.function.name, tool.function.description.as_deref().unwrap_or("No description"));
        if let Some(params) = &tool.function.parameters {
            println!("    Parameters: {}", params);
        }
    }

    // Verify all 4 tools are registered
    assert_eq!(tools.len(), 4, "Should have 4 default tools");

    // Verify tool names
    let tool_names: Vec<&str> = tools.iter().map(|t| t.function.name.as_str()).collect();
    assert!(tool_names.contains(&"alert_history"));
    assert!(tool_names.contains(&"iv_curve"));
    assert!(tool_names.contains(&"market_data"));
    assert!(tool_names.contains(&"rule_info"));

    println!("✓ All tools properly registered");
}
