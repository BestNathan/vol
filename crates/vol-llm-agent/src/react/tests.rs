//! Unit tests for the react module.

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use vol_llm_core::LLMClient;
use vol_llm_core::{ConversationRequest, ConversationResponse, StreamReceiver, SupportedParam};

struct DummyLlm;
#[async_trait::async_trait]
impl LLMClient for DummyLlm {
    fn provider(&self) -> vol_llm_core::LLMProvider { vol_llm_core::LLMProvider::Anthropic }
    fn model(&self) -> &str { "dummy" }
    fn supported_params(&self) -> &[SupportedParam] { &[] }
    async fn converse(&self, _request: ConversationRequest) -> vol_llm_core::Result<ConversationResponse> { unimplemented!() }
    async fn converse_stream(&self, _request: ConversationRequest) -> vol_llm_core::Result<StreamReceiver> { unimplemented!() }
}

// ========================
// config_builder.rs tests
// ========================

#[tokio::test]
async fn test_builder_default() {
    let builder = AgentConfigBuilder::new();
    let result = builder.build();
    // Should fail without LLM
    assert!(result.is_err());
}

#[tokio::test]
async fn test_builder_with_methods() {
    let llm = Arc::new(DummyLlm);
    let tmp_dir = tempfile::tempdir().unwrap();
    let _tmp_dir = tmp_dir; // kept for future test expansion
    let session = Arc::new(vol_session::Session::new(
        Arc::new(vol_session::InMemoryEntryStore::new()),
    ));
    // Build succeeds with all builder methods chained
    let result = AgentConfig::builder()
        .with_llm(llm)
        .with_system_prompt("You are a test assistant.".to_string())
        .with_session(session)
        .build();

    assert!(result.is_ok());
}

#[test]
fn test_build_missing_llm() {
    let result = AgentConfigBuilder::new().build();
    assert!(result.is_err());
    if let Err(e) = result {
        assert!(e.to_string().contains("LLM") || e.to_string().contains("llm"));
    }
}

#[tokio::test]
async fn test_build_with_plugin() {
    struct DummyPlugin;
    #[async_trait::async_trait]
    impl plugin::AgentPlugin for DummyPlugin {
        fn id(&self) -> plugin::PluginId { "dummy".to_string() }
        fn priority(&self) -> u32 { 50 }
        async fn intercept(&self, _: &AgentStreamEvent, _: &RunContext) -> plugin::PluginDecision { plugin::PluginDecision::Continue }
        async fn listen(&self, _: &AgentStreamEvent, _: &RunContext) {}
    }

    let llm = Arc::new(DummyLlm);
    let config = AgentConfig::builder()
        .with_llm(llm)
        .with_plugin(DummyPlugin)
        .build()
        .unwrap();
    let agent = ReActAgent::new(config);

    // Build succeeds with plugin registered
    let _ = agent;
}

// ========================
// response.rs tests
// ========================

#[test]
fn test_agent_error_display() {
    let llm_err = AgentError::Llm(vol_llm_core::LLMError::Timeout("api failed".to_string()));
    assert!(llm_err.to_string().contains("api failed"));

    let tool_err = AgentError::ToolExecution {
        tool: "bash".to_string(),
        error: "permission denied".to_string(),
    };
    assert!(tool_err.to_string().contains("bash"));
    assert!(tool_err.to_string().contains("permission denied"));

    let max_err = AgentError::MaxIterationsReached { max: 5 };
    assert!(max_err.to_string().contains("5"));

    let ctx_err = AgentError::Context("missing context".to_string());
    assert!(ctx_err.to_string().contains("missing context"));

    let session_err = AgentError::SessionError("session failed".to_string());
    assert!(session_err.to_string().contains("session failed"));
}

#[test]
fn test_agent_response_construction() {
    let response = AgentResponse {
        content: "Hello World".to_string(),
        reasoning: vec![],
        run_id: "run_123".to_string(),
        session_id: "sess_456".to_string(),
        iterations: 3,
        tool_calls: vec![],
        error: None,
    };

    assert_eq!(response.content, "Hello World");
    assert_eq!(response.run_id, "run_123");
    assert!(response.error.is_none()); // No error = success
}

#[test]
fn test_agent_response_with_error() {
    let response = AgentResponse {
        content: String::new(),
        reasoning: vec![],
        run_id: "run_123".to_string(),
        session_id: "sess_456".to_string(),
        iterations: 0,
        tool_calls: vec![],
        error: Some("failed".to_string()),
    };

    assert!(response.error.is_some());
    assert_eq!(response.error, Some("failed".to_string()));
}

// ========================
// state.rs tests
// ========================

#[test]
fn test_reasoning_step_creation() {
    let step = state::ReasoningStep::new(1, "thinking about it".to_string(), Some(100));
    assert_eq!(step.iteration, 1);
    assert_eq!(step.thinking, "thinking about it");
    assert_eq!(step.duration_ms, Some(100));
}

