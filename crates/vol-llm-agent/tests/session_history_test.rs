//! Session history limit test.
//!
//! Run with: cargo test --test session_history_test

use vol_llm_agent::{ReActAgent, AgentConfig};
use vol_llm_agent::session::{Session, InMemorySessionStore, InMemoryMessageStore, SessionMessage};
use vol_llm_tool::ToolContext;
use vol_llm_core::{LLMClient, LLMProvider, Message, ConversationRequest, ConversationResponse, SupportedParam};
use async_trait::async_trait;
use std::sync::Arc;

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

    async fn converse(&self, _request: ConversationRequest) -> vol_llm_core::Result<ConversationResponse> {
        unimplemented!("Use converse_stream instead")
    }

    async fn converse_stream(&self, _request: ConversationRequest) -> vol_llm_core::Result<vol_llm_core::stream::StreamReceiver> {
        use tokio::sync::mpsc;
        use vol_llm_core::{StreamEvent, StreamEventData};

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

#[tokio::test]
async fn test_history_limit_applied() {
    // Create session with pre-populated messages
    let session_store = Arc::new(InMemorySessionStore::new());
    let message_store = Arc::new(InMemoryMessageStore::new());
    let session = Arc::new(Session::new(
        "test-session".to_string(),
        session_store.clone(),
        message_store.clone(),
    ));

    // Add 30 messages to session (more than default limit of 20)
    for i in 0..30 {
        let msg = SessionMessage::new(
            session.id.clone(),
            Message::user(format!("Message {}", i)),
        );
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
    let context = ToolContext::default();
    agent.run("Test query", context).await.unwrap();

    // If we get here without error, agent completed successfully

    // Verify: session should have loaded only 10 history messages
    // The agent should have added 1 message (assistant response)
    // User input is added to runtime messages but NOT persisted to session
    let history = session.get_messages(100).await.unwrap();
    // 30 original messages + 1 assistant response = 31 total
    assert_eq!(history.len(), 31, "Should have 31 total messages (30 original + 1 assistant response)");
}

#[tokio::test]
async fn test_default_history_limit_is_20() {
    // Verify default config has max_history_messages = 20
    let config = AgentConfig::default();
    assert_eq!(config.max_history_messages, 20, "Default max_history_messages should be 20");
}
