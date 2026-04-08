//! Debug agent output test.
//!
//! Run with: cargo test --test debug_agent_output -- --nocapture
//!
//! This test verifies the agent can produce output using real LLM.

use vol_llm_agent::{ReActAgent, AgentConfig, AgentStreamEvent};
use vol_llm_tool::{ToolRegistry, ToolContext};
use vol_llm_tdengine::{IndexPriceTool};
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

    async fn converse(&self, _request: ConversationRequest) -> vol_llm_core::Result<ConversationResponse> {
        unimplemented!("Use converse_stream instead")
    }

    async fn converse_stream(&self, request: ConversationRequest) -> vol_llm_core::Result<vol_llm_core::stream::StreamReceiver> {
        use tokio::sync::mpsc;
        use vol_llm_core::{StreamEvent, StreamEventData};

        let (tx, rx) = mpsc::channel(10);

        // Check if messages contain tool result
        let has_tool_result = request.messages.iter()
            .any(|m| matches!(m.role, vol_llm_core::MessageRole::Tool));

        tokio::spawn(async move {
            if has_tool_result {
                // Second call - return final answer
                let _ = tx.send(Ok(StreamEvent {
                    id: "event_1".to_string(),
                    data: StreamEventData::ContentComplete {
                        content: "BTC price is $69,000 based on the market data.".to_string(),
                    },
                })).await;
            } else {
                // First call - return tool call
                let _ = tx.send(Ok(StreamEvent {
                    id: "event_2".to_string(),
                    data: StreamEventData::ToolCallComplete {
                        tool_call: ToolCall {
                            id: "call_1".to_string(),
                            name: "index_price".to_string(),
                            arguments: r#"{"instrument": "btc_usd", "limit": 1}"#.to_string(),
                            r#type: "function".to_string(),
                        },
                    },
                })).await;
                let _ = tx.send(Ok(StreamEvent {
                    id: "event_3".to_string(),
                    data: StreamEventData::ContentComplete {
                        content: "Let me check the market data.".to_string(),
                    },
                })).await;
            }
        });

        Ok(vol_llm_core::StreamReceiver::new(rx))
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

    // Create agent with builder
    let agent = ReActAgent::builder()
        .with_llm(Arc::new(mock_llm))
        .with_tool(IndexPriceTool::new(None))
        .with_max_iterations(3)
        .with_system_prompt("You are a test assistant. Use tools to get information.".to_string())
        .with_verbose(true)
        .build()
        .unwrap();

    let context = ToolContext::default();
    let stream_result = agent.run("What is the BTC price?", context).await;

    println!("\n========== TEST RESULTS ==========\n");

    match stream_result {
        Ok(mut stream) => {
            let mut final_response = None;
            let mut iterations = 0u32;
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
                    }
                    _ => {}
                }
            }

            println!("✓ Agent completed successfully");
            println!("Content: {:?}", final_response);
            println!("Iterations: {}", iterations);
            println!("Tool calls: {}", tool_calls_count);

            // Verify agent ran the full ReAct cycle (2 iterations = tool call + final answer)
            assert!(iterations >= 2, "Should have at least 2 iterations (tool call + final answer)");

            // Verify final response has content
            assert!(final_response.unwrap().contains("69"), "Response should contain price info");

            println!("\n========== TEST PASSED ==========\n");
        }
        Err(e) => {
            eprintln!("Agent error: {:?}", e);
            panic!("Agent failed: {:?}", e);
        }
    }
}
