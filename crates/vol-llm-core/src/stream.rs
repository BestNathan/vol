//! Streaming response types.

use crate::{FinishReason, Message, TokenUsage, ToolCall};
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
    ToolCallArgumentDelta {
        tool_call_id: String,
        tool_name: String,
        delta: String,
    },

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
#[derive(Debug, Clone, Serialize)]
pub enum AgentStreamEvent {
    // === Lifecycle (3) ===
    AgentStart {
        timestamp: chrono::DateTime<chrono::Utc>,
        input: String,
    },
    AgentComplete {
        timestamp: chrono::DateTime<chrono::Utc>,
        response: Option<serde_json::Value>,
    },
    AgentAborted {
        timestamp: chrono::DateTime<chrono::Utc>,
        reason: String,
    },

    /// Emitted when max iterations is reached, before asking for continuation.
    MaxIterationsReached {
        timestamp: chrono::DateTime<chrono::Utc>,
        current_iteration: u32,
        max_iterations: u32,
    },

    /// Emitted when user approves continuation and iteration counter resets.
    IterationContinued {
        timestamp: chrono::DateTime<chrono::Utc>,
        from_iteration: u32,
    },

    // === LLM Call (3) ===
    LLMCallStart {
        timestamp: chrono::DateTime<chrono::Utc>,
        iteration: u32,
        messages: Vec<Message>,
    },
    LLMCallComplete {
        timestamp: chrono::DateTime<chrono::Utc>,
        model: String,
        usage: Option<TokenUsage>,
    },
    LLMCallError {
        timestamp: chrono::DateTime<chrono::Utc>,
        error: String,
    },

