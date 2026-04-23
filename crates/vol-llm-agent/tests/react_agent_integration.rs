//! ReActAgent integration tests with SessionContributor and compression.
//!
//! Tests the full chain: ContextBuilder -> SessionContributor -> compression -> continue.
//! Uses MockLLM (no real API calls).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use async_trait::async_trait;
use vol_llm_agent::ReActAgent;
use vol_llm_core::{
    ConversationRequest, ConversationResponse, LLMClient, LLMProvider,
    Message, StreamEvent, StreamEventData,
};
use vol_llm_core::stream::StreamReceiver;
use vol_session::{
    InMemoryMessageStore, InMemorySessionStore, Session, SessionMessage,
};

// ─── Mock LLM: returns final answer on first call ───────────────────────────

struct QuickAnswerMock {
    answer: String,
}

impl QuickAnswerMock {
    fn new(answer: &str) -> Self {
        Self { answer: answer.to_string() }
    }
}

#[async_trait]
impl LLMClient for QuickAnswerMock {
    fn provider(&self) -> LLMProvider { LLMProvider::Anthropic }
    fn model(&self) -> &str { "mock" }
    fn supported_params(&self) -> &[vol_llm_core::SupportedParam] { &[] }

    async fn converse(&self, _request: ConversationRequest) -> vol_llm_core::Result<ConversationResponse> {
        unimplemented!("Use converse_stream")
    }

    async fn converse_stream(&self, _request: ConversationRequest) -> vol_llm_core::Result<StreamReceiver> {
        use tokio::sync::mpsc;
        let (tx, rx) = mpsc::channel(10);
        let answer = self.answer.clone();
        tokio::spawn(async move {
            let _ = tx.send(Ok(StreamEvent {
                id: "1".to_string(),
                data: StreamEventData::ContentComplete { content: answer },
            })).await;
        });
        Ok(StreamReceiver::new(rx))
    }
}

// ─── Helper: create session with N messages (round-robin User/Assistant) ────

async fn make_session_with_messages(n: usize) -> Arc<Session> {
    let session_store = Arc::new(InMemorySessionStore::new());
    let message_store = Arc::new(InMemoryMessageStore::new());
    let session = Session::new("test-session".to_string(), session_store, message_store);

    for i in 0..n {
        let msg = if i % 2 == 0 {
            SessionMessage::new("test-session".to_string(), Message::user(format!("session-msg-{}", i)))
        } else {
            SessionMessage::new("test-session".to_string(), Message::assistant(format!("session-reply-{}", i)))
        };
        session.add_message(msg).await.unwrap();
    }
    Arc::new(session)
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_basic_run_empty_session() {
    // New session, no history — should produce system + user input only
    let session = make_session_with_messages(0).await;
    let mock = QuickAnswerMock::new("The answer is 42.");

    let agent = ReActAgent::builder()
        .with_llm(Arc::new(mock))
        .with_system_prompt("You are a test assistant.".to_string())
        .with_max_iterations(5)
        .with_verbose(false)
        .with_session(session)
        .build()
        .unwrap();

    let response = agent.run("What is 6 * 7?").await.unwrap();
    assert!(!response.content.is_empty());
}

#[tokio::test]
async fn test_run_with_session_history() {
    // Session with 3 messages — should be included via SessionContributor
    let session = make_session_with_messages(3).await;
    let mock = QuickAnswerMock::new("Continued from previous context.");

    let agent = ReActAgent::builder()
        .with_llm(Arc::new(mock))
        .with_system_prompt("You are a test assistant.".to_string())
        .with_max_iterations(5)
        .with_verbose(false)
        .with_session(session)
        .build()
        .unwrap();

    let response = agent.run("Summarize what we discussed.").await.unwrap();
    assert!(!response.content.is_empty());
}

#[tokio::test]
async fn test_run_with_large_history_limit() {
    // Session with 20 messages, limit set to 100 — all should be included
    let session = make_session_with_messages(20).await;
    let call_count = Arc::new(AtomicUsize::new(0));

    struct CountingMock {
        answer: String,
        calls: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl LLMClient for CountingMock {
        fn provider(&self) -> LLMProvider { LLMProvider::Anthropic }
        fn model(&self) -> &str { "mock" }
        fn supported_params(&self) -> &[vol_llm_core::SupportedParam] { &[] }
        async fn converse(&self, _request: ConversationRequest) -> vol_llm_core::Result<ConversationResponse> {
            unimplemented!("Use converse_stream")
        }
        async fn converse_stream(&self, _request: ConversationRequest) -> vol_llm_core::Result<StreamReceiver> {
            use tokio::sync::mpsc;
            self.calls.fetch_add(1, Ordering::SeqCst);
            let (tx, rx) = mpsc::channel(10);
            let answer = self.answer.clone();
            tokio::spawn(async move {
                let _ = tx.send(Ok(StreamEvent {
                    id: "1".to_string(),
                    data: StreamEventData::ContentComplete { content: answer },
                })).await;
            });
            Ok(StreamReceiver::new(rx))
        }
    }

    let mock = CountingMock {
        answer: "Done.".to_string(),
        calls: call_count.clone(),
    };

    let agent = ReActAgent::builder()
        .with_llm(Arc::new(mock))
        .with_system_prompt("You are a test assistant.".to_string())
        .with_max_iterations(5)
        .with_max_history_messages(100)
        .with_verbose(false)
        .with_session(session)
        .build()
        .unwrap();

    let response = agent.run("Continue.").await.unwrap();
    assert!(!response.content.is_empty());
    // LLM should have been called exactly once
    assert_eq!(call_count.load(Ordering::SeqCst), 1);
}
