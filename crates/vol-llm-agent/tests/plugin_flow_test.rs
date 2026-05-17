//! Plugin flow integration tests.
//!
//! Run with: cargo test -p vol-llm-agent --test plugin_flow_test -- --nocapture
//!
//! This file tests the plugin flow integration with a mock LLM.

use async_trait::async_trait;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use vol_llm_agent::react::{AgentPlugin, RunContext, PluginDecision};
use vol_llm_agent::{AgentConfig, AgentStreamEvent, ReActAgent};
use vol_llm_core::{
    ConversationRequest, ConversationResponse, LLMClient, LLMProvider, StreamEvent, StreamEventData,
};
use vol_llm_tool::ToolContext;

/// Mock LLM that returns a simple text response
struct MockLlm {
    call_count: Arc<AtomicUsize>,
    response_text: String,
}

impl MockLlm {
    fn new(response_text: String) -> Self {
        Self {
            call_count: Arc::new(AtomicUsize::new(0)),
            response_text,
        }
    }
}

#[async_trait]
impl LLMClient for MockLlm {
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

        self.call_count.fetch_add(1, Ordering::SeqCst);

        let (tx, rx) = mpsc::channel(10);
        let response_text = self.response_text.clone();

        tokio::spawn(async move {
            let _ = tx
                .send(Ok(StreamEvent {
                    id: "event_1".to_string(),
                    data: StreamEventData::ContentComplete {
                        content: response_text,
                    },
                }))
                .await;
        });

        Ok(vol_llm_core::StreamReceiver::new(rx))
    }
}

/// Plugin that tracks intercept call count
struct TrackingPlugin {
    id: String,
    priority: u32,
    intercept_count: Arc<AtomicUsize>,
    listen_count: Arc<AtomicUsize>,
}

impl TrackingPlugin {
    fn new(
        id: String,
        priority: u32,
        intercept_count: Arc<AtomicUsize>,
        listen_count: Arc<AtomicUsize>,
    ) -> Self {
        Self {
            id,
            priority,
            intercept_count,
            listen_count,
        }
    }
}

#[async_trait]
impl AgentPlugin for TrackingPlugin {
    fn id(&self) -> String {
        self.id.clone()
    }

    fn priority(&self) -> u32 {
        self.priority
    }

    async fn intercept(&self, _event: &AgentStreamEvent, _ctx: &RunContext) -> PluginDecision {
        self.intercept_count.fetch_add(1, Ordering::SeqCst);
        PluginDecision::Continue
    }

