//! Session event listener for event-driven message recording.
//!
//! SessionListener subscribes to the event bus and records key events
//! to the session message store.

use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{error, warn};

use vol_llm_core::AgentStreamEvent;
use vol_tracing::TracedEvent;

use crate::{MessageStore, SessionError, SessionMessage};

/// Event-driven session listener for message recording.
///
/// Subscribes to the agent event bus and filters records key events
/// to the session message store.
pub struct SessionListener {
    event_rx: broadcast::Receiver<TracedEvent<AgentStreamEvent>>,
    store: Arc<dyn MessageStore>,
    session_id: String,
}

impl SessionListener {
    /// Create a new session listener.
    ///
    /// # Arguments
    /// * `event_rx` - Broadcast receiver for agent stream events
    /// * `store` - Message store for persisting recorded messages
    /// * `session_id` - Session ID to associate with recorded messages
    pub fn new(
        event_rx: broadcast::Receiver<TracedEvent<AgentStreamEvent>>,
        store: Arc<dyn MessageStore>,
        session_id: String,
    ) -> Self {
        Self {
            event_rx,
            store,
            session_id,
        }
    }

    /// Determine if an event should be recorded to the session.
    ///
    /// # Arguments
    /// * `event` - The agent stream event to check
    ///
    /// # Returns
    /// `true` if the event should be recorded, `false` otherwise
    fn should_record(event: &AgentStreamEvent) -> bool {
        matches!(
            event,
            AgentStreamEvent::AgentStart { .. }
                | AgentStreamEvent::ThinkingComplete { .. }
                | AgentStreamEvent::ContentComplete { .. }
                | AgentStreamEvent::ToolCallBegin { .. }
                | AgentStreamEvent::ToolCallComplete { .. }
                | AgentStreamEvent::ToolCallError { .. }
                | AgentStreamEvent::ToolCallSkipped { .. }
                | AgentStreamEvent::IterationComplete { .. }
        )
    }

    /// Convert an agent event to a session message.
    ///
    /// # Arguments
    /// * `event` - The agent stream event to convert
    ///
    /// # Returns
    /// `Some(SessionMessage)` if the event should be recorded, `None` otherwise
    fn event_to_message(&self, event: &AgentStreamEvent) -> Option<SessionMessage> {
        match event {
            // AgentStart -> User message (NEW)
            AgentStreamEvent::AgentStart { input, .. } => Some(SessionMessage::new(
                self.session_id.clone(),
                vol_llm_core::Message::user(input.clone()),
            )),

            // ThinkingComplete -> Assistant message (thinking content)
            AgentStreamEvent::ThinkingComplete { thinking, .. } => Some(SessionMessage::new(
                self.session_id.clone(),
                vol_llm_core::Message::assistant(thinking.clone()),
            )),

            // ContentComplete -> Assistant message (content)
            AgentStreamEvent::ContentComplete { content, .. } => Some(SessionMessage::new(
                self.session_id.clone(),
                vol_llm_core::Message::assistant(content.clone()),
            )),

            // ToolCallBegin -> Assistant message with tool_calls
            // This preserves the original structure for LLM request restoration
            AgentStreamEvent::ToolCallBegin {
                tool_call_id,
                tool_name,
                arguments,
                ..
            } => {
                let tool_call = vol_llm_core::ToolCall {
                    id: tool_call_id.clone(),
                    name: tool_name.clone(),
                    arguments: arguments.clone(),
                    r#type: "function".to_string(),
                };
                Some(SessionMessage::new(
                    self.session_id.clone(),
                    vol_llm_core::Message::assistant_with_tools("", vec![tool_call]),
                ))
            }

            // ToolCallComplete -> Tool message with tool_call_id and result
            // This preserves the original structure for LLM request restoration
            AgentStreamEvent::ToolCallComplete {
                tool_call_id,
                tool_name,
                result,
                ..
            } => {
                let content = format!("Tool '{}' returned: {}", tool_name, result);
                Some(SessionMessage::new(
                    self.session_id.clone(),
                    vol_llm_core::Message::tool(content, tool_call_id.clone()),
                ))
            }

            // ToolCallError -> Tool message with error
            AgentStreamEvent::ToolCallError {
                tool_call_id,
                tool_name,
                error,
                ..
            } => {
                let content = format!("Tool '{}' error: {}", tool_name, error);
                Some(SessionMessage::new(
                    self.session_id.clone(),
                    vol_llm_core::Message::tool(content, tool_call_id.clone()),
                ))
            }

            // ToolCallSkipped -> Tool message with skip reason
            AgentStreamEvent::ToolCallSkipped {
                tool_call_id,
                tool_name,
                reason,
                ..
            } => {
                let content = format!("Tool '{}' skipped: {}", tool_name, reason);
                Some(SessionMessage::new(
                    self.session_id.clone(),
                    vol_llm_core::Message::tool(content, tool_call_id.clone()),
                ))
            }

            // IterationComplete with final_answer -> Assistant message (final answer)
            AgentStreamEvent::IterationComplete { final_answer, .. } => {
                final_answer.as_ref().map(|answer| {
                    SessionMessage::new(
                        self.session_id.clone(),
                        vol_llm_core::Message::assistant(answer.clone()),
                    )
                })
            }

            // Other events are not recorded
            _ => None,
        }
    }

