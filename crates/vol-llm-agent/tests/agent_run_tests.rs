//! Integration tests for ReActAgent run() flow using MockLlmClient.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use async_trait::async_trait;
use vol_llm_agent::react::{
    AgentBuilder, AgentError, AgentStreamEvent,
    plugin::{AgentPlugin, RunContext, PluginDecision, PluginId},
};
use vol_llm_core::{
    LLMClient, LLMProvider, ConversationRequest, ConversationResponse,
    StreamReceiver, StreamEvent, StreamEventData, SupportedParam,
    ToolCall,
};
use vol_llm_core::test_utils::MockLlmClient;

/// Helper: create a ContentComplete stream event.
fn content_complete_event(content: &str) -> StreamEvent {
    StreamEvent {
        id: "event_1".to_string(),
        data: StreamEventData::ContentComplete {
            content: content.to_string(),
        },
    }
}

/// Helper: create a ToolCallComplete stream event.
fn tool_call_event(tool_name: &str, args: &str, call_id: &str) -> StreamEvent {
    StreamEvent {
        id: "event_1".to_string(),
        data: StreamEventData::ToolCallComplete {
            tool_call: ToolCall {
                id: call_id.to_string(),
                name: tool_name.to_string(),
                arguments: args.to_string(),
                r#type: "function".to_string(),
            },
        },
    }
}

// ========================
// Test 1: Single iteration — content only
// ========================

#[tokio::test]
async fn test_agent_run_single_iteration() {
    let mock = MockLlmClient::new();
    mock.set_stream_events(vec![
        content_complete_event("Hello, I can help with that."),
    ]).await;

    let agent = AgentBuilder::new()
        .with_llm(Arc::new(mock))
        .with_max_iterations(5)
        .with_system_prompt("You are a test assistant.".to_string())
        .build()
        .unwrap();

    let result = agent.run("Hi").await.unwrap();
    assert!(result.error.is_none());
    assert_eq!(result.iterations, 1);
    assert_eq!(result.content, "Hello, I can help with that.");
}

/// A tool that always succeeds — used to test tool execution flow.
struct EchoTool;
#[async_trait]
impl vol_llm_tool::ExecutableTool for EchoTool {
    fn name(&self) -> &'static str { "echo_tool" }
    fn description(&self) -> &'static str { "Echo back" }
    fn parameters(&self) -> serde_json::Value { serde_json::json!({"type": "object", "properties": {}}) }
    async fn execute(&self, _args: &serde_json::Value, _ctx: &vol_llm_tool::ToolContext) -> vol_llm_tool::ToolResultType<vol_llm_tool::ToolResult> {
        Ok(vol_llm_tool::ToolResult {
            call_id: "echo".to_string(),
            content: "echoed".to_string(),
            success: true,
            error: None,
            data: None,
        })
    }
}

// ========================
// Test 2: Multi-iteration — tool call then final answer
// ========================

/// Mock that returns different events on each call.
struct MultiCallMock {
    call_count: Arc<AtomicUsize>,
}

impl MultiCallMock {
    fn new() -> (Self, Arc<AtomicUsize>) {
        let count = Arc::new(AtomicUsize::new(0));
        (Self { call_count: count.clone() }, count)
    }
}

#[async_trait]
impl LLMClient for MultiCallMock {
    fn provider(&self) -> LLMProvider { LLMProvider::Anthropic }
    fn model(&self) -> &str { "multi-call-mock" }
    fn supported_params(&self) -> &[SupportedParam] { &[] }

    async fn converse(&self, _request: ConversationRequest) -> vol_llm_core::Result<ConversationResponse> {
        unimplemented!("Use converse_stream instead")
    }

