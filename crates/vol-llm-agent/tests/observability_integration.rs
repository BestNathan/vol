//! Observability plugin integration test.
//!
//! Run with: cargo test -p vol-llm-agent --test observability_integration

use vol_llm_agent::{ReActAgent, AgentStreamEvent};
use vol_llm_agent::react::{PluginContext, PluginDecision};
use vol_llm_agent::session::{Session, InMemorySessionStore, InMemoryMessageStore};
use vol_llm_core::{LLMClient, ConversationRequest, LLMProvider, StreamEvent, StreamEventData};
use async_trait::async_trait;
use std::sync::Arc;

/// Mock LLM for testing
struct MockLlm;

#[async_trait]
impl LLMClient for MockLlm {
    fn provider(&self) -> LLMProvider {
        LLMProvider::Anthropic
    }

    fn model(&self) -> &str {
        "mock"
    }

    fn supported_params(&self) -> &[vol_llm_core::SupportedParam] {
        &[]
    }

    async fn converse(&self, _request: ConversationRequest) -> vol_llm_core::Result<vol_llm_core::ConversationResponse> {
        unimplemented!("Use converse_stream instead")
    }

    async fn converse_stream(&self, _request: ConversationRequest) -> vol_llm_core::Result<vol_llm_core::stream::StreamReceiver> {
        use tokio::sync::mpsc;

        let (tx, rx) = mpsc::channel(10);
        tokio::spawn(async move {
            let _ = tx.send(Ok(StreamEvent {
                id: "event_1".to_string(),
                data: StreamEventData::ContentComplete {
                    content: "Mock response".to_string(),
                },
            })).await;
        });

        Ok(vol_llm_core::StreamReceiver::new(rx))
    }
}

/// Observability plugin that tracks event counts
struct TestObservabilityPlugin {
    event_count: Arc<tokio::sync::Mutex<usize>>,
}

impl TestObservabilityPlugin {
    fn new(event_count: Arc<tokio::sync::Mutex<usize>>) -> Self {
        Self { event_count }
    }
}

#[async_trait::async_trait]
impl vol_llm_agent::react::plugin::AgentPlugin for TestObservabilityPlugin {
    fn id(&self) -> String {
        "test_observability".to_string()
    }

    fn priority(&self) -> u32 {
        10
    }

    async fn intercept(&self, _event: &AgentStreamEvent, _ctx: &PluginContext) -> PluginDecision {
        PluginDecision::Continue
    }

    async fn listen(&self, _event: &AgentStreamEvent, _ctx: &PluginContext) {
        let mut count = self.event_count.lock().await;
        *count += 1;
    }
}

#[tokio::test]
async fn test_full_agent_run_with_observability() {
    let session_store = Arc::new(InMemorySessionStore::new());
    let message_store = Arc::new(InMemoryMessageStore::new());
    let session = Arc::new(Session::new(
        "test-session".to_string(),
        session_store.clone(),
        message_store.clone(),
    ));

    // Track observability events
    let event_count = Arc::new(tokio::sync::Mutex::new(0usize));

    let agent = ReActAgent::builder()
        .with_llm(Arc::new(MockLlm))
        .with_session(session)
        .with_max_iterations(1)
        .with_system_prompt("You are a helpful assistant.".to_string())
        .with_plugin(TestObservabilityPlugin::new(event_count.clone()))
        .build()
        .unwrap();

    
    agent.run("Test query").await.unwrap();

    // Verify agent completed successfully (if we get here without error, it completed)

    // Verify observability plugin received events
    let count = *event_count.lock().await;
    assert!(count > 0, "Observability plugin should have received events");

    println!("Agent completed successfully, observability captured {} events", count);
}
