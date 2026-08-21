//! Code Agent API simulation test.
//!
//! Run with: cargo test --test code_agent_simulation -- --nocapture
//!
//! This test simulates a real Code Agent calling the LLM API with proper request/response format.
//!
//! Agent-run tests use `StubTool` stand-ins for the deleted TDengine-backed
//! tools: deterministic `execute`, no external service. Keeps agent-run tests
//! fast and green everywhere.

use async_trait::async_trait;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use vol_llm_agent::react::plugin::{PluginDecision, RunContext};
use vol_llm_agent::{AgentConfig, ReActAgent};
use vol_llm_core::{
    ConversationRequest, ConversationResponse, FinishReason, LLMClient, LLMProvider, Message,
    MessageRole, StreamEvent, StreamEventData, SupportedParam, TokenUsage,
};
use vol_llm_tool::{ExecutableTool, ToolContext, ToolRegistry, ToolResult, ToolResultType};

/// Local stand-in for a TDengine-backed tool: deterministic `execute`,
/// no external service. Keeps agent-run tests fast and green everywhere.
struct StubTool(&'static str);

#[async_trait]
impl ExecutableTool for StubTool {
    fn name(&self) -> &'static str {
        self.0
    }

    fn description(&self) -> &'static str {
        "Local stub tool (no external service)"
    }

    async fn execute(
        &self,
        _args: &serde_json::Value,
        _context: &ToolContext,
    ) -> ToolResultType<ToolResult> {
        Ok(ToolResult::success("stub result"))
    }
}

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
            let user_query = request
                .messages
                .iter()
                .find(|m| m.role == MessageRole::User)
                .and_then(|m| m.content.as_ref())
                .map(vol_llm_core::MessageContent::as_str)
                .unwrap_or("");

            let query_lower = user_query.to_lowercase();

            // Priority: volatility/iv queries
            if query_lower.contains("volatility") || query_lower.contains("iv") {
                // Return tool call for volatility_index
                let tool_call = vol_llm_core::ToolCall {
                    id: "toolu_volatility123456".to_string(),
                    name: "volatility_index".to_string(),
                    arguments: r#"{"symbol": "btc_usd", "limit": 10, "hours": 24}"#.to_string(),
                    r#type: "function".to_string(),
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
            if query_lower.contains("price")
                || query_lower.contains("market")
                || query_lower.contains("btc")
                || query_lower.contains("eth")
            {
                // Return tool call for index_price
                let tool_call = vol_llm_core::ToolCall {
                    id: "toolu_01234567890abcdef".to_string(),
                    name: "index_price".to_string(),
                    arguments: r#"{"instrument": "btc_usd", "limit": 1}"#.to_string(),
                    r#type: "function".to_string(),
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
            let tool_results: Vec<&Message> = request
                .messages
                .iter()
                .filter(|m| m.role == MessageRole::Tool)
                .collect();

            if !tool_results.is_empty() {
                // Analyze tool results and generate natural language response
                let tool_content: Vec<&str> = tool_results
                    .iter()
                    .filter_map(|m| m.content.as_ref().map(vol_llm_core::MessageContent::as_str))
                    .collect();

                // Check which tool was called based on content
                let first_content = tool_content.first().copied().unwrap_or("");

                let response_text = if first_content.contains("index_price")
                    || first_content.contains("price")
                    || first_content.contains("Index")
                {
                    "Based on the latest market data, BTC is currently trading at approximately $69,000.
                    This price reflects the most recent index price from our data source."
                } else if first_content.contains("volatility")
                    || first_content.contains("Volatility")
                {
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

    async fn converse(
        &self,
        request: ConversationRequest,
    ) -> vol_llm_core::Result<ConversationResponse> {
        // Simulate API call with realistic response
        // Note: converse_stream is used by the agent, this is just for completeness
        Ok(self.generate_tool_response(&request))
    }

    async fn converse_stream(
        &self,
        request: ConversationRequest,
    ) -> vol_llm_core::Result<vol_llm_core::stream::StreamReceiver> {
        // Use the same logic as converse but emit streaming events
        let response = self.generate_tool_response(&request);
        let (tx, rx) = mpsc::channel(10);

        tokio::spawn(async move {
            // Emit content from the response
            if let Some(content) = response.message.content {
                let _ = tx
                    .send(Ok(StreamEvent {
                        id: "event_1".to_string(),
                        data: StreamEventData::ContentComplete {
                            content: content.as_str().to_string(),
                        },
                    }))
                    .await;
            }

            // Emit tool calls if present
            if let Some(tool_calls) = response.message.tool_calls {
                for tool_call in tool_calls {
                    let _ = tx
                        .send(Ok(StreamEvent {
                            id: "event_tool".to_string(),
                            data: StreamEventData::ToolCallComplete { tool_call },
                        }))
                        .await;
                }
            }
        });

        Ok(vol_llm_core::stream::StreamReceiver::new(rx))
    }
}

// ============================================================================
// Test Cases
// ============================================================================

#[tokio::test]
async fn test_code_agent_market_data_query() {
    println!("\n=== Test: Market Data Query ===\n");

    // Deterministic mock: first LLM call scripts an index_price tool call,
    // second call returns the final answer. No query-text parsing.
    use vol_llm_core::test_utils::MockLlmClient;
    use vol_llm_core::{StreamEvent, StreamEventData, ToolCall};

    let mock_llm = MockLlmClient::new();
    mock_llm
        .set_stream_event_queue(vec![
            vec![
                StreamEvent {
                    id: "event_1".to_string(),
                    data: StreamEventData::ToolCallComplete {
                        tool_call: ToolCall {
                            id: "call_market_data".to_string(),
                            name: "index_price".to_string(),
                            arguments: r#"{"instrument": "btc_usd", "limit": 1}"#.to_string(),
                            r#type: "function".to_string(),
                        },
                    },
                },
                StreamEvent {
                    id: "event_2".to_string(),
                    data: StreamEventData::ContentComplete {
                        content: "Let me check the current market data for you.".to_string(),
                    },
                },
            ],
            vec![StreamEvent {
                id: "event_3".to_string(),
                data: StreamEventData::ContentComplete {
                    content: "Based on the latest market data, BTC is trading at $69,000."
                        .to_string(),
                },
            }],
        ])
        .await;

    // Track tool calls via plugin
    use vol_llm_agent::react::plugin::AgentPlugin;
    use vol_llm_agent::AgentStreamEvent;

    struct ToolCallTracker {
        calls: Arc<tokio::sync::Mutex<Vec<String>>>,
    }

    #[async_trait]
    impl AgentPlugin for ToolCallTracker {
        fn id(&self) -> String {
            "tool_tracker".to_string()
        }

        fn priority(&self) -> u32 {
            100
        }

        async fn intercept(&self, _event: &AgentStreamEvent, _ctx: &RunContext) -> PluginDecision {
            PluginDecision::Continue
        }

        async fn listen(&self, event: &AgentStreamEvent, _ctx: &RunContext) {
            if let AgentStreamEvent::ToolCallComplete { tool_name, .. } = event {
                let mut calls = self.calls.lock().await;
                calls.push(tool_name.clone());
            }
        }
    }

    let tool_calls = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let tracker = ToolCallTracker {
        calls: tool_calls.clone(),
    };

    let config = AgentConfig::builder()
        .with_llm(Arc::new(mock_llm))
        .with_tool(StubTool("volatility_index"))
        .with_tool(StubTool("index_price"))
        .with_tool(StubTool("options"))
        .with_tool(StubTool("rv"))
        .with_system_prompt("You are a code analysis assistant.".to_string())
        .with_plugin(tracker)
        .build()
        .unwrap();
    let base_tools = config.tools.clone();
    let skill_loader = config.skill_loader.clone();
    let agent = ReActAgent::new(config, base_tools, skill_loader);

    // Test: Query BTC price

    agent.run("What is the current BTC price?").await.unwrap();

    // Verify tool was called
    let calls = tool_calls.lock().await;
    println!("Tools called: {:?}", *calls);
    assert!(!calls.is_empty(), "Should call at least one tool");
    assert!(
        calls.contains(&"index_price".to_string()),
        "Should call index_price tool"
    );
}

#[tokio::test]
async fn test_code_agent_volatility_query() {
    println!("\n=== Test: Volatility Query ===\n");

    let mock_llm = CodeAgentSimulator::new("claude-sonnet-4-6");

    let config = AgentConfig::builder()
        .with_llm(Arc::new(mock_llm))
        .with_tool(StubTool("volatility_index"))
        .with_tool(StubTool("index_price"))
        .with_tool(StubTool("options"))
        .with_tool(StubTool("rv"))
        .with_system_prompt("You are a volatility analysis assistant.".to_string())
        .build()
        .unwrap();
    let base_tools = config.tools.clone();
    let skill_loader = config.skill_loader.clone();
    let agent = ReActAgent::new(config, base_tools, skill_loader);

    // Test: Query volatility

    agent.run("Show me ETH volatility").await.unwrap();

    println!("Agent completed successfully");
}

#[tokio::test]
async fn test_code_agent_multi_turn_conversation() {
    println!("\n=== Test: Multi-turn Conversation ===\n");

    let mock_llm = CodeAgentSimulator::new("claude-sonnet-4-6");

    let config = AgentConfig::builder()
        .with_llm(Arc::new(mock_llm))
        .with_tool(StubTool("volatility_index"))
        .with_tool(StubTool("index_price"))
        .with_tool(StubTool("options"))
        .with_tool(StubTool("rv"))
        .with_system_prompt("You are a helpful market data assistant.".to_string())
        .build()
        .unwrap();
    let base_tools = config.tools.clone();
    let skill_loader = config.skill_loader.clone();
    let agent = ReActAgent::new(config, base_tools, skill_loader);

    // Test: Multi-turn with follow-up

    agent
        .run("What is the BTC price and how does it compare to ETH?")
        .await
        .unwrap();

    println!("Agent completed multi-turn conversation");
}

#[tokio::test]
async fn test_code_agent_tool_choice_auto() {
    println!("\n=== Test: Tool Choice Auto ===\n");

    let mock_llm = CodeAgentSimulator::new("claude-sonnet-4-6");

    let config = AgentConfig::builder()
        .with_llm(Arc::new(mock_llm))
        .with_tool(StubTool("volatility_index"))
        .with_tool(StubTool("index_price"))
        .with_tool(StubTool("options"))
        .with_tool(StubTool("rv"))
        .with_system_prompt("You are a helpful assistant.".to_string())
        .build()
        .unwrap();
    let base_tools = config.tools.clone();
    let skill_loader = config.skill_loader.clone();
    let agent = ReActAgent::new(config, base_tools, skill_loader);

    // Test: Simple greeting (may not need tools)

    agent.run("Hello, can you help me?").await.unwrap();

    println!("Agent responded to greeting");
}

#[tokio::test]
async fn test_code_agent_tool_definitions() {
    println!("\n=== Test: Tool Definitions Verification ===\n");

    // Verify tools are properly registered
    let mut registry = ToolRegistry::new();
    registry.register(StubTool("volatility_index"));
    registry.register(StubTool("index_price"));
    registry.register(StubTool("options"));
    registry.register(StubTool("rv"));

    let tools = registry.definitions();

    println!("Registered tools:");
    for tool in &tools {
        println!(
            "  - {}: {}",
            tool.name,
            tool.description.as_deref().unwrap_or("No description")
        );
        if let Some(params) = &tool.parameters {
            println!("    Parameters: {params}");
        }
    }

    // Verify all 4 tools are registered
    assert_eq!(tools.len(), 4, "Should have 4 tools");

    // Verify tool names
    let tool_names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    assert!(tool_names.contains(&"volatility_index"));
    assert!(tool_names.contains(&"index_price"));
    assert!(tool_names.contains(&"options"));
    assert!(tool_names.contains(&"rv"));

    println!("All tools properly registered");
}