    /// Run the listener loop, receiving events and recording them.
    ///
    /// This method runs until the event channel is closed.
    ///
    /// # Errors
    /// Returns `Err` if there's an error storing a message.
    pub async fn run(&mut self) -> Result<(), SessionError> {
        loop {
            match self.event_rx.recv().await {
                Ok(traced_event) => {
                    let event = traced_event.value();

                    if !Self::should_record(event) {
                        continue;
                    }

                    if let Some(session_msg) = self.event_to_message(event) {
                        if let Err(e) = self.store.save(session_msg).await {
                            error!("Failed to save session message: {}", e);
                            return Err(SessionError::StoreError(e));
                        }
                    }
                }
                Err(broadcast::error::RecvError::Closed) => {
                    // Channel closed, exit gracefully
                    tracing::debug!("Event channel closed, stopping session listener");
                    break;
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    // Lagged - missed n events, continue receiving
                    warn!("Session listener lagged, missed {} events", n);
                    continue;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::InMemoryMessageStore;

    #[tokio::test]
    async fn test_should_record_thinking_complete() {
        let event = AgentStreamEvent::ThinkingComplete {
            timestamp: chrono::Utc::now(),
            thinking: "Let me think about this...".to_string(),
        };
        assert!(SessionListener::should_record(&event));
    }

    #[tokio::test]
    async fn test_should_record_tool_call_begin() {
        let event = AgentStreamEvent::ToolCallBegin {
            timestamp: chrono::Utc::now(),
            tool_call_id: "call_123".to_string(),
            tool_name: "get_weather".to_string(),
            arguments: r#"{"city": "Beijing"}"#.to_string(),
        };
        assert!(SessionListener::should_record(&event));
    }

    #[tokio::test]
    async fn test_should_record_tool_call_complete() {
        let event = AgentStreamEvent::ToolCallComplete {
            timestamp: chrono::Utc::now(),
            tool_call_id: "call_123".to_string(),
            tool_name: "get_weather".to_string(),
            result: "25°C".to_string(),
            duration_ms: None,
        };
        assert!(SessionListener::should_record(&event));
    }

    #[tokio::test]
    async fn test_should_record_iteration_complete_with_final_answer() {
        let event = AgentStreamEvent::IterationComplete {
            timestamp: chrono::Utc::now(),
            iteration: 1,
            tool_calls: Vec::new(),
            final_answer: Some("The weather is 25°C".to_string()),
        };
        assert!(SessionListener::should_record(&event));
    }

    #[tokio::test]
    async fn test_should_record_agent_start() {
        let event = AgentStreamEvent::AgentStart {
            timestamp: chrono::Utc::now(),
            input: "test".to_string(),
        };
        assert!(SessionListener::should_record(&event));
    }

    #[tokio::test]
    async fn test_event_to_message_agent_start() {
        let store = Arc::new(InMemoryMessageStore::new());
        let (_tx, rx) = broadcast::channel(100);
        let listener = SessionListener::new(rx, store, "session-1".to_string());

        let event = AgentStreamEvent::AgentStart {
            timestamp: chrono::Utc::now(),
            input: "User's question".to_string(),
        };

        let msg = listener.event_to_message(&event).unwrap();
        assert_eq!(msg.session_id, "session-1");
        assert_eq!(msg.message.role, vol_llm_core::MessageRole::User);
        assert!(msg.message.content.is_some());
        assert_eq!(msg.message.content.unwrap().as_str(), "User's question");
    }

    #[tokio::test]
    async fn test_event_to_message_tool_call_begin() {
        let store = Arc::new(InMemoryMessageStore::new());
        let (_tx, rx) = broadcast::channel(100);
        let listener = SessionListener::new(rx, store, "session-1".to_string());

        let event = AgentStreamEvent::ToolCallBegin {
            timestamp: chrono::Utc::now(),
            tool_call_id: "call_123".to_string(),
            tool_name: "get_weather".to_string(),
            arguments: r#"{"city": "Beijing"}"#.to_string(),
        };

        let msg = listener.event_to_message(&event).unwrap();
        assert_eq!(msg.session_id, "session-1");
        assert_eq!(msg.message.role, vol_llm_core::MessageRole::Assistant);
        // Verify tool_calls field is populated
        assert!(msg.message.tool_calls.is_some());
        let tool_calls = msg.message.tool_calls.as_ref().unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].id, "call_123");
        assert_eq!(tool_calls[0].name, "get_weather");
    }

    #[tokio::test]
    async fn test_event_to_message_tool_call_complete() {
        let store = Arc::new(InMemoryMessageStore::new());
        let (_tx, rx) = broadcast::channel(100);
        let listener = SessionListener::new(rx, store, "session-1".to_string());

        let event = AgentStreamEvent::ToolCallComplete {
            timestamp: chrono::Utc::now(),
            tool_call_id: "call_123".to_string(),
            tool_name: "get_weather".to_string(),
            result: "25°C".to_string(),
            duration_ms: None,
        };

        let msg = listener.event_to_message(&event).unwrap();
        assert_eq!(msg.session_id, "session-1");
        assert_eq!(msg.message.role, vol_llm_core::MessageRole::Tool);
        // Verify tool_call_id field is populated
        assert_eq!(msg.message.tool_call_id, Some("call_123".to_string()));
    }

    #[tokio::test]
    async fn test_event_to_message_iteration_complete_with_final_answer() {
        let store = Arc::new(InMemoryMessageStore::new());
        let (_tx, rx) = broadcast::channel(100);
        let listener = SessionListener::new(rx, store, "session-1".to_string());

        let event = AgentStreamEvent::IterationComplete {
            timestamp: chrono::Utc::now(),
            iteration: 1,
            tool_calls: Vec::new(),
            final_answer: Some("The answer is 42".to_string()),
        };

        let msg = listener.event_to_message(&event).unwrap();
        assert_eq!(msg.session_id, "session-1");
        assert_eq!(msg.message.role, vol_llm_core::MessageRole::Assistant);
        assert!(msg.message.content.is_some());
    }

    #[tokio::test]
    async fn test_event_to_message_iteration_without_final_answer_returns_none() {
        let store = Arc::new(InMemoryMessageStore::new());
        let (_tx, rx) = broadcast::channel(100);
        let listener = SessionListener::new(rx, store, "session-1".to_string());

        let event = AgentStreamEvent::IterationComplete {
            timestamp: chrono::Utc::now(),
            iteration: 1,
            tool_calls: Vec::new(),
            final_answer: None,
        };

        let msg = listener.event_to_message(&event);
        assert!(msg.is_none());
    }

    #[tokio::test]
    async fn test_listener_run_records_events() {
        let store = Arc::new(InMemoryMessageStore::new());
        let (tx, rx) = broadcast::channel(100);
        let mut listener = SessionListener::new(rx, store.clone(), "session-1".to_string());

        // Send a ThinkingComplete event
        let event = TracedEvent::without_span(AgentStreamEvent::ThinkingComplete {
            timestamp: chrono::Utc::now(),
            thinking: "Test thinking".to_string(),
        });
        tx.send(event).map_err(|_| "send error").unwrap();

        // Drop the sender to close the channel and allow listener to exit
        drop(tx);

        // Run the listener until channel closes
        listener.run().await.unwrap();

        // Verify the message was saved
        let messages = store.get_by_session("session-1", 10).await.unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(
            messages[0].message.role,
            vol_llm_core::MessageRole::Assistant
        );
    }

    #[tokio::test]
    async fn test_listener_run_records_multiple_events() {
        let store = Arc::new(InMemoryMessageStore::new());
        let (tx, rx) = broadcast::channel(100);
        let mut listener = SessionListener::new(rx, store.clone(), "session-1".to_string());

        // Send multiple events
        let events = vec![
            AgentStreamEvent::ThinkingComplete {
                timestamp: chrono::Utc::now(),
                thinking: "Thinking...".to_string(),
            },
            AgentStreamEvent::ToolCallComplete {
                timestamp: chrono::Utc::now(),
                tool_call_id: "call_1".to_string(),
                tool_name: "get_weather".to_string(),
                result: "25°C".to_string(),
                duration_ms: None,
            },
            AgentStreamEvent::IterationComplete {
                timestamp: chrono::Utc::now(),
                iteration: 1,
                tool_calls: Vec::new(),
                final_answer: Some("Final answer".to_string()),
            },
        ];

        for event in events {
            let traced = TracedEvent::without_span(event);
            tx.send(traced).map_err(|_| "send error").unwrap();
        }

        // Drop sender to close channel
        drop(tx);

        // Run listener
        listener.run().await.unwrap();

        // Verify all messages were saved
        let messages = store.get_by_session("session-1", 10).await.unwrap();
        assert_eq!(messages.len(), 3);

        // First: thinking (Assistant)
        assert_eq!(
            messages[0].message.role,
            vol_llm_core::MessageRole::Assistant
        );
        // Second: tool result (Tool)
        assert_eq!(messages[1].message.role, vol_llm_core::MessageRole::Tool);
        // Verify tool_call_id is set
        assert_eq!(messages[1].message.tool_call_id, Some("call_1".to_string()));
        // Third: final answer (Assistant)
        assert_eq!(
            messages[2].message.role,
            vol_llm_core::MessageRole::Assistant
        );
    }

    #[tokio::test]
    async fn test_should_record_content_complete() {
        let event = AgentStreamEvent::ContentComplete {
            timestamp: chrono::Utc::now(),
            content: "Test content".to_string(),
        };
        assert!(SessionListener::should_record(&event));
    }

    #[tokio::test]
    async fn test_should_record_tool_call_error() {
        let event = AgentStreamEvent::ToolCallError {
            timestamp: chrono::Utc::now(),
            tool_call_id: "call_123".to_string(),
            tool_name: "bash".to_string(),
            error: "command failed".to_string(),
            duration_ms: None,
        };
        assert!(SessionListener::should_record(&event));
    }

    #[tokio::test]
    async fn test_should_record_tool_call_skipped() {
        let event = AgentStreamEvent::ToolCallSkipped {
            timestamp: chrono::Utc::now(),
            tool_call_id: "call_123".to_string(),
            tool_name: "bash".to_string(),
            reason: "User rejected".to_string(),
            duration_ms: None,
        };
        assert!(SessionListener::should_record(&event));
    }

    #[tokio::test]
    async fn test_should_not_record_thinking_start() {
        let event = AgentStreamEvent::ThinkingStart {
            timestamp: chrono::Utc::now(),
        };
        assert!(!SessionListener::should_record(&event));
    }

    #[tokio::test]
    async fn test_should_not_record_thinking_delta() {
        let event = AgentStreamEvent::ThinkingDelta {
            timestamp: chrono::Utc::now(),
            delta: "test".to_string(),
        };
        assert!(!SessionListener::should_record(&event));
    }

    #[tokio::test]
    async fn test_should_not_record_content_start() {
        let event = AgentStreamEvent::ContentStart {
            timestamp: chrono::Utc::now(),
        };
        assert!(!SessionListener::should_record(&event));
    }

    #[tokio::test]
    async fn test_should_not_record_content_delta() {
        let event = AgentStreamEvent::ContentDelta {
            timestamp: chrono::Utc::now(),
            delta: "test".to_string(),
        };
        assert!(!SessionListener::should_record(&event));
    }

    #[tokio::test]
    async fn test_should_not_record_llm_call_start() {
        let event = AgentStreamEvent::LLMCallStart {
            timestamp: chrono::Utc::now(),
            iteration: 1,
            messages: vec![],
        };
        assert!(!SessionListener::should_record(&event));
    }

    #[tokio::test]
    async fn test_should_not_record_llm_call_complete() {
        let event = AgentStreamEvent::LLMCallComplete {
            timestamp: chrono::Utc::now(),
            model: "test".to_string(),
            usage: None,
        };
        assert!(!SessionListener::should_record(&event));
    }

    #[tokio::test]
    async fn test_should_not_record_llm_call_error() {
        let event = AgentStreamEvent::LLMCallError {
            timestamp: chrono::Utc::now(),
            error: "test".to_string(),
        };
        assert!(!SessionListener::should_record(&event));
    }

    #[tokio::test]
    async fn test_should_not_record_agent_complete() {
        let event = AgentStreamEvent::AgentComplete {
            timestamp: chrono::Utc::now(),
            response: None,
        };
        assert!(!SessionListener::should_record(&event));
    }

    #[tokio::test]
    async fn test_event_to_message_content_complete() {
        let store = Arc::new(InMemoryMessageStore::new());
        let (_tx, rx) = broadcast::channel(100);
        let listener = SessionListener::new(rx, store, "session-1".to_string());

        let event = AgentStreamEvent::ContentComplete {
            timestamp: chrono::Utc::now(),
            content: "The answer is 42".to_string(),
        };

        let msg = listener.event_to_message(&event).unwrap();
        assert_eq!(msg.session_id, "session-1");
        assert_eq!(msg.message.role, vol_llm_core::MessageRole::Assistant);
        assert!(msg.message.content.is_some());
    }

    #[tokio::test]
    async fn test_event_to_message_tool_call_error() {
        let store = Arc::new(InMemoryMessageStore::new());
        let (_tx, rx) = broadcast::channel(100);
        let listener = SessionListener::new(rx, store, "session-1".to_string());

        let event = AgentStreamEvent::ToolCallError {
            timestamp: chrono::Utc::now(),
            tool_call_id: "call_123".to_string(),
            tool_name: "bash".to_string(),
            error: "command failed".to_string(),
            duration_ms: None,
        };

        let msg = listener.event_to_message(&event).unwrap();
        assert_eq!(msg.session_id, "session-1");
        assert_eq!(msg.message.role, vol_llm_core::MessageRole::Tool);
        assert_eq!(msg.message.tool_call_id, Some("call_123".to_string()));
    }

    #[tokio::test]
    async fn test_event_to_message_tool_call_skipped() {
        let store = Arc::new(InMemoryMessageStore::new());
        let (_tx, rx) = broadcast::channel(100);
        let listener = SessionListener::new(rx, store, "session-1".to_string());

        let event = AgentStreamEvent::ToolCallSkipped {
            timestamp: chrono::Utc::now(),
            tool_call_id: "call_123".to_string(),
            tool_name: "bash".to_string(),
            reason: "User rejected".to_string(),
            duration_ms: None,
        };

        let msg = listener.event_to_message(&event).unwrap();
        assert_eq!(msg.session_id, "session-1");
        assert_eq!(msg.message.role, vol_llm_core::MessageRole::Tool);
        assert_eq!(msg.message.tool_call_id, Some("call_123".to_string()));
    }
}
