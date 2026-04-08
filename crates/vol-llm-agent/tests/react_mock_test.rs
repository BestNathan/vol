//! ReAct Agent streaming workflow test.
//!
//! Run with: cargo test --test react_mock_test -- --nocapture
//!
//! This test verifies the ReAct Agent streaming workflow using a simple mock.

use vol_llm_agent::{ReActAgent, AgentConfig, AgentStreamEvent, react::plugin::PluginRegistry};
use vol_llm_tool::{ToolRegistry, ToolContext};
use vol_llm_tdengine::{VolatilityIndexTool, IndexPriceTool, OptionsTool, RvTool};
use vol_llm_core::{LLMClient, Message, ConversationRequest, ConversationResponse, TokenUsage, FinishReason, LLMProvider};
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

    async fn converse(&self, _request: ConversationRequest) -> vol_llm_core::Result<ConversationResponse> {
        unimplemented!("Use converse_stream instead")
    }

    async fn converse_stream(&self, request: ConversationRequest) -> vol_llm_core::Result<vol_llm_core::stream::StreamReceiver> {
        use tokio::sync::mpsc;
        use vol_llm_core::{StreamEvent, StreamEventData};

        let count = self.call_count.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = mpsc::channel(10);

        tokio::spawn(async move {
            if count == 0 {
                // First call: return tool call
                let _ = tx.send(Ok(StreamEvent {
                    id: "event_1".to_string(),
                    data: StreamEventData::ToolCallComplete {
                        tool_call: vol_llm_core::ToolCall {
                            id: "call_1".to_string(),
                            name: "index_price".to_string(),
                            arguments: r#"{"instrument": "btc_usd", "limit": 1}"#.to_string(),
                            r#type: "function".to_string(),
                        },
                    },
                })).await;
                let _ = tx.send(Ok(StreamEvent {
                    id: "event_2".to_string(),
                    data: StreamEventData::ContentComplete {
                        content: "Let me check the market data.".to_string(),
                    },
                })).await;
            } else {
                // Second call: return final answer
                let _ = tx.send(Ok(StreamEvent {
                    id: "event_3".to_string(),
                    data: StreamEventData::ContentComplete {
                        content: "The BTC price is $69,000.".to_string(),
                    },
                })).await;
            }
        });

        Ok(vol_llm_core::StreamReceiver::new(rx))
    }
}

#[tokio::test]
async fn test_agent_executes_full_react_cycle() {
    let mock_llm = SimpleMock::new();

    // Create tool registry with TDengine tools
    let mut registry = ToolRegistry::new();
    registry.register(VolatilityIndexTool::new(None));
    registry.register(IndexPriceTool::new(None));
    registry.register(OptionsTool::new(None));
    registry.register(RvTool::new(None));

    let config = AgentConfig {
        max_iterations: 5,
        max_history_messages: 20,
        system_prompt: "You are a test assistant.".to_string(),
        verbose: true,
        plugin_registry: PluginRegistry::new(),
    };

    let agent = ReActAgent::builder()
        .with_llm(Arc::new(mock_llm))
        .with_tool(IndexPriceTool::new(None))
        .with_tool(VolatilityIndexTool::new(None))
        .with_tool(OptionsTool::new(None))
        .with_tool(RvTool::new(None))
        .with_max_iterations(5)
        .with_system_prompt("You are a test assistant.".to_string())
        .with_verbose(true)
        .build()
        .unwrap();

    let context = ToolContext::default();
    let stream_result = agent.run("What is the BTC price?", context).await;

    match stream_result {
        Ok(mut stream) => {
            let mut final_response = None;
            let mut tool_calls_count = 0;
            let mut iterations = 0u32;

            while let Some(event) = stream.recv().await {
                match event.unwrap() {
                    AgentStreamEvent::ToolCallBegin { tool_name, .. } => {
                        println!("Tool call begin: {}", tool_name);
                        tool_calls_count += 1;
                    }
                    AgentStreamEvent::ToolCallComplete { tool_name, result } => {
                        println!("Tool call complete: {} = {}", tool_name, result);
                    }
                    AgentStreamEvent::IterationComplete { iteration, tool_calls, final_answer } => {
                        println!("Iteration {} complete, tool_calls: {}, final_answer: {:?}", iteration, tool_calls.len(), final_answer);
                        iterations = iteration;
                        if final_answer.is_some() {
                            final_response = final_answer;
                        }
                    }
                    AgentStreamEvent::AgentComplete { response } => {
                        println!("Agent complete: {}", response.content);
                        final_response = Some(response.content.clone());
                        println!("Content: {}", response.content);
                        println!("Iterations: {}", response.iterations);
                        println!("Tool calls: {}", response.tool_calls.len());
                    }
                    _ => {}
                }
            }

            // Verify agent called the tool
            assert_eq!(tool_calls_count, 1, "Agent should have called one tool");

            // Verify iterations (tool call + final response = 2)
            assert!(iterations >= 2, "Should have at least 2 iterations");

            // Verify final response
            assert!(final_response.unwrap().contains("69"), "Response should contain price info");
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
            unimplemented!("Use converse_stream instead")
        }

        async fn converse_stream(&self, _request: ConversationRequest) -> vol_llm_core::Result<vol_llm_core::stream::StreamReceiver> {
            use tokio::sync::mpsc;
            use vol_llm_core::{StreamEvent, StreamEventData};

            self.call_count.fetch_add(1, Ordering::SeqCst);

            let (tx, rx) = mpsc::channel(10);

            tokio::spawn(async move {
                // Always return tool call
                let _ = tx.send(Ok(StreamEvent {
                    id: "event_1".to_string(),
                    data: StreamEventData::ToolCallComplete {
                        tool_call: vol_llm_core::ToolCall {
                            id: "call_loop".to_string(),
                            name: "index_price".to_string(),
                            arguments: r#"{"instrument": "btc_usd", "limit": 1}"#.to_string(),
                            r#type: "function".to_string(),
                        },
                    },
                })).await;
            });

            Ok(vol_llm_core::StreamReceiver::new(rx))
        }
    }

    let mock_llm = LoopMock::new();

    let agent = ReActAgent::builder()
        .with_llm(Arc::new(mock_llm))
        .with_tool(VolatilityIndexTool::new(None))
        .with_tool(IndexPriceTool::new(None))
        .with_tool(OptionsTool::new(None))
        .with_tool(RvTool::new(None))
        .with_max_iterations(3)
        .with_system_prompt("You are a test assistant.".to_string())
        .with_verbose(false)
        .build()
        .unwrap();

    let context = ToolContext::default();
    let stream_result = agent.run("Keep querying...", context).await;

    match stream_result {
        Ok(mut stream) => {
            // Consume stream - should get MaxIterationsReached error
            while let Some(event) = stream.recv().await {
                match event {
                    Err(vol_llm_agent::AgentError::MaxIterationsReached { max }) => {
                        println!("Correctly hit max iterations: {}", max);
                        assert_eq!(max, 3);
                        return;
                    }
                    Ok(_) => continue,
                    Err(e) => panic!("Expected MaxIterationsReached, got: {:?}", e),
                }
            }
            panic!("Expected MaxIterationsReached error");
        }
        Err(e) => {
            panic!("Expected stream but got error: {:?}", e);
        }
    }
}
