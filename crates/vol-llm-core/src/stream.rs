//! Streaming response types.

use serde::{Deserialize, Serialize};
use crate::{TokenUsage, FinishReason, ToolCall};

/// Stream event
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StreamEvent {
    pub id: String,
    pub data: StreamEventData,
}

/// Stream event data - unified enum combining event type and payload
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEventData {
    // Lifecycle events
    ResponseStart { model: String },
    ResponseComplete { finish_reason: FinishReason },

    // Content (text output)
    ContentDelta { delta: String },
    ContentComplete { content: String },

    // Thinking (model reasoning)
    ThinkingDelta { thinking: String },
    ThinkingComplete { thinking: String },

    // Tool calls
    ToolCallComplete { tool_call: ToolCall },

    // Usage
    UsageUpdate { usage: TokenUsage },

    // Error handling
    Error { code: String, message: String },
}

/// Stream receiver - receives streaming events from provider
pub struct StreamReceiver {
    rx: tokio::sync::mpsc::Receiver<Result<StreamEvent, crate::LLMError>>,
}

impl StreamReceiver {
    pub fn new(rx: tokio::sync::mpsc::Receiver<Result<StreamEvent, crate::LLMError>>) -> Self {
        Self { rx }
    }

    pub async fn recv(&mut self) -> Option<Result<StreamEvent, crate::LLMError>> {
        self.rx.recv().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_event_creation() {
        let event = StreamEvent {
            id: "event_1".to_string(),
            data: StreamEventData::ContentDelta { delta: "Hello".to_string() },
        };
        assert_eq!(event.id, "event_1");
    }

    #[test]
    fn test_stream_event_complete() {
        let event = StreamEvent {
            id: "event_2".to_string(),
            data: StreamEventData::ContentComplete { content: "Hello world".to_string() },
        };
        match event.data {
            StreamEventData::ContentComplete { ref content } => {
                assert_eq!(content, "Hello world");
            }
            _ => panic!("Expected ContentComplete"),
        }
    }
}