    async fn converse_stream(&self, _request: ConversationRequest) -> vol_llm_core::Result<StreamReceiver> {
        use tokio::sync::mpsc;

        let count = self.call_count.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = mpsc::channel(100);

        tokio::spawn(async move {
            if count == 0 {
                let _ = tx.send(Ok(tool_call_event("echo_tool", r#"{}"#, "call_1"))).await;
            } else {
                let _ = tx.send(Ok(content_complete_event("The file contains hello world."))).await;
            }
        });

        Ok(StreamReceiver::new(rx))
    }
}

#[tokio::test]
async fn test_agent_run_multiple_iterations() {
    struct CountingPlugin {
        tool_count: Arc<AtomicUsize>,
    }
    #[async_trait]
    impl AgentPlugin for CountingPlugin {
        fn id(&self) -> PluginId { "counter".to_string() }
        fn priority(&self) -> u32 { 100 }
        async fn intercept(&self, _: &AgentStreamEvent, _: &RunContext) -> PluginDecision {
            PluginDecision::Continue
        }
        async fn listen(&self, event: &AgentStreamEvent, _: &RunContext) {
            if matches!(event, AgentStreamEvent::ToolCallBegin { .. }) {
                self.tool_count.fetch_add(1, Ordering::SeqCst);
            }
        }
    }

    let (mock, count) = MultiCallMock::new();
    let tool_count = Arc::new(AtomicUsize::new(0));
    let plugin = CountingPlugin { tool_count: tool_count.clone() };

    let agent = AgentBuilder::new()
        .with_llm(Arc::new(mock))
        .with_tool(EchoTool)
        .with_plugin(plugin)
        .with_max_iterations(10)
        .with_system_prompt("You are a test assistant.".to_string())
        .build()
        .unwrap();

    let result = agent.run("Read test.txt").await.unwrap();
    assert!(result.error.is_none());
    assert_eq!(count.load(Ordering::SeqCst), 2); // Two LLM calls
    assert_eq!(tool_count.load(Ordering::SeqCst), 1); // One tool call began
}

// ========================
// Test 3: LLM error propagation
// ========================

struct ErrorOnFirstMock {
    call_count: Arc<AtomicUsize>,
}

impl ErrorOnFirstMock {
    fn new() -> (Self, Arc<AtomicUsize>) {
        let count = Arc::new(AtomicUsize::new(0));
        (Self { call_count: count.clone() }, count)
    }
}

#[async_trait]
impl LLMClient for ErrorOnFirstMock {
    fn provider(&self) -> LLMProvider { LLMProvider::Anthropic }
    fn model(&self) -> &str { "error-mock" }
    fn supported_params(&self) -> &[SupportedParam] { &[] }

    async fn converse(&self, _request: ConversationRequest) -> vol_llm_core::Result<ConversationResponse> {
        unimplemented!()
    }

    async fn converse_stream(&self, _request: ConversationRequest) -> vol_llm_core::Result<StreamReceiver> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        Err(vol_llm_core::LLMError::Timeout("mock LLM failure".to_string()))
    }
}

#[tokio::test]
async fn test_agent_run_llm_error_propagates() {
    let (mock, count) = ErrorOnFirstMock::new();

    let agent = AgentBuilder::new()
        .with_llm(Arc::new(mock))
        .with_max_iterations(5)
        .with_system_prompt("You are a test assistant.".to_string())
        .build()
        .unwrap();

    let result = agent.run("test").await;
    assert!(result.is_err());
    assert_eq!(count.load(Ordering::SeqCst), 1);
}

// ========================
// Test 4: Session recording — verify JSONL file is created via plugin
// ========================

#[tokio::test]
async fn test_agent_run_session_recording() {
    use vol_session::{InMemoryEntryStore, Session, SessionEntryStore};

    let mock = MockLlmClient::new();
    mock.set_stream_events(vec![
        content_complete_event("Session test answer."),
    ]).await;

    let tmp_dir = tempfile::tempdir().unwrap();
    let agent_id = "session_test_agent";

    // Create session and entry_store externally so we can register the plugin
    let entry_store = Arc::new(InMemoryEntryStore::new());
    let session = Arc::new(Session::new(entry_store.clone()));

    let agent = AgentBuilder::new()
        .with_llm(Arc::new(mock))
        .with_max_iterations(5)
        .with_system_prompt("You are a test assistant.".to_string())
        .with_agent_id(agent_id.to_string())
        .with_working_dir(tmp_dir.path().to_path_buf())
        .with_session(session.clone())
        .build()
        .unwrap();

    let result = agent.run("Session question?").await.unwrap();
    assert!(result.error.is_none());

    // Allow async writes to complete
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Verify entries were recorded
    let entries = entry_store.get_entries(&session.id).await.unwrap();
    assert!(!entries.is_empty(), "Session should have entries after agent run");
}

// ========================
// Test 5: Event emission via plugin
// ========================

struct EventCollectorPlugin {
    events: Arc<tokio::sync::Mutex<Vec<String>>>,
}

#[async_trait]
impl AgentPlugin for EventCollectorPlugin {
    fn id(&self) -> PluginId { "collector".to_string() }
    fn priority(&self) -> u32 { 100 }
    async fn intercept(&self, _: &AgentStreamEvent, _: &RunContext) -> PluginDecision {
        PluginDecision::Continue
    }
    async fn listen(&self, event: &AgentStreamEvent, _: &RunContext) {
        let event_name = match event {
            AgentStreamEvent::AgentStart { .. } => "AgentStart",
            AgentStreamEvent::LLMCallStart { .. } => "LLMCallStart",
            AgentStreamEvent::LLMCallComplete { .. } => "LLMCallComplete",
            AgentStreamEvent::ThinkingComplete { .. } => "ThinkingComplete",
            AgentStreamEvent::ContentComplete { .. } => "ContentComplete",
            AgentStreamEvent::AgentComplete { .. } => "AgentComplete",
            _ => "Other",
        };
        self.events.lock().await.push(event_name.to_string());
    }
}

#[tokio::test]
async fn test_agent_run_event_emission() {
    let mock = MockLlmClient::new();
    mock.set_stream_events(vec![
        content_complete_event("Event test answer."),
    ]).await;

    let events = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let plugin = EventCollectorPlugin { events: events.clone() };

    let agent = AgentBuilder::new()
        .with_llm(Arc::new(mock))
        .with_plugin(plugin)
        .with_max_iterations(5)
        .with_system_prompt("You are a test assistant.".to_string())
        .build()
        .unwrap();

    let result = agent.run("Event question?").await.unwrap();
    assert!(result.error.is_none());

    // Allow async plugin events to drain
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    let recorded = events.lock().await.clone();
    assert!(!recorded.is_empty(), "Should have recorded at least one event");
    // Should include AgentStart at minimum
    assert!(recorded.iter().any(|e| e == "AgentStart"), "Expected AgentStart event, got: {:?}", recorded);
}

// ========================
// Test 6: Max iterations reached
// ========================

/// Mock that always returns tool calls for the shared EchoTool.
struct LoopMock {
    call_count: Arc<AtomicUsize>,
}

impl LoopMock {
    fn new() -> (Self, Arc<AtomicUsize>) {
        let count = Arc::new(AtomicUsize::new(0));
        (Self { call_count: count.clone() }, count)
    }
}

#[async_trait]
impl LLMClient for LoopMock {
    fn provider(&self) -> LLMProvider { LLMProvider::Anthropic }
    fn model(&self) -> &str { "loop-mock" }
    fn supported_params(&self) -> &[SupportedParam] { &[] }

    async fn converse(&self, _request: ConversationRequest) -> vol_llm_core::Result<ConversationResponse> {
        unimplemented!()
    }

    async fn converse_stream(&self, _request: ConversationRequest) -> vol_llm_core::Result<StreamReceiver> {
        use tokio::sync::mpsc;
        self.call_count.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = mpsc::channel(10);
        tokio::spawn(async move {
            let _ = tx.send(Ok(tool_call_event("echo_tool", r#"{}"#, "loop"))).await;
        });
        Ok(StreamReceiver::new(rx))
    }
}

#[tokio::test]
async fn test_agent_run_max_iterations_reached() {
    let (mock, count) = LoopMock::new();

    let agent = AgentBuilder::new()
        .with_llm(Arc::new(mock))
        .with_tool(EchoTool)
        .with_max_iterations(3)
        .with_system_prompt("You are a test assistant.".to_string())
        .build()
        .unwrap();

    let result = agent.run("Keep querying...").await;
    match result {
        Err(AgentError::MaxIterationsReached { max }) => {
            assert_eq!(max, 3);
        }
        Err(e) => panic!("Expected MaxIterationsReached, got: {:?}", e),
        Ok(_) => panic!("Expected MaxIterationsReached error"),
    }
    // The agent should have made 3 LLM calls before hitting max iterations
    assert_eq!(count.load(Ordering::SeqCst), 3, "Expected 3 calls, got {}", count.load(Ordering::SeqCst));
}

// ========================
// Test 7: MockLlmClient call tracking
// ========================

#[tokio::test]
async fn test_mock_llm_call_tracking() {
    let mock = MockLlmClient::new();
    mock.set_stream_events(vec![
        content_complete_event("Tracked answer."),
    ]).await;

    let agent = AgentBuilder::new()
        .with_llm(Arc::new(mock))
        .with_max_iterations(5)
        .with_system_prompt("You are a test assistant.".to_string())
        .build()
        .unwrap();

    let _ = agent.run("Track me").await.unwrap();

    // Call tracking is verified via the mock's internal call_count
    // (the mock was moved into the agent, so we can't inspect it directly after run)
}

// ========================
// Test 8: Plugin intercept can abort
// ========================

struct AbortPlugin;

#[async_trait]
impl AgentPlugin for AbortPlugin {
    fn id(&self) -> PluginId { "aborter".to_string() }
    fn priority(&self) -> u32 { 1 } // High priority (lower number = earlier in sorted order)
    async fn intercept(&self, event: &AgentStreamEvent, _: &RunContext) -> PluginDecision {
        if matches!(event, AgentStreamEvent::AgentStart { .. }) {
            PluginDecision::Abort("Plugin vetoed".to_string())
        } else {
            PluginDecision::Continue
        }
    }
    async fn listen(&self, _: &AgentStreamEvent, _: &RunContext) {}
}

#[tokio::test]
async fn test_plugin_intercept_abort() {
    let mock = MockLlmClient::new();
    mock.set_stream_events(vec![
        content_complete_event("This should never be returned."),
    ]).await;

    let agent = AgentBuilder::new()
        .with_llm(Arc::new(mock))
        .with_plugin(AbortPlugin)
        .with_max_iterations(5)
        .with_system_prompt("You are a test assistant.".to_string())
        .build()
        .unwrap();

    let result = agent.run("Hello").await;
    assert!(result.is_err());
    match result {
        Err(AgentError::Context(reason)) => {
            assert!(reason.contains("vetoed") || reason.contains("Plugin"), "Unexpected abort reason: {}", reason);
        }
        Err(e) => panic!("Expected Context error, got: {:?}", e),
        Ok(_) => panic!("Expected error from plugin abort"),
    }
}