#[test]
fn test_reasoning_step_no_duration() {
    let step = state::ReasoningStep::new(5, "more thinking".to_string(), None);
    assert_eq!(step.iteration, 5);
    assert!(step.duration_ms.is_none());
}

#[test]
fn test_tool_call_record() {
    let record = state::ToolCallRecord {
        tool_name: "bash".to_string(),
        arguments: "{}".to_string(),
        result: "ok".to_string(),
        iteration: 1,
        success: true,
    };
    assert_eq!(record.tool_name, "bash");
    assert!(record.success);
}

// ========================
// stream.rs tests (1 existing test in stream.rs itself, adding 2 more here)
// ========================

#[test]
fn test_agent_stream_receiver_creation() {
    let (_tx, rx) = tokio::sync::mpsc::channel(1);
    let _receiver = AgentStreamReceiver::new(rx);
}

#[tokio::test]
async fn test_agent_stream_receiver_recv() {
    let (tx, rx) = tokio::sync::mpsc::channel(10);
    let mut receiver = AgentStreamReceiver::new(rx);

    tx.send(Ok(AgentStreamEvent::agent_start("test".to_string()))).await.unwrap();
    drop(tx);

    let event = receiver.recv().await;
    assert!(event.is_some());
    let event = event.unwrap().unwrap();
    assert!(matches!(event, AgentStreamEvent::AgentStart { .. }));
}

// ========================
// plugin_stream.rs tests
// ========================

#[tokio::test]
async fn test_run_interceptor_loop_continue_decision() {
    // A plugin that always returns Continue
    struct ContinuePlugin;
    #[async_trait::async_trait]
    impl plugin::AgentPlugin for ContinuePlugin {
        fn id(&self) -> plugin::PluginId { "continue".to_string() }
        fn priority(&self) -> u32 { 10 }
        async fn intercept(&self, _: &AgentStreamEvent, _: &RunContext) -> plugin::PluginDecision {
            plugin::PluginDecision::Continue
        }
        async fn listen(&self, _: &AgentStreamEvent, _: &RunContext) {}
    }

    let (plugin_tx, plugin_rx) = tokio::sync::mpsc::channel(10);
    let (event_tx, _) = tokio::sync::broadcast::channel(10);
    let event_tx = Arc::new(event_tx);
    let (run_ctx, _rx) = RunContext::new(
        "test".to_string(),
        "test".to_string(),
        AgentConfig::default(),
    );

    let plugins: Vec<Arc<dyn plugin::AgentPlugin>> = vec![Arc::new(ContinuePlugin)];

    let interceptor = tokio::spawn(run_interceptor_loop(plugin_rx, plugins, event_tx, run_ctx));

    // Send an intercept request
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    plugin_tx.send(PluginRequest::Intercept {
        event: vol_tracing::TracedEvent::without_span(AgentStreamEvent::agent_start("test".to_string())),
        tx: reply_tx,
    }).await.unwrap();

    let decision = reply_rx.await.unwrap();
    assert!(matches!(decision, plugin::PluginDecision::Continue));

    // Shutdown
    drop(plugin_tx);
    interceptor.await.unwrap();
}

#[tokio::test]
async fn test_run_interceptor_loop_skip_decision() {
    struct SkipPlugin;
    #[async_trait::async_trait]
    impl plugin::AgentPlugin for SkipPlugin {
        fn id(&self) -> plugin::PluginId { "skip".to_string() }
        fn priority(&self) -> u32 { 10 }
        async fn intercept(&self, _: &AgentStreamEvent, _: &RunContext) -> plugin::PluginDecision {
            plugin::PluginDecision::Skip
        }
        async fn listen(&self, _: &AgentStreamEvent, _: &RunContext) {}
    }

    let (plugin_tx, plugin_rx) = tokio::sync::mpsc::channel(10);
    let (event_tx, _) = tokio::sync::broadcast::channel(10);
    let event_tx = Arc::new(event_tx);
    let (run_ctx, _rx) = RunContext::new(
        "test".to_string(),
        "test".to_string(),
        AgentConfig::default(),
    );

    let plugins: Vec<Arc<dyn plugin::AgentPlugin>> = vec![Arc::new(SkipPlugin)];

    let interceptor = tokio::spawn(run_interceptor_loop(plugin_rx, plugins, event_tx.clone(), run_ctx));

    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    plugin_tx.send(PluginRequest::Intercept {
        event: vol_tracing::TracedEvent::without_span(AgentStreamEvent::agent_start("test".to_string())),
        tx: reply_tx,
    }).await.unwrap();

    let decision = reply_rx.await.unwrap();
    assert!(matches!(decision, plugin::PluginDecision::Skip));

    drop(plugin_tx);
    interceptor.await.unwrap();
}

