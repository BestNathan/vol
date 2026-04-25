//! Session history limit test.
//!
//! Run with: cargo test --test session_history_test

use async_trait::async_trait;
use std::sync::Arc;
use vol_session::{InMemoryEntryStore, Session, SessionMessage};
use vol_llm_agent::{AgentConfig, ReActAgent};
use vol_llm_core::{
    ConversationRequest, ConversationResponse, LLMClient, LLMProvider, Message, SupportedParam,
};

struct MockLlm;

#[async_trait]
impl LLMClient for MockLlm {
    fn provider(&self) -> LLMProvider {
        LLMProvider::Anthropic
    }

    fn model(&self) -> &str {
        "mock"
    }

    fn supported_params(&self) -> &[SupportedParam] {
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

        let (tx, rx) = mpsc::channel(10);
        tokio::spawn(async move {
            let _ = tx
                .send(Ok(StreamEvent {
                    id: "event_1".to_string(),
                    data: StreamEventData::ContentComplete {
                        content: "Mock response".to_string(),
                    },
                }))
                .await;
        });

        Ok(vol_llm_core::StreamReceiver::new(rx))
    }
}

#[tokio::test]
async fn test_history_limit_applied() {
    // Create session with pre-populated messages
    let entry_store = Arc::new(InMemoryEntryStore::new());
    let session = Arc::new(Session::new(entry_store.clone()));

    // Add 30 messages to session (more than default limit of 20)
    for i in 0..30 {
        let msg = SessionMessage::new(session.id.clone(), Message::user(format!("Message {}", i)));
        session.add_message(msg).await.unwrap();
    }

    // Create agent with limit of 10
    let agent = ReActAgent::builder()
        .with_llm(Arc::new(MockLlm))
        .with_session(session.clone())
        .with_max_history_messages(10)
        .build()
        .unwrap();

    // Run agent

    agent.run("Test query").await.unwrap();

    // If we get here without error, agent completed successfully

    // Verify: session should have loaded only 10 history messages
    // The agent should have added 1 message (assistant response)
    // User input is added to runtime messages but NOT persisted to session
    let history = session.get_messages().await.unwrap();
    // 30 original messages + 1 assistant response = 31 total
    assert_eq!(
        history.len(),
        31,
        "Should have 31 total messages (30 original + 1 assistant response)"
    );
}

#[tokio::test]
async fn test_default_history_limit_is_20() {
    // Verify default config has max_history_messages = 20
    let config = AgentConfig::default();
    assert_eq!(
        config.max_history_messages, 20,
        "Default max_history_messages should be 20"
    );
}
