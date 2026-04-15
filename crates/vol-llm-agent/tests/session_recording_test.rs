//! Test session recording completeness - verify user input is recorded.
//!
//! Run with: cargo test --test session_recording_test

use std::sync::Arc;
use tokio::sync::broadcast;
use vol_llm_agent::session::{FileMessageStore, InMemorySessionStore, MessageStore, Session, SessionListener};
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

/// Test that session recording includes user input
#[tokio::test]
async fn test_session_records_user_input() {
    let tmp_dir = tempfile::tempdir().unwrap();

    // Create session with FileMessageStore for session recording
    let session_store = Arc::new(InMemorySessionStore::new());
    let message_store = Arc::new(FileMessageStore::new(
        tmp_dir.path().to_str().unwrap(),
        "test-session",
    ));
    let session = Arc::new(Session::new(
        "test-session".to_string(),
        session_store,
        message_store,
    ));

    // Create agent
    let agent = ReActAgent::builder()
        .with_llm(Arc::new(MockLlm))
        .with_session(session.clone())
        .with_log_base_path(tmp_dir.path().to_path_buf())
        .with_agent_id("test_agent".to_string())
        .build()
        .unwrap();

    // Run agent with specific user input
    let user_input = "What is the weather in Beijing?";
    agent.run(user_input).await.unwrap();

    // Read session log file
    // SessionListener uses log_base_path/agent_id/sessions/session_id.jsonl
    let session_log_path = tmp_dir
        .path()
        .join("test_agent")
        .join("sessions")
        .join("test-session.jsonl");

    // Give async writes time to complete
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Check if file exists
    if !session_log_path.exists() {
        panic!("Session log file does not exist: {:?}", session_log_path);
    }

    let content = tokio::fs::read_to_string(&session_log_path).await.unwrap();
    let lines: Vec<&str> = content.lines().collect();

    println!("Session log content:\n{}", content);
    println!("Total lines: {}", lines.len());

    // Verify user input is recorded
    let contains_user_input = content.contains(user_input);

    assert!(
        contains_user_input,
        "Session log should contain user input '{}', but content was:\n{}",
        user_input, content
    );
}

/// Test session recording with event-driven approach - verify what events are recorded
#[tokio::test]
async fn test_session_listener_records_what_events() {
    let (event_tx, event_rx) = broadcast::channel(100);

    let tmp_dir = tempfile::tempdir().unwrap();
    let store: Arc<dyn MessageStore> = Arc::new(FileMessageStore::new(
        tmp_dir.path().to_str().unwrap(),
        "session-events",
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

    // Verify file contents
    let file_path = tmp_dir
        .path()
        .join("sessions")
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