#[tokio::test]
async fn test_run_interceptor_loop_emit_request() {
    let (plugin_tx, plugin_rx) = tokio::sync::mpsc::channel(10);
    let (event_tx, mut event_rx) = tokio::sync::broadcast::channel(10);
    let event_tx = Arc::new(event_tx);
    let (run_ctx, _rx) = RunContext::new(
        "test".to_string(),
        "test".to_string(),
        AgentConfig::default(),
    );

    let plugins: Vec<Arc<dyn plugin::AgentPlugin>> = vec![];

    let interceptor = tokio::spawn(run_interceptor_loop(plugin_rx, plugins, event_tx, run_ctx));

    // Send an emit request
    plugin_tx.send(PluginRequest::Emit {
        event: vol_tracing::TracedEvent::without_span(AgentStreamEvent::agent_start("test".to_string())),
    }).await.unwrap();

    // Should receive event on broadcast
    let event = event_rx.recv().await.unwrap();
    assert!(matches!(event.value(), AgentStreamEvent::AgentStart { .. }));

    drop(plugin_tx);
    interceptor.await.unwrap();
}

#[tokio::test]
async fn test_plugin_decision_variants() {
    // Compile-time + runtime check for all decision variants
    let _continue = plugin::PluginDecision::Continue;
    let _skip = plugin::PluginDecision::Skip;
    let _abort = plugin::PluginDecision::Abort("reason".to_string());
}

// ========================
// Additional hitl.rs tests
// ========================

#[test]
fn test_hitl_config_with_triggers() {
    let config = hitl::HitlConfig {
        triggers: vec![
            hitl::ApprovalTrigger::ToolExecution { tools: None },
            hitl::ApprovalTrigger::AfterIteration,
            hitl::ApprovalTrigger::BeforeFinalAnswer,
        ],
        timeout_secs: 30,
        on_timeout: hitl::TimeoutBehavior::Reject { reason: "timed out".to_string() },
        timeout_message: Some("Please respond within 30 seconds".to_string()),
    };

    assert_eq!(config.triggers.len(), 3);
    assert_eq!(config.timeout_secs, 30);
}

#[tokio::test]
async fn test_spawn_listener_tasks_shutdown_on_close() {
    struct ListenPlugin {
        count: Arc<AtomicUsize>,
    }
    #[async_trait::async_trait]
    impl plugin::AgentPlugin for ListenPlugin {
        fn id(&self) -> plugin::PluginId { "listen".to_string() }
        fn priority(&self) -> u32 { 100 }
        async fn intercept(&self, _: &AgentStreamEvent, _: &RunContext) -> plugin::PluginDecision {
            plugin::PluginDecision::Continue
        }
        async fn listen(&self, _: &AgentStreamEvent, _: &RunContext) {
            self.count.fetch_add(1, Ordering::SeqCst);
        }
    }

    let (mut run_ctx, _rx) = RunContext::new(
        "test".to_string(),
        "test".to_string(),
        AgentConfig::default(),
    );

    let count1 = Arc::new(AtomicUsize::new(0));
    let count2 = Arc::new(AtomicUsize::new(0));

    let plugins: Vec<Arc<dyn plugin::AgentPlugin>> = vec![
        Arc::new(ListenPlugin { count: count1.clone() }),
        Arc::new(ListenPlugin { count: count2.clone() }),
    ];

    // Emit an event while listeners are running
    run_ctx.emit(AgentStreamEvent::agent_start("test".to_string())).await;

    // Spawn listeners
    let mut listener_set = spawn_listener_tasks(plugins, run_ctx.clone());

    // Emit another event
    run_ctx.emit(AgentStreamEvent::agent_complete_with_response(serde_json::json!({}))).await;

    // Give listeners a moment to process
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Drop the sender — this closes the broadcast and triggers listener exit
    run_ctx.event_tx.take();

    // Wait for all listeners to exit
    while let Some(result) = listener_set.join_next().await {
        assert!(result.is_ok(), "Listener task should not panic");
    }

    // Both plugins should have processed events
    assert!(count1.load(Ordering::SeqCst) > 0,
        "Plugin 1 should have processed events, got {}", count1.load(Ordering::SeqCst));
    assert!(count2.load(Ordering::SeqCst) > 0,
        "Plugin 2 should have processed events, got {}", count2.load(Ordering::SeqCst));
}

#[test]
fn test_hitl_needs_tool_approval_all_tools() {
    // Test via HitlConfig + ApprovalTrigger combination
    let config = hitl::HitlConfig {
        triggers: vec![hitl::ApprovalTrigger::ToolExecution { tools: None }],
        ..Default::default()
    };
    assert!(!config.triggers.is_empty());
}
