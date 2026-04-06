//! ReAct Agent simple workflow test.
//!
//! Run with: cargo test --test react_mock_test -- --nocapture
//!
//! This test verifies the ReAct Agent workflow using a simple mock.

use vol_llm_agent::{ReActAgent, AgentConfig};
use vol_llm_tool::{ToolRegistry, ToolContext};
use vol_llm_core::{LLMClient, Message, ConversationRequest, ConversationResponse, TokenUsage, FinishReason, LLMProvider, FunctionCall};
use async_trait::async_trait;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Simple mock that returns tool call then final answer
struct SimpleMock {
    call_count: Arc<AtomicUsize>,
}

impl SimpleMock {
    fn new() -> Self {
        Self {
            call_count: Arc::new(AtomicUsize::new(0)),
        }
    }
}

#[async_trait]
impl LLMClient for SimpleMock {
    fn provider(&self) -> LLMProvider {
        LLMProvider::Anthropic
    }

    fn model(&self) -> &str {
        "mock-model"
    }

    fn supported_params(&self) -> &[vol_llm_core::SupportedParam] {
        &[]
    }

    async fn converse(&self, request: ConversationRequest) -> vol_llm_core::Result<ConversationResponse> {
        let count = self.call_count.fetch_add(1, Ordering::SeqCst);

        if count == 0 {
            // First call: return tool call
            let tool_call = vol_llm_core::ToolCall {
                id: "call_1".to_string(),
                r#type: "function".to_string(),
                function: FunctionCall {
                    name: "market_data".to_string(),
                    arguments: r#"{"instrument": "btc_usd"}"#.to_string(),
                },
            };

            Ok(ConversationResponse {
                message: Message::assistant_with_tools("Let me check the market data.", vec![tool_call]),
                model: "mock".to_string(),
                usage: TokenUsage::default(),
                finish_reason: FinishReason::ToolCalls,
                raw: None,
            })
        } else {
            // Check if tool result was included
            let has_tool_result = request.messages.iter().any(|m| m.role == vol_llm_core::MessageRole::Tool);

            if has_tool_result {
                // Second call after tool: return final answer
                Ok(ConversationResponse {
                    message: Message::assistant("The BTC price is $69,000."),
                    model: "mock".to_string(),
                    usage: TokenUsage::default(),
                    finish_reason: FinishReason::Stop,
                    raw: None,
                })
            } else {
                // Should not happen - return tool call again
                let tool_call = vol_llm_core::ToolCall {
                    id: "call_2".to_string(),
                    r#type: "function".to_string(),
                    function: FunctionCall {
                        name: "market_data".to_string(),
                        arguments: r#"{"instrument": "btc_usd"}"#.to_string(),
                    },
                };

                Ok(ConversationResponse {
                    message: Message::assistant_with_tools("Checking...", vec![tool_call]),
                    model: "mock".to_string(),
                    usage: TokenUsage::default(),
                    finish_reason: FinishReason::ToolCalls,
                    raw: None,
                })
            }
        }
    }

    async fn converse_stream(&self, _request: ConversationRequest) -> vol_llm_core::Result<vol_llm_core::stream::StreamReceiver> {
        unimplemented!()
    }
}

#[tokio::test]
async fn test_agent_executes_full_react_cycle() {
    let mock_llm = SimpleMock::new();

    // Create tool registry with TDengine tools
    let mut registry = ToolRegistry::new();
    registry.register_default_tools();

    let config = AgentConfig {
        max_iterations: 5,
        system_prompt: "You are a test assistant.".to_string(),
        verbose: true,
    };

    let agent = ReActAgent::new(Box::new(mock_llm), Arc::new(registry), config);

    let context = ToolContext::default();
    let result = agent.run("What is the BTC price?", context).await;

    match result {
        Ok(response) => {
            println!("Agent response: {}", response.content);
            println!("Iterations: {}", response.iterations);
            println!("Tool calls: {}", response.tool_calls.len());

            // Verify agent called the tool
            assert_eq!(response.tool_calls.len(), 1, "Agent should have called one tool");
            assert_eq!(response.tool_calls[0].function.name, "market_data");

            // Verify iterations (tool call + final response = 2)
            assert!(response.iterations >= 2, "Should have at least 2 iterations");

            // Verify final response
            assert!(response.content.contains("69"), "Response should contain price info");
        }
        Err(e) => {
            panic!("Agent failed: {:?}", e);
        }
    }
}

#[tokio::test]
async fn test_agent_max_iterations() {
    // Mock that always returns tool calls
    struct LoopMock {
        call_count: Arc<AtomicUsize>,
    }

    impl LoopMock {
        fn new() -> Self {
            Self {
                call_count: Arc::new(AtomicUsize::new(0)),
            }
        }
    }

    #[async_trait]
    impl LLMClient for LoopMock {
        fn provider(&self) -> LLMProvider {
            LLMProvider::Anthropic
        }

        fn model(&self) -> &str {
            "mock-model"
        }

        fn supported_params(&self) -> &[vol_llm_core::SupportedParam] {
            &[]
        }

        async fn converse(&self, _request: ConversationRequest) -> vol_llm_core::Result<ConversationResponse> {
            self.call_count.fetch_add(1, Ordering::SeqCst);

            let tool_call = vol_llm_core::ToolCall {
                id: "call_loop".to_string(),
                r#type: "function".to_string(),
                function: FunctionCall {
                    name: "market_data".to_string(),
                    arguments: r#"{"instrument": "btc_usd"}"#.to_string(),
                },
            };

            Ok(ConversationResponse {
                message: Message::assistant_with_tools("Still thinking...", vec![tool_call]),
                model: "mock".to_string(),
                usage: TokenUsage::default(),
                finish_reason: FinishReason::ToolCalls,
                raw: None,
            })
        }

        async fn converse_stream(&self, _request: ConversationRequest) -> vol_llm_core::Result<vol_llm_core::stream::StreamReceiver> {
            unimplemented!()
        }
    }

    let mock_llm = LoopMock::new();

    let mut registry = ToolRegistry::new();
    registry.register_default_tools();

    let config = AgentConfig {
        max_iterations: 3,
        system_prompt: "You are a test assistant.".to_string(),
        verbose: false,
    };

    let agent = ReActAgent::new(Box::new(mock_llm), Arc::new(registry), config);

    let context = ToolContext::default();
    let result = agent.run("Keep querying...", context).await;

    match result {
        Err(vol_llm_agent::AgentError::MaxIterationsReached { max }) => {
            println!("Correctly hit max iterations: {}", max);
            assert_eq!(max, 3);
        }
        Err(e) => {
            panic!("Expected MaxIterationsReached, got: {:?}", e);
        }
        Ok(_) => {
            panic!("Expected error but agent completed");
        }
    }
}
