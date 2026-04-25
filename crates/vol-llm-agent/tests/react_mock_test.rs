//! ReAct Agent streaming workflow test.
//!
//! Run with: cargo test --test react_mock_test -- --nocapture
//!
//! This test verifies the ReAct Agent streaming workflow using a simple mock.

use async_trait::async_trait;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use vol_llm_agent::react::plugin::{AgentPlugin, PluginDecision};
use vol_llm_agent::react::PluginContext;
use vol_llm_agent::{AgentStreamEvent, ReActAgent};
use vol_llm_core::{ConversationRequest, ConversationResponse, LLMClient, LLMProvider};
use vol_llm_tdengine::{IndexPriceTool, OptionsTool, RvTool, VolatilityIndexTool};

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

    async fn converse(
        &self,
        _request: ConversationRequest,
    ) -> vol_llm_core::Result<ConversationResponse> {
        unimplemented!("Use converse_stream instead")
    }

    async fn converse_stream(
        &self,
        _request: ConversationRequest,
    ) -> vol_llm_core::Result<vol_llm_core::stream::StreamReceiver> {
        use tokio::sync::mpsc;
        use vol_llm_core::{StreamEvent, StreamEventData};

        let count = self.call_count.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = mpsc::channel(10);

        tokio::spawn(async move {
            if count == 0 {
                // First call: return tool call
                let _ = tx
                    .send(Ok(StreamEvent {
                        id: "event_1".to_string(),
                        data: StreamEventData::ToolCallComplete {
                            tool_call: vol_llm_core::ToolCall {
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
                        id: "event_2".to_string(),
                        data: StreamEventData::ContentComplete {
                            content: "Let me check the market data.".to_string(),
                        },
                    }))
                    .await;
            } else {
                // Second call: return final answer
                let _ = tx
                    .send(Ok(StreamEvent {
                        id: "event_3".to_string(),
                        data: StreamEventData::ContentComplete {
                            content: "The BTC price is $69,000.".to_string(),
                        },
                    }))
                    .await;
            }
        });

        Ok(vol_llm_core::StreamReceiver::new(rx))
    }
}

#[tokio::test]
async fn test_agent_executes_full_react_cycle() {
    // Track tool calls via a counting plugin
    use vol_llm_agent::react::plugin::{AgentPlugin, PluginDecision};

    struct ToolCallCounter {
        count: Arc<AtomicUsize>,
    }

    impl Clone for ToolCallCounter {
        fn clone(&self) -> Self {
            Self {
                count: self.count.clone(),
            }
        }
    }

    #[async_trait]
    impl AgentPlugin for ToolCallCounter {
        fn id(&self) -> String {
            "tool_counter".to_string()
        }

        fn priority(&self) -> u32 {
            100
        }

        async fn intercept(
            &self,
            _event: &AgentStreamEvent,
            _ctx: &PluginContext,
        ) -> PluginDecision {
            PluginDecision::Continue
        }

        async fn listen(&self, event: &AgentStreamEvent, _ctx: &PluginContext) {
            if let AgentStreamEvent::ToolCallBegin { .. } = event {
                self.count.fetch_add(1, Ordering::SeqCst);
            }
        }
    }

    let tool_counter = ToolCallCounter {
        count: Arc::new(AtomicUsize::new(0)),
    };

    let mock_llm = SimpleMock::new();

    let agent = ReActAgent::builder()
        .with_llm(Arc::new(mock_llm))
        .with_tool(IndexPriceTool::new(None))
        .with_tool(VolatilityIndexTool::new(None))
        .with_tool(OptionsTool::new(None))
        .with_tool(RvTool::new(None))
        .with_plugin(tool_counter.clone())
        .with_max_iterations(5)
        .with_system_prompt("You are a test assistant.".to_string())
        .build()
        .unwrap();

    agent.run("What is the BTC price?").await.unwrap();

    // Verify tool was called via plugin counter
    let tool_calls = tool_counter.count.load(Ordering::SeqCst);
    assert_eq!(tool_calls, 1, "Agent should have called one tool");
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

        async fn converse(
            &self,
            _request: ConversationRequest,
        ) -> vol_llm_core::Result<ConversationResponse> {
            unimplemented!("Use converse_stream instead")
        }

        async fn converse_stream(
            &self,
            _request: ConversationRequest,
        ) -> vol_llm_core::Result<vol_llm_core::stream::StreamReceiver> {
            use tokio::sync::mpsc;
            use vol_llm_core::{StreamEvent, StreamEventData};

            self.call_count.fetch_add(1, Ordering::SeqCst);

            let (tx, rx) = mpsc::channel(10);

            tokio::spawn(async move {
                // Always return tool call
                let _ = tx
                    .send(Ok(StreamEvent {
                        id: "event_1".to_string(),
                        data: StreamEventData::ToolCallComplete {
                            tool_call: vol_llm_core::ToolCall {
                                id: "call_loop".to_string(),
                                name: "index_price".to_string(),
                                arguments: r#"{"instrument": "btc_usd", "limit": 1}"#.to_string(),
                                r#type: "function".to_string(),
                            },
                        },
                    }))
                    .await;
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
        .build()
        .unwrap();

    // Agent should return MaxIterationsReached error when max_iterations is exceeded
    let result = agent.run("Keep querying...").await;

    match result {
        Err(vol_llm_agent::AgentError::MaxIterationsReached { max }) => {
            println!("Correctly hit max iterations: {}", max);
            assert_eq!(max, 3);
        }
        Err(e) => panic!("Expected MaxIterationsReached, got: {:?}", e),
        Ok(_) => panic!("Expected MaxIterationsReached error but got Ok"),
    }
}
