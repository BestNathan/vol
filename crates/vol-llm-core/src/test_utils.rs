//! Mock LLM client for testing agent loops without real API calls.
//!
//! Gated behind `#[cfg(feature = "test-utils")]`.

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::{
    ConversationRequest, ConversationResponse, LLMClient, LLMProvider, StreamEvent, StreamReceiver,
    SupportedParam,
};

struct MockState {
    converse_response: Option<ConversationResponse>,
    stream_events: Vec<StreamEvent>,
    error_at: Option<usize>,
    call_log: Vec<ConversationRequest>,
}

/// Configurable mock LLM client for testing.
///
/// Uses shared Arc state so the mock can be configured before creation
/// and inspected after the agent run completes.
pub struct MockLlmClient {
    state: Arc<Mutex<MockState>>,
    call_count: Arc<std::sync::atomic::AtomicUsize>,
}

impl MockLlmClient {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(MockState {
                converse_response: None,
                stream_events: Vec::new(),
                error_at: None,
                call_log: Vec::new(),
            })),
            call_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    /// Set the response for converse() calls.
    pub async fn set_converse_response(&self, resp: ConversationResponse) {
        self.state.lock().await.converse_response = Some(resp);
    }

    /// Set the stream events for converse_stream() calls.
    /// Events are returned in order on each call.
    pub async fn set_stream_events(&self, events: Vec<StreamEvent>) {
        self.state.lock().await.stream_events = events;
    }

    /// Configure error at a specific call index (0-based).
    pub async fn set_error_at(&self, index: usize) {
        self.state.lock().await.error_at = Some(index);
    }

    /// Get the number of LLM calls made.
    pub fn call_count(&self) -> usize {
        self.call_count.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Get the last conversation request.
    pub async fn last_request(&self) -> Option<ConversationRequest> {
        self.state.lock().await.call_log.last().cloned()
    }

    /// Get all conversation requests.
    pub async fn all_requests(&self) -> Vec<ConversationRequest> {
        self.state.lock().await.call_log.clone()
    }
}

impl Default for MockLlmClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LLMClient for MockLlmClient {
    fn provider(&self) -> LLMProvider {
        LLMProvider::Anthropic
    }

    fn model(&self) -> &str {
        "mock-llm"
    }

    fn supported_params(&self) -> &[SupportedParam] {
        &[]
    }

    async fn converse(&self, request: ConversationRequest) -> crate::Result<ConversationResponse> {
        let count = self
            .call_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let mut state = self.state.lock().await;
        state.call_log.push(request);

        if let Some(error_at) = state.error_at {
            if count == error_at {
                return Err(crate::LLMError::Timeout("mock error".to_string()));
            }
        }

        state
            .converse_response
            .clone()
            .ok_or_else(|| crate::LLMError::Timeout("mock converse_response not set".to_string()))
    }

    async fn converse_stream(&self, request: ConversationRequest) -> crate::Result<StreamReceiver> {
        use tokio::sync::mpsc;

        let count = self
            .call_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let mut state = self.state.lock().await;
        state.call_log.push(request);

        if let Some(error_at) = state.error_at {
            if count == error_at {
                return Err(crate::LLMError::Timeout("mock stream error".to_string()));
            }
        }

        let events = state.stream_events.clone();
        let (tx, rx) = mpsc::channel(100);

        tokio::spawn(async move {
            for event in events {
                let _ = tx.send(Ok(event)).await;
            }
        });

        Ok(StreamReceiver::new(rx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FinishReason, Message, MessageContent, StreamEventData, TokenUsage};

    #[tokio::test]
    async fn test_mock_default_values() {
        let mock = MockLlmClient::new();
        assert_eq!(mock.provider(), LLMProvider::Anthropic);
        assert_eq!(mock.model(), "mock-llm");
        assert_eq!(mock.call_count(), 0);
    }

    #[tokio::test]
    async fn test_mock_converse_response() {
        let mock = MockLlmClient::new();
        let resp = ConversationResponse {
            message: Message::assistant(MessageContent::Text("test".to_string())),
            model: "mock".to_string(),
            usage: TokenUsage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
                cached_tokens: None,
            },
            finish_reason: FinishReason::Stop,
            raw: None,
        };
        mock.set_converse_response(resp.clone()).await;

        let request = ConversationRequest {
            system: None,
            messages: vec![],
            model_config: Default::default(),
            tools: None,
            tool_choice: None,
            stream: false,
        };
        let result = mock.converse(request).await.unwrap();
        assert_eq!(result.message.role, resp.message.role);
        assert!(result.message.content.is_some());
        assert_eq!(mock.call_count(), 1);
    }

    #[tokio::test]
    async fn test_mock_stream_events() {
        let mock = MockLlmClient::new();
        let events = vec![
            StreamEvent {
                id: "e1".to_string(),
                data: StreamEventData::ContentDelta {
                    delta: "Hello".to_string(),
                },
            },
            StreamEvent {
                id: "e2".to_string(),
                data: StreamEventData::ContentComplete {
                    content: "Hello World".to_string(),
                },
            },
        ];
        mock.set_stream_events(events).await;

        let request = ConversationRequest {
            system: None,
            messages: vec![],
            model_config: Default::default(),
            tools: None,
            tool_choice: None,
            stream: false,
        };
        let mut receiver = mock.converse_stream(request).await.unwrap();
        let mut received = Vec::new();
        while let Some(event) = receiver.recv().await {
            received.push(event.unwrap());
        }
        assert_eq!(received.len(), 2);
        assert_eq!(received[0].id, "e1");
        assert_eq!(received[1].id, "e2");
    }

    #[tokio::test]
    async fn test_mock_error_at() {
        let mock = MockLlmClient::new();
        mock.set_error_at(0).await;

        let request = ConversationRequest {
            system: None,
            messages: vec![],
            model_config: Default::default(),
            tools: None,
            tool_choice: None,
            stream: false,
        };
        let result = mock.converse_stream(request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mock_call_logging() {
        let mock = MockLlmClient::new();

        let request = ConversationRequest {
            system: Some("sys".to_string()),
            messages: vec![Message::user("hi")],
            model_config: Default::default(),
            tools: None,
            tool_choice: None,
            stream: false,
        };
        mock.set_stream_events(vec![]).await;
        let _ = mock.converse_stream(request.clone()).await;

        assert_eq!(mock.call_count(), 1);
        let last = mock.last_request().await.unwrap();
        assert_eq!(last.system, Some("sys".to_string()));
        assert_eq!(mock.all_requests().await.len(), 1);
    }
}