    // === Streaming: Thinking (3) ===
    ThinkingStart {
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    ThinkingDelta {
        timestamp: chrono::DateTime<chrono::Utc>,
        delta: String,
    },
    ThinkingComplete {
        timestamp: chrono::DateTime<chrono::Utc>,
        thinking: String,
    },

    // === Streaming: Content (3) ===
    ContentStart {
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    ContentDelta {
        timestamp: chrono::DateTime<chrono::Utc>,
        delta: String,
    },
    ContentComplete {
        timestamp: chrono::DateTime<chrono::Utc>,
        content: String,
    },

    // === Tool Execution (4) ===
    ToolCallBegin {
        timestamp: chrono::DateTime<chrono::Utc>,
        tool_call_id: String,
        tool_name: String,
        arguments: String,
    },
    ToolCallComplete {
        timestamp: chrono::DateTime<chrono::Utc>,
        tool_call_id: String,
        tool_name: String,
        result: String,
        duration_ms: Option<u64>,
    },
    ToolCallError {
        timestamp: chrono::DateTime<chrono::Utc>,
        tool_call_id: String,
        tool_name: String,
        error: String,
        duration_ms: Option<u64>,
    },
    ToolCallSkipped {
        timestamp: chrono::DateTime<chrono::Utc>,
        tool_call_id: String,
        tool_name: String,
        reason: String,
        duration_ms: Option<u64>,
    },

    // === Tool Argument Streaming (1) ===
    ToolCallArgumentDelta {
        timestamp: chrono::DateTime<chrono::Utc>,
        tool_call_id: String,
        tool_name: String,
        delta: String,
    },

    // === Iteration (1) ===
    IterationComplete {
        timestamp: chrono::DateTime<chrono::Utc>,
        iteration: u32,
        tool_calls: Vec<ToolCall>,
        final_answer: Option<String>,
    },

    // === Plugin (1) ===
    PluginEvent {
        timestamp: chrono::DateTime<chrono::Utc>,
        name: String,
        data: serde_json::Map<String, serde_json::Value>,
    },
}

impl AgentStreamEvent {
    pub fn agent_start(input: String) -> Self {
        Self::AgentStart { timestamp: chrono::Utc::now(), input }
    }
    pub fn agent_complete() -> Self {
        Self::AgentComplete { timestamp: chrono::Utc::now(), response: None }
    }
    pub fn agent_complete_with_response(response: serde_json::Value) -> Self {
        Self::AgentComplete { timestamp: chrono::Utc::now(), response: Some(response) }
    }
    pub fn agent_aborted(reason: String) -> Self {
        Self::AgentAborted { timestamp: chrono::Utc::now(), reason }
    }
    pub fn max_iterations_reached(current_iteration: u32, max_iterations: u32) -> Self {
        Self::MaxIterationsReached { timestamp: chrono::Utc::now(), current_iteration, max_iterations }
    }
    pub fn iteration_continued(from_iteration: u32) -> Self {
        Self::IterationContinued { timestamp: chrono::Utc::now(), from_iteration }
    }
    pub fn llm_call_start(iteration: u32, messages: Vec<Message>) -> Self {
        Self::LLMCallStart { timestamp: chrono::Utc::now(), iteration, messages }
    }
    pub fn llm_call_complete(model: String, usage: Option<TokenUsage>) -> Self {
        Self::LLMCallComplete { timestamp: chrono::Utc::now(), model, usage }
    }
    pub fn llm_call_error(error: String) -> Self {
        Self::LLMCallError { timestamp: chrono::Utc::now(), error }
    }
    pub fn thinking_start() -> Self {
        Self::ThinkingStart { timestamp: chrono::Utc::now() }
    }
    pub fn thinking_delta(delta: String) -> Self {
        Self::ThinkingDelta { timestamp: chrono::Utc::now(), delta }
    }
    pub fn thinking_complete(thinking: String) -> Self {
        Self::ThinkingComplete { timestamp: chrono::Utc::now(), thinking }
    }
    pub fn content_start() -> Self {
        Self::ContentStart { timestamp: chrono::Utc::now() }
    }
    pub fn content_delta(delta: String) -> Self {
        Self::ContentDelta { timestamp: chrono::Utc::now(), delta }
    }
    pub fn content_complete(content: String) -> Self {
        Self::ContentComplete { timestamp: chrono::Utc::now(), content }
    }
    pub fn tool_call_begin(tool_call_id: String, tool_name: String, arguments: String) -> Self {
        Self::ToolCallBegin { timestamp: chrono::Utc::now(), tool_call_id, tool_name, arguments }
    }
    pub fn tool_call_complete(tool_call_id: String, tool_name: String, result: String, duration_ms: Option<u64>) -> Self {
        Self::ToolCallComplete { timestamp: chrono::Utc::now(), tool_call_id, tool_name, result, duration_ms }
    }
    pub fn tool_call_error(tool_call_id: String, tool_name: String, error: String, duration_ms: Option<u64>) -> Self {
        Self::ToolCallError { timestamp: chrono::Utc::now(), tool_call_id, tool_name, error, duration_ms }
    }
    pub fn tool_call_skipped(tool_call_id: String, tool_name: String, reason: String, duration_ms: Option<u64>) -> Self {
        Self::ToolCallSkipped { timestamp: chrono::Utc::now(), tool_call_id, tool_name, reason, duration_ms }
    }
    pub fn tool_call_argument_delta(tool_call_id: String, tool_name: String, delta: String) -> Self {
        Self::ToolCallArgumentDelta {
            timestamp: chrono::Utc::now(),
            tool_call_id,
            tool_name,
            delta,
        }
    }
    pub fn iteration_complete(iteration: u32, tool_calls: Vec<ToolCall>, final_answer: Option<String>) -> Self {
        Self::IterationComplete { timestamp: chrono::Utc::now(), iteration, tool_calls, final_answer }
    }
    pub fn plugin_event(name: String, data: serde_json::Map<String, serde_json::Value>) -> Self {
        Self::PluginEvent { timestamp: chrono::Utc::now(), name, data }
    }

    /// Extract the timestamp from any event variant.
    pub fn timestamp(&self) -> chrono::DateTime<chrono::Utc> {
        match self {
            Self::AgentStart { timestamp, .. } => *timestamp,
            Self::AgentComplete { timestamp, .. } => *timestamp,
            Self::AgentAborted { timestamp, .. } => *timestamp,
            Self::MaxIterationsReached { timestamp, .. } => *timestamp,
            Self::IterationContinued { timestamp, .. } => *timestamp,
            Self::LLMCallStart { timestamp, .. } => *timestamp,
            Self::LLMCallComplete { timestamp, .. } => *timestamp,
            Self::LLMCallError { timestamp, .. } => *timestamp,
            Self::ThinkingStart { timestamp, .. } => *timestamp,
            Self::ThinkingDelta { timestamp, .. } => *timestamp,
            Self::ThinkingComplete { timestamp, .. } => *timestamp,
            Self::ContentStart { timestamp, .. } => *timestamp,
            Self::ContentDelta { timestamp, .. } => *timestamp,
            Self::ContentComplete { timestamp, .. } => *timestamp,
            Self::ToolCallBegin { timestamp, .. } => *timestamp,
            Self::ToolCallComplete { timestamp, .. } => *timestamp,
            Self::ToolCallError { timestamp, .. } => *timestamp,
            Self::ToolCallSkipped { timestamp, .. } => *timestamp,
            Self::ToolCallArgumentDelta { timestamp, .. } => *timestamp,
            Self::IterationComplete { timestamp, .. } => *timestamp,
            Self::PluginEvent { timestamp, .. } => *timestamp,
        }
    }

    /// Returns the event variant name as a string, suitable for logging.
    pub fn event_name(&self) -> &'static str {
        match self {
            Self::AgentStart { .. } => "AgentStart",
            Self::AgentComplete { .. } => "AgentComplete",
            Self::AgentAborted { .. } => "AgentAborted",
            Self::MaxIterationsReached { .. } => "MaxIterationsReached",
            Self::IterationContinued { .. } => "IterationContinued",
            Self::LLMCallStart { .. } => "LLMCallStart",
            Self::LLMCallComplete { .. } => "LLMCallComplete",
            Self::LLMCallError { .. } => "LLMCallError",
            Self::ThinkingStart { .. } => "ThinkingStart",
            Self::ThinkingDelta { .. } => "ThinkingDelta",
            Self::ThinkingComplete { .. } => "ThinkingComplete",
            Self::ContentStart { .. } => "ContentStart",
            Self::ContentDelta { .. } => "ContentDelta",
            Self::ContentComplete { .. } => "ContentComplete",
            Self::ToolCallBegin { .. } => "ToolCallBegin",
            Self::ToolCallComplete { .. } => "ToolCallComplete",
            Self::ToolCallError { .. } => "ToolCallError",
            Self::ToolCallSkipped { .. } => "ToolCallSkipped",
            Self::ToolCallArgumentDelta { .. } => "ToolCallArgumentDelta",
            Self::IterationComplete { .. } => "IterationComplete",
            Self::PluginEvent { .. } => "PluginEvent",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_stream_event_creation() {
        let event = AgentStreamEvent::agent_start("test".to_string());
        match event {
            AgentStreamEvent::AgentStart { input, .. } => {
                assert_eq!(input, "test");
            }
            _ => panic!("Expected AgentStart"),
        }
    }

    #[test]
    fn test_agent_stream_event_tool_call() {
        let event = AgentStreamEvent::tool_call_begin(
            "call_123".to_string(),
            "get_weather".to_string(),
            r#"{"city": "Beijing"}"#.to_string(),
        );
        match event {
            AgentStreamEvent::ToolCallBegin {
                tool_call_id,
                tool_name,
                arguments,
                ..
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
        let event = AgentStreamEvent::iteration_complete(
            1,
            Vec::new(),
            Some("The answer".to_string()),
        );
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
        let event = AgentStreamEvent::agent_aborted("max iterations".to_string());
        match event {
            AgentStreamEvent::AgentAborted { reason, .. } => {
                assert_eq!(reason, "max iterations");
            }
            _ => panic!("Expected AgentAborted"),
        }
    }

    #[test]
    fn test_agent_stream_event_max_iterations() {
        let event = AgentStreamEvent::max_iterations_reached(5, 10);
        match event {
            AgentStreamEvent::MaxIterationsReached {
                current_iteration,
                max_iterations,
                ..
            } => {
                assert_eq!(current_iteration, 5);
                assert_eq!(max_iterations, 10);
            }
            _ => panic!("Expected MaxIterationsReached"),
        }
    }

    #[test]
    fn test_agent_stream_event_iteration_continued() {
        let event = AgentStreamEvent::iteration_continued(10);
        match event {
            AgentStreamEvent::IterationContinued { from_iteration, .. } => {
                assert_eq!(from_iteration, 10);
            }
            _ => panic!("Expected IterationContinued"),
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

        let event = AgentStreamEvent::plugin_event("custom".to_string(), data);
        match event {
            AgentStreamEvent::PluginEvent { name, .. } => {
                assert_eq!(name, "custom");
            }
            _ => panic!("Expected PluginEvent"),
        }
    }
}
