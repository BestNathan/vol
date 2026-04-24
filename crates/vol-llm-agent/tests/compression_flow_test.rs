//! Integration test: ContextBuilder over budget -> SessionContributor.compress() -> compressed get_messages()

use std::sync::Arc;
use tokio::sync::Mutex;
use vol_llm_agent::react::{ContextContributor, context_contributors::SessionContributor};
use vol_llm_core::Message;
use vol_session::{InMemoryMessageStore, InMemorySessionStore, Session, SessionMessage};

async fn make_session_with_messages(n: usize) -> Arc<Mutex<Session>> {
    let session_store = Arc::new(InMemorySessionStore::new());
    let message_store = Arc::new(InMemoryMessageStore::new());
    let session = Session::new("test-session".to_string(), session_store, message_store);

    // Add messages to the store
    for i in 0..n {
        let msg = SessionMessage::new("test-session".to_string(), Message::user(format!("msg-{}", i)));
        session.add_message(msg).await.unwrap();
    }

    Arc::new(Mutex::new(session))
}

#[tokio::test]
async fn test_session_contributor_compress_flow() {
    // Create session with 10 messages
    let session = make_session_with_messages(10).await;

    // Create SessionContributor with max_history=10
    let mut contributor = SessionContributor::new(session.clone(), 10);

    // First contribute: should get all 10 messages
    let blocks = contributor.contribute().await;
    assert_eq!(blocks.len(), 1);
    let total_messages: usize = blocks.iter().map(|b| b.messages.len()).sum();
    assert_eq!(total_messages, 10);

    // Compress
    contributor.compress().await;

    // Second contribute: should get compressed result
    let blocks = contributor.contribute().await;
    assert_eq!(blocks.len(), 1);
    let compressed_count = blocks.iter().map(|b| b.messages.len()).sum::<usize>();
    // Default PositionSampleCompressor(keep_first=3, sample_every=5) on 10 messages:
    // [0,1,2] + [3, 8] + [9] = 6
    assert!(compressed_count < 10, "Should have fewer messages after compression");
    assert!(compressed_count >= 3, "Should keep at least first messages");
}

#[tokio::test]
async fn test_session_contributor_empty_session_compress() {
    let session_store = Arc::new(InMemorySessionStore::new());
    let message_store = Arc::new(InMemoryMessageStore::new());
    let session = Session::new("empty-session".to_string(), session_store, message_store);
    let session = Arc::new(Mutex::new(session));

    let mut contributor = SessionContributor::new(session.clone(), 10);

    // Contribute from empty session
    let blocks = contributor.contribute().await;
    assert!(blocks.is_empty());

    // Compress with no messages — should be no-op
    contributor.compress().await;

    // Contribute again — still empty
    let blocks = contributor.contribute().await;
    assert!(blocks.is_empty());
}
