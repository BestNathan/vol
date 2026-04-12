//! Agent Message History Test.
//!
//! Run with: cargo test --test agent_message_history_test -- --nocapture
//!
//! This test verifies that tool results are properly passed to subsequent LLM iterations.

use async_trait::async_trait;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;
use vol_llm_agent::{
    react::plugin::{AgentPlugin, PluginDecision},
    react::PluginContext,
    AgentStreamEvent, ReActAgent,
};
use vol_llm_core::{
    ConversationRequest, ConversationResponse, LLMClient, LLMProvider, MessageRole, StreamEvent,
    StreamEventData, ToolCall,
};

/// Tracks all messages sent to the LLM across iterations
struct MessageTracker {
    iterations: Arc<Mutex<Vec<Vec<vol_llm_core::Message>>>>,
}

impl MessageTracker {
    fn new() -> Self {
        Self {
            iterations: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

/// Mock LLM that returns tool call then final answer, tracking all requests
struct TrackingMock {
    call_count: Arc<AtomicUsize>,
    message_tracker: Arc<Mutex<Vec<Vec<vol_llm_core::Message>>>>,
}

impl TrackingMock {
    fn new(tracker: Arc<Mutex<Vec<Vec<vol_llm_core::Message>>>>) -> Self {
        Self {
            call_count: Arc::new(AtomicUsize::new(0)),
            message_tracker: tracker,
        }
    }
}

#[async_trait]
impl LLMClient for TrackingMock {
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
        request: ConversationRequest,
    ) -> vol_llm_core::Result<vol_llm_core::stream::StreamReceiver> {
        use tokio::sync::mpsc;

        // Track messages for this iteration
        self.message_tracker
            .lock()
            .await
            .push(request.messages.clone());

        let count = self.call_count.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = mpsc::channel(10);

        tokio::spawn(async move {
            if count == 0 {
                // First call: return tool call
                let _ = tx
                    .send(Ok(StreamEvent {
                        id: "event_1".to_string(),
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
                        id: "event_2".to_string(),
                        data: StreamEventData::ContentComplete {
                            content: "Let me check the market data.".to_string(),
                        },
                    }))
                    .await;
            } else {
                // Second call: should have tool result in messages, return final answer
                let _ = tx
                    .send(Ok(StreamEvent {
                        id: "event_3".to_string(),
                        data: StreamEventData::ContentComplete {
                            content: "Based on the tool result, BTC price is $69,000.".to_string(),
                        },
                    }))
                    .await;
            }
        });

        Ok(vol_llm_core::StreamReceiver::new(rx))
    }
}

/// Plugin that captures all messages from RunContext
struct MessageCapturePlugin {
    captured_messages: Arc<Mutex<Vec<Vec<vol_llm_core::Message>>>>,
}

#[async_trait]
impl AgentPlugin for MessageCapturePlugin {
    fn id(&self) -> String {
        "message_capture".to_string()
    }

    fn priority(&self) -> u32 {
        100
    }

    async fn intercept(&self, _event: &AgentStreamEvent, _ctx: &PluginContext) -> PluginDecision {
        PluginDecision::Continue
    }

    async fn listen(&self, event: &AgentStreamEvent, ctx: &PluginContext) {
        // Capture messages at key events
        if let AgentStreamEvent::IterationComplete { .. } = event {
            let messages = ctx.get_messages().await;
            self.captured_messages.lock().await.push(messages);
        }
    }
}

#[tokio::test]
async fn test_tool_results_passed_to_next_iteration() {
    println!("\n=== Test: Tool Results in Message History ===\n");

    let message_tracker = Arc::new(Mutex::new(Vec::new()));
    let mock_llm = TrackingMock::new(message_tracker.clone());

    let captured_messages = Arc::new(Mutex::new(Vec::new()));
    let capture_plugin = MessageCapturePlugin {
        captured_messages: captured_messages.clone(),
    };

    let agent = ReActAgent::builder()
        .with_llm(Arc::new(mock_llm))
        .with_tool(vol_llm_tdengine::IndexPriceTool::new(None))
        .with_plugin(capture_plugin)
        .with_max_iterations(3)
        .with_system_prompt("You are a test assistant. Use tools to get information.".to_string())
        .with_verbose(true)
        .build()
        .unwrap();

    agent.run("What is the BTC price?").await.unwrap();

    // Check tracked messages (from LLM perspective)
    let tracked = message_tracker.lock().await;
    println!("LLM was called {} times", tracked.len());
    assert!(
        tracked.len() >= 2,
        "LLM should be called at least twice (tool call + final answer)"
    );

    // Check first iteration (should NOT have tool messages yet)
    let iteration_0 = &tracked[0];
    println!("\n=== Iteration 1 ({} messages) ===", iteration_0.len());
    let mut has_tool_message_iter_0 = false;
    for (idx, msg) in iteration_0.iter().enumerate() {
        println!(
            "  [{}] {:?}: content={:.50}, tool_call_id={:?}",
            idx,
            msg.role,
            msg.content.as_ref().map(|c| c.as_str()).unwrap_or("<none>"),
            msg.tool_call_id
        );
        if msg.role == MessageRole::Tool {
            has_tool_message_iter_0 = true;
        }
    }
    assert!(
        !has_tool_message_iter_0,
        "First iteration should NOT have tool messages"
    );

    // Check second iteration (SHOULD have tool message with correct format)
    let iteration_1 = &tracked[1];
    println!("\n=== Iteration 2 ({} messages) ===", iteration_1.len());
    let mut found_tool_message = false;
    let mut tool_message_correct_format = false;
    let mut found_assistant_with_tool_calls = false;

    for (idx, msg) in iteration_1.iter().enumerate() {
        println!(
            "  [{}] {:?}: content={:.80}, tool_call_id={:?}, tool_calls={}",
            idx,
            msg.role,
            msg.content.as_ref().map(|c| c.as_str()).unwrap_or("<none>"),
            msg.tool_call_id,
            msg.tool_calls.as_ref().map(|v| v.len()).unwrap_or(0)
        );

        if msg.role == MessageRole::Tool {
            found_tool_message = true;
            // Verify correct format: tool message should have tool_call_id set
            if msg.tool_call_id.is_some() && msg.tool_call_id.as_ref().unwrap() == "call_1" {
                tool_message_correct_format = true;
                println!("      ^^^ CORRECT: Tool message has correct tool_call_id!");
            }
        }

        // Verify assistant message has tool_calls
        if msg.role == MessageRole::Assistant
            && msg.tool_calls.as_ref().map(|v| v.len()).unwrap_or(0) > 0
        {
            found_assistant_with_tool_calls = true;
            let tool_calls = msg.tool_calls.as_ref().unwrap();
            println!(
                "      ^^^ CORRECT: Assistant message has {} tool call(s): {}",
                tool_calls.len(),
                tool_calls[0].name
            );
        }
    }

    assert!(
        found_tool_message,
        "Second iteration SHOULD have tool message"
    );
    assert!(
        tool_message_correct_format,
        "Tool message MUST have correct tool_call_id='call_1'"
    );
    assert!(
        found_assistant_with_tool_calls,
        "Second iteration MUST have Assistant message with tool_calls"
    );

    println!("\n=== Test PASSED: Tool results correctly passed to next iteration ===\n");
}

#[tokio::test]
async fn test_message_history_grows_correctly() {
    println!("\n=== Test: Message History Growth ===\n");

    let message_tracker = Arc::new(Mutex::new(Vec::new()));
    let mock_llm = TrackingMock::new(message_tracker.clone());

    let agent = ReActAgent::builder()
        .with_llm(Arc::new(mock_llm))
        .with_tool(vol_llm_tdengine::IndexPriceTool::new(None))
        .with_max_iterations(3)
        .with_system_prompt("You are a test assistant.".to_string())
        .with_verbose(false)
        .build()
        .unwrap();

    agent.run("What is the BTC price?").await.unwrap();

    let tracked = message_tracker.lock().await;

    // Each iteration should have more messages (history accumulates)
    println!("Iteration 1: {} messages", tracked[0].len());
    println!("Iteration 2: {} messages", tracked[1].len());

    assert!(tracked.len() >= 2, "Should have at least 2 iterations");
    assert!(
        tracked[1].len() > tracked[0].len(),
        "Second iteration should have more messages (tool result added)"
    );

    // Should have: system + user + assistant(tool calls) + tool(result) = 4
    assert!(
        tracked[1].len() >= 4,
        "Second iteration should have at least 4 messages (system, user, assistant, tool)"
    );

    // Verify assistant message with tool_calls exists
    let has_assistant_with_tool_calls = tracked[1].iter().any(|msg| {
        msg.role == MessageRole::Assistant
            && msg.tool_calls.as_ref().map(|v| v.len()).unwrap_or(0) > 0
    });
    assert!(
        has_assistant_with_tool_calls,
        "Second iteration should have Assistant message with tool_calls"
    );

    // Verify tool message exists with correct format
    let has_tool_message = tracked[1]
        .iter()
        .any(|msg| msg.role == MessageRole::Tool && msg.tool_call_id.is_some());
    assert!(
        has_tool_message,
        "Second iteration should have tool message with tool_call_id"
    );

    println!("=== Test PASSED: Message history grows correctly ===\n");
}
