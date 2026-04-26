//! Test session recording completeness via SessionRecorderPlugin.
//!
//! Run with: cargo test --test session_recording_test

use std::sync::Arc;
use vol_session::{InMemoryEntryStore, Session, SessionEntryStore, SessionRecorderPlugin};
use vol_llm_core::AgentStreamEvent;

/// Test SessionRecorderPlugin records events directly
#[tokio::test]
async fn test_session_recorder_plugin_records_events() {
    let entry_store: Arc<dyn SessionEntryStore> = Arc::new(InMemoryEntryStore::new());
    let session = Session::new(entry_store.clone());
    let plugin = SessionRecorderPlugin::new(
        Arc::new(session.clone()),
        entry_store.clone(),
    );

    // Send AgentStart event (user input)
    plugin.record(&AgentStreamEvent::AgentStart {
        input: "User's first input".to_string(),
        timestamp: chrono::Utc::now(),
    }, "test-run").await;

    // Send ThinkingComplete event
    plugin.record(&AgentStreamEvent::ThinkingComplete {
        thinking: "Let me think...".to_string(),
        timestamp: chrono::Utc::now(),
    }, "test-run").await;

    let entries = entry_store.get_entries(&session.id).await.unwrap();
    assert_eq!(entries.len(), 2, "Should have 2 recorded entries");

    // First entry: user input
    if let vol_session::SessionEntryData::Message { message } = &entries[0].data {
        assert_eq!(message.message.role, vol_llm_core::MessageRole::User);
        assert!(message.message.content.as_ref().unwrap().as_str().contains("User's first input"));
    } else {
        panic!("Expected message entry");
    }

    // Second entry: thinking
    if let vol_session::SessionEntryData::Message { message } = &entries[1].data {
        assert_eq!(message.message.role, vol_llm_core::MessageRole::Assistant);
        assert!(message.message.content.as_ref().unwrap().as_str().contains("Let me think"));
    } else {
        panic!("Expected message entry");
    }

    println!("Test passed: {} entries recorded", entries.len());
}
