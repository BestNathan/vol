//! Streaming response types.

use serde::{Deserialize, Serialize};
use crate::{TokenUsage, FinishReason};

/// Stream event
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StreamEvent {
    pub id: String,
    pub event: StreamEventType,
    pub data: StreamEventData,
}

/// Stream event type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamEventType {
    ResponseStart,
    ContentDelta,
    ToolCallDelta,
    UsageUpdate,
    ResponseComplete,
    Error,
}

/// Stream event data
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum StreamEventData {
    ContentDelta { delta: String },
    ToolCallDelta { tool_call_index: usize, delta: ToolCallDelta },
    UsageUpdate { usage: TokenUsage },
    ResponseComplete { finish_reason: FinishReason },
    Error { code: String, message: String },
}

/// Tool call delta
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolCallDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
}

/// Stream receiver
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
            event: StreamEventType::ContentDelta,
            data: StreamEventData::ContentDelta { delta: "Hello".to_string() },
        };
        assert_eq!(event.id, "event_1");
    }
}
