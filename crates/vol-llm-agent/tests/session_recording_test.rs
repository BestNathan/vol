//! Test session recording completeness - verify user input is recorded.
//!
//! Run with: cargo test --test session_recording_test

use std::sync::Arc;
use tokio::sync::broadcast;
use vol_session::{FileSessionEntryStore, Session, SessionEntryStore, SessionListener};
use vol_llm_agent::ReActAgent;
use vol_llm_core::{
    ConversationRequest, ConversationResponse, LLMClient, LLMProvider, SupportedParam,
};
use vol_tracing::TracedEvent;

/// Mock LLM that returns immediately with a simple response
struct MockLlm;

#[async_trait::async_trait]
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

/// Test that session recording captures agent output
#[tokio::test]
async fn test_session_records_agent_output() {
    let tmp_dir = tempfile::tempdir().unwrap();

    // Create session with InMemoryEntryStore
    let entry_store = Arc::new(vol_session::InMemoryEntryStore::new());
    let session = Arc::new(Session::new(entry_store.clone()));

    // Create agent
    let agent = ReActAgent::builder()
        .with_llm(Arc::new(MockLlm))
        .with_session(session.clone())
        .with_log_base_path(tmp_dir.path().to_path_buf())
        .with_agent_id("test_agent".to_string())
        .build()
        .unwrap();

    // Run agent
    agent.run("What is the weather in Beijing?").await.unwrap();

    // Allow async writes to complete
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Verify the session's entry store has entries after the agent run
    let entries = entry_store.get_entries(&session.id).await.unwrap();
    assert!(!entries.is_empty(), "Session should have entries after agent run");
}

/// Test session recording with event-driven approach - verify what events are recorded
#[tokio::test]
async fn test_session_listener_records_what_events() {
    let (event_tx, event_rx) = broadcast::channel(100);

    let tmp_dir = tempfile::tempdir().unwrap();
    let store: Arc<dyn vol_session::SessionEntryStore> = Arc::new(FileSessionEntryStore::new(
        tmp_dir.path().to_str().unwrap(),
    ));

    let mut listener = SessionListener::new(
        event_rx,
        store,
        "session-events".to_string(),
    );
    let handle = tokio::spawn(async move { listener.run().await.unwrap() });

    // Send AgentStart event (user input)
    event_tx
        .send(TracedEvent::without_span(vol_llm_core::AgentStreamEvent::AgentStart {
            input: "User's first input".to_string(),
            timestamp: chrono::Utc::now(),
        }))
        .map_err(|_| "send error")
        .unwrap();

    // Send ThinkingComplete event
    event_tx
        .send(TracedEvent::without_span(
            vol_llm_core::AgentStreamEvent::ThinkingComplete {
                thinking: "Let me think...".to_string(),
                timestamp: chrono::Utc::now(),
            },
        ))
        .map_err(|_| "send error")
        .unwrap();

    // Wait for processing
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Close channel
    drop(event_tx);
    handle.await.unwrap();

    // Verify file contents — FileSessionEntryStore writes to {entry_dir}/{session_id}.jsonl
    let file_path = tmp_dir
        .path()
        .join("session-events.jsonl");

    let content = tokio::fs::read_to_string(&file_path).await.unwrap();
    let lines: Vec<&str> = content.lines().collect();

    println!("Session log content:\n{}", content);

    // AgentStart SHOULD NOW be recorded (user input)
    let contains_user_input = content.contains("User's first input");

    assert!(
        contains_user_input,
        "AgentStart should be recorded as user message, content was: {}",
        content
    );

    // ThinkingComplete SHOULD be recorded (as assistant message)
    assert!(
        content.contains("Let me think"),
        "ThinkingComplete thinking content should be recorded, content was: {}",
        content
    );

    println!(
        "Test passed: {} lines recorded, AgentStart is recorded",
        lines.len()
    );
}
