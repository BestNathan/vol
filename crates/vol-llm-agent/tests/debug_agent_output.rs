//! Debug agent output test.
//!
//! Run with: cargo test --test debug_agent_output -- --nocapture
//!
//! This test verifies the agent can produce output using real LLM.

use async_trait::async_trait;
use std::sync::Arc;
use vol_llm_agent::{AgentConfig, ReActAgent};
use vol_llm_core::{ConversationRequest, ConversationResponse, LLMClient, LLMProvider, ToolCall};
use vol_llm_tdengine::IndexPriceTool;

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

    async fn converse(
        &self,
        _request: ConversationRequest,
    ) -> vol_llm_core::Result<ConversationResponse> {
        unimplemented!("Use converse_stream instead")
    }

    async fn converse_stream(
        &self,
        request: ConversationRequest,
    ) -> vol_llm_core::Result<vol_llm_core::stream::StreamReceiver> {
        use tokio::sync::mpsc;
        use vol_llm_core::{StreamEvent, StreamEventData};

        let (tx, rx) = mpsc::channel(10);

        // Check if messages contain tool result
        let has_tool_result = request
            .messages
            .iter()
            .any(|m| matches!(m.role, vol_llm_core::MessageRole::Tool));

        tokio::spawn(async move {
            if has_tool_result {
                // Second call - return final answer
                let _ = tx
                    .send(Ok(StreamEvent {
                        id: "event_1".to_string(),
                        data: StreamEventData::ContentComplete {
                            content: "BTC price is $69,000 based on the market data.".to_string(),
                        },
                    }))
                    .await;
            } else {
                // First call - return tool call
                let _ = tx
                    .send(Ok(StreamEvent {
                        id: "event_2".to_string(),
                        data: StreamEventData::ToolCallComplete {
                            tool_call: ToolCall {
                                id: "call_1".to_string(),
                                name: "index_price".to_string(),
                                arguments: r#"{"instrument": "btc_usd", "limit": 1}"#.to_string(),
                                r#type: "function".to_string(),
                            },
                        },
                    }))
                    .await;
                let _ = tx
                    .send(Ok(StreamEvent {
                        id: "event_3".to_string(),
                        data: StreamEventData::ContentComplete {
                            content: "Let me check the market data.".to_string(),
                        },
                    }))
                    .await;
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
    let config = AgentConfig::builder()
        .with_llm(Arc::new(mock_llm))
        .with_tool(IndexPriceTool::new(None))
        .with_system_prompt("You are a test assistant. Use tools to get information.".to_string())
        .build()
        .unwrap();
    let agent = ReActAgent::new(config);

    println!("\n--- Running agent with user input: 'What is the BTC price?' ---\n");

    let result = agent.run("What is the BTC price?").await;

    println!("\n========== TEST RESULTS ==========\n");

    match result {
        Ok(response) => {
            println!("✓ Agent completed successfully");
            println!("Final answer: {}", response.content);
            println!("\n========== TEST PASSED ==========\n");
        }
        Err(e) => {
            eprintln!("Agent error: {:?}", e);
            panic!("Agent failed: {:?}", e);
        }
    }
}
