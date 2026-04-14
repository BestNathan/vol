//! Streaming response types.

use crate::{FinishReason, TokenUsage, ToolCall};
use serde::{Deserialize, Serialize};

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

/// Agent stream event for ReAct agent workflow.
///
/// These events are emitted during agent execution and can be used
/// for session recording, observability, and plugin interception.
///
/// # Semantic Guarantees
///
/// 1. Every execution path ends with AgentComplete or AgentAborted
/// 2. LLM calls are paired: LLMCallStart → LLMCallComplete or LLMCallError
/// 3. Tool calls are paired: ToolCallBegin → ToolCallComplete or ToolCallError or ToolCallSkipped
/// 4. Delta sequences are complete: Start → Delta×N → Complete
#[derive(Debug, Clone)]
pub enum AgentStreamEvent {
    // === Lifecycle (3) ===
    AgentStart { input: String },
    AgentComplete,
    AgentAborted { reason: String },

    // === LLM Call (3) ===
    LLMCallStart { iteration: u32 },
    LLMCallComplete { model: String, usage: Option<TokenUsage> },
    LLMCallError { error: String },

    // === Streaming: Thinking (3) ===
    ThinkingStart,
    ThinkingDelta { delta: String },
    ThinkingComplete { thinking: String },

    // === Streaming: Content (3) ===
    ContentStart,
    ContentDelta { delta: String },
    ContentComplete { content: String },

    // === Tool Execution (4) ===
    ToolCallBegin {
        tool_call_id: String,
        tool_name: String,
        arguments: String,
    },
    ToolCallComplete {
        tool_call_id: String,
        tool_name: String,
        result: String,
    },
    ToolCallError {
        tool_call_id: String,
        tool_name: String,
        error: String,
    },
    ToolCallSkipped {
        tool_call_id: String,
        tool_name: String,
        reason: String,
    },

    // === Iteration (1) ===
    IterationComplete {
        iteration: u32,
        tool_calls: Vec<ToolCall>,
        final_answer: Option<String>,
    },

    // === Plugin (1) ===
    PluginEvent {
        name: String,
        data: serde_json::Map<String, serde_json::Value>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_stream_event_creation() {
        let event = AgentStreamEvent::AgentStart {
            input: "test".to_string(),
        };
        match event {
            AgentStreamEvent::AgentStart { input } => {
                assert_eq!(input, "test");
            }
            _ => panic!("Expected AgentStart"),
        }
    }

    #[test]
    fn test_agent_stream_event_tool_call() {
        let event = AgentStreamEvent::ToolCallBegin {
            tool_call_id: "call_123".to_string(),
            tool_name: "get_weather".to_string(),
            arguments: r#"{"city": "Beijing"}"#.to_string(),
        };
        match event {
            AgentStreamEvent::ToolCallBegin {
                tool_call_id,
                tool_name,
                arguments,
            } => {
                assert_eq!(tool_call_id, "call_123");
                assert_eq!(tool_name, "get_weather");
                assert_eq!(arguments, r#"{"city": "Beijing"}"#);
            }
            _ => panic!("Expected ToolCallBegin"),
        }
    }

    #[test]
    fn test_agent_stream_event_iteration_complete() {
        let event = AgentStreamEvent::IterationComplete {
            iteration: 1,
            tool_calls: Vec::new(),
            final_answer: Some("The answer".to_string()),
        };
        match event {
            AgentStreamEvent::IterationComplete {
                iteration,
                final_answer,
                ..
            } => {
                assert_eq!(iteration, 1);
                assert_eq!(final_answer, Some("The answer".to_string()));
            }
            _ => panic!("Expected IterationComplete"),
        }
    }

    #[test]
    fn test_agent_stream_event_aborted() {
        let event = AgentStreamEvent::AgentAborted {
            reason: "max iterations".to_string(),
        };
        match event {
            AgentStreamEvent::AgentAborted { reason } => {
                assert_eq!(reason, "max iterations");
            }
            _ => panic!("Expected AgentAborted"),
        }
    }

    #[test]
    fn test_agent_stream_event_plugin_event() {
        use serde_json::Map;
        let mut data = Map::new();
        data.insert(
            "key".to_string(),
            serde_json::Value::String("value".to_string()),
        );

        let event = AgentStreamEvent::PluginEvent {
            name: "custom".to_string(),
            data,
        };
        match event {
            AgentStreamEvent::PluginEvent { name, .. } => {
                assert_eq!(name, "custom");
            }
            _ => panic!("Expected PluginEvent"),
        }
    }
}