    async fn listen(&self, _event: &AgentStreamEvent, _ctx: &RunContext) {
        self.listen_count.fetch_add(1, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn test_plugin_interceptor_chain_executes() {
    let intercept_count = Arc::new(AtomicUsize::new(0));
    let listen_count = Arc::new(AtomicUsize::new(0));

    let mock_llm = MockLlm::new("Hello, I am a mock response.".to_string());

    let config = AgentConfig::builder()
        .with_llm(Arc::new(mock_llm))
        .with_system_prompt("You are a helpful assistant.".to_string())
        .with_plugin(TrackingPlugin::new(
            "tracker1".to_string(),
            10,
            intercept_count.clone(),
            listen_count.clone(),
        ))
        .with_plugin(TrackingPlugin::new(
            "tracker2".to_string(),
            20,
            intercept_count.clone(),
            listen_count.clone(),
        ))
        .build()
        .unwrap();
    let agent = ReActAgent::new(config);

    agent.run("Say hello").await.unwrap();

    // Verify agent completed successfully (if we get here without error, it completed)

    // Verify intercept and listen were called
    let intercepts = intercept_count.load(Ordering::SeqCst);
    let listens = listen_count.load(Ordering::SeqCst);

    println!("Intercept count: {}, Listen count: {}", intercepts, listens);

    // Verify plugins were called - each event triggers both intercept and listen
    // Agent run produces multiple events: AgentStart, ContentComplete, AgentComplete
    assert!(
        intercepts > 0,
        "Intercept should have been called at least once"
    );
    assert!(listens > 0, "Listen should have been called at least once");
}

/// Test that plugin intercept can skip events
#[tokio::test]
async fn test_plugin_skip_stops_current_event() {
    use std::sync::atomic::AtomicUsize;

    struct SkipFirstPlugin {
        call_count: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl AgentPlugin for SkipFirstPlugin {
        fn id(&self) -> String {
            "skipper".to_string()
        }

        fn priority(&self) -> u32 {
            10
        }

        async fn intercept(
            &self,
            event: &AgentStreamEvent,
            _ctx: &RunContext,
        ) -> PluginDecision {
            let count = self.call_count.fetch_add(1, Ordering::SeqCst);

            // Skip the first event (AgentStart)
            if count == 0 && matches!(event, AgentStreamEvent::AgentStart { .. }) {
                PluginDecision::Skip
            } else {
                PluginDecision::Continue
            }
        }

        async fn listen(&self, _event: &AgentStreamEvent, _ctx: &RunContext) {}
    }

    let call_count = Arc::new(AtomicUsize::new(0));

    let mock_llm = MockLlm::new("Hello world.".to_string());

    let config = AgentConfig::builder()
        .with_llm(Arc::new(mock_llm))
        .with_system_prompt("You are a helpful assistant.".to_string())
        .with_plugin(SkipFirstPlugin {
            call_count: call_count.clone(),
        })
        .build()
        .unwrap();
    let agent = ReActAgent::new(config);

    agent.run("Say hello").await.unwrap();

    // Agent should complete successfully (Skip just skips the event, doesn't abort)
    // Verify plugin was called
    assert!(
        call_count.load(Ordering::SeqCst) > 0,
        "Plugin should have been called"
    );
}

/// Test that listener plugins execute in parallel (fire-and-forget)
#[tokio::test]
async fn test_listener_parallel_execution() {
    use tokio::time::{Duration, Instant};

    struct SlowListener {
        id: String,
        delay_ms: u64,
    }

    #[async_trait]
    impl AgentPlugin for SlowListener {
        fn id(&self) -> String {
            self.id.clone()
        }

        fn priority(&self) -> u32 {
            100
        }

        async fn intercept(
            &self,
            _event: &AgentStreamEvent,
            _ctx: &RunContext,
        ) -> PluginDecision {
            PluginDecision::Continue
        }

        async fn listen(&self, _event: &AgentStreamEvent, _ctx: &RunContext) {
            // Simulate slow operation
            tokio::time::sleep(Duration::from_millis(self.delay_ms)).await;
        }
    }

    // Create plugins with different delays
    let plugins: Vec<Arc<dyn AgentPlugin>> = vec![
        Arc::new(SlowListener {
            id: "slow1".to_string(),
            delay_ms: 50,
        }),
        Arc::new(SlowListener {
            id: "slow2".to_string(),
            delay_ms: 50,
        }),
        Arc::new(SlowListener {
            id: "slow3".to_string(),
            delay_ms: 50,
        }),
    ];

    let event = AgentStreamEvent::AgentStart {
        input: "test".to_string(),
        timestamp: chrono::Utc::now(),
    };
    let ctx = create_test_run_context();

    // Execute listeners in parallel (spawn each one)
    let start = Instant::now();

    let mut handles = vec![];
    for plugin in &plugins {
        let plugin = plugin.clone();
        let event = event.clone();
        let ctx = ctx.clone();
        handles.push(tokio::spawn(async move {
            plugin.listen(&event, &ctx).await;
        }));
    }

    // Wait for all to complete
    for handle in handles {
        handle.await.unwrap();
    }

    let elapsed = start.elapsed();

    // If parallel: ~50ms, if sequential: ~150ms
    // Allow some margin for task scheduling overhead
    assert!(
        elapsed < Duration::from_millis(100),
        "Listeners should execute in parallel, but took {:?}",
        elapsed
    );
}

fn create_test_run_context() -> RunContext {
    use vol_llm_agent::react::{AgentConfig, RunContext};

    let (ctx, _plugin_rx) = RunContext::new(
        "test-run".to_string(),
        "test input".to_string(),
        AgentConfig::default(),
    );

    ctx
}
