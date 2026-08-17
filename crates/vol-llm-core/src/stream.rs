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
    ResponseStart {
        model: String,
    },
    ResponseComplete {
        finish_reason: FinishReason,
    },

    // Content (text output)
    ContentDelta {
        delta: String,
    },
    ContentComplete {
        content: String,
    },

    // Thinking (model reasoning)
    ThinkingDelta {
        thinking: String,
    },
    ThinkingComplete {
        thinking: String,
    },

    // Tool calls
    ToolCallComplete {
        tool_call: ToolCall,
    },
    ToolCallArgumentDelta {
        tool_call_id: String,
        tool_name: String,
        delta: String,
    },

    // Usage
    UsageUpdate {
        usage: TokenUsage,
    },

    // Error handling
    Error {
        code: String,
        message: String,
    },
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
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
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
        Self::AgentStart {
            timestamp: chrono::Utc::now(),
            input,
        }
    }
    pub fn agent_complete() -> Self {
        Self::AgentComplete {
            timestamp: chrono::Utc::now(),
            response: None,
        }
    }
    pub fn agent_complete_with_response(response: serde_json::Value) -> Self {
        Self::AgentComplete {
            timestamp: chrono::Utc::now(),
            response: Some(response),
        }
    }
    pub fn agent_aborted(reason: String) -> Self {
        Self::AgentAborted {
            timestamp: chrono::Utc::now(),
            reason,
        }
    }
    pub fn max_iterations_reached(current_iteration: u32, max_iterations: u32) -> Self {
        Self::MaxIterationsReached {
            timestamp: chrono::Utc::now(),
            current_iteration,
            max_iterations,
        }
    }
    pub fn iteration_continued(from_iteration: u32) -> Self {
        Self::IterationContinued {
            timestamp: chrono::Utc::now(),
            from_iteration,
        }
    }
    pub fn llm_call_start(iteration: u32, messages: Vec<Message>) -> Self {
        Self::LLMCallStart {
            timestamp: chrono::Utc::now(),
            iteration,
            messages,
        }
    }
    pub fn llm_call_complete(model: String, usage: Option<TokenUsage>) -> Self {
        Self::LLMCallComplete {
            timestamp: chrono::Utc::now(),
            model,
            usage,
        }
    }
    pub fn llm_call_error(error: String) -> Self {
        Self::LLMCallError {
            timestamp: chrono::Utc::now(),
            error,
        }
    }
    pub fn thinking_start() -> Self {
        Self::ThinkingStart {
            timestamp: chrono::Utc::now(),
        }
    }
    pub fn thinking_delta(delta: String) -> Self {
        Self::ThinkingDelta {
            timestamp: chrono::Utc::now(),
            delta,
        }
    }
    pub fn thinking_complete(thinking: String) -> Self {
        Self::ThinkingComplete {
            timestamp: chrono::Utc::now(),
            thinking,
        }
    }
    pub fn content_start() -> Self {
        Self::ContentStart {
            timestamp: chrono::Utc::now(),
        }
    }
    pub fn content_delta(delta: String) -> Self {
        Self::ContentDelta {
            timestamp: chrono::Utc::now(),
            delta,
        }
    }
    pub fn content_complete(content: String) -> Self {
        Self::ContentComplete {
            timestamp: chrono::Utc::now(),
            content,
        }
    }
    pub fn tool_call_begin(tool_call_id: String, tool_name: String, arguments: String) -> Self {
        Self::ToolCallBegin {
            timestamp: chrono::Utc::now(),
            tool_call_id,
            tool_name,
            arguments,
        }
    }
    pub fn tool_call_complete(
        tool_call_id: String,
        tool_name: String,
        result: String,
        duration_ms: Option<u64>,
    ) -> Self {
        Self::ToolCallComplete {
            timestamp: chrono::Utc::now(),
            tool_call_id,
            tool_name,
            result,
            duration_ms,
        }
    }
    pub fn tool_call_error(
        tool_call_id: String,
        tool_name: String,
        error: String,
        duration_ms: Option<u64>,
    ) -> Self {
        Self::ToolCallError {
            timestamp: chrono::Utc::now(),
            tool_call_id,
            tool_name,
            error,
            duration_ms,
        }
    }
    pub fn tool_call_skipped(
        tool_call_id: String,
        tool_name: String,
        reason: String,
        duration_ms: Option<u64>,
    ) -> Self {
        Self::ToolCallSkipped {
            timestamp: chrono::Utc::now(),
            tool_call_id,
            tool_name,
            reason,
            duration_ms,
        }
    }
    pub fn tool_call_argument_delta(
        tool_call_id: String,
        tool_name: String,
        delta: String,
    ) -> Self {
        Self::ToolCallArgumentDelta {
            timestamp: chrono::Utc::now(),
            tool_call_id,
            tool_name,
            delta,
        }
    }
    pub fn iteration_complete(
        iteration: u32,
        tool_calls: Vec<ToolCall>,
        final_answer: Option<String>,
    ) -> Self {
        Self::IterationComplete {
            timestamp: chrono::Utc::now(),
            iteration,
            tool_calls,
            final_answer,
        }
    }
    pub fn plugin_event(name: String, data: serde_json::Map<String, serde_json::Value>) -> Self {
        Self::PluginEvent {
            timestamp: chrono::Utc::now(),
            name,
            data,
        }
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
        let event =
            AgentStreamEvent::iteration_complete(1, Vec::new(), Some("The answer".to_string()));
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

    #[test]
    fn test_agent_complete_events() {
        match AgentStreamEvent::agent_complete() {
            AgentStreamEvent::AgentComplete { response, .. } => {
                assert!(response.is_none());
            }
            _ => panic!("Expected AgentComplete"),
        }

        match AgentStreamEvent::agent_complete_with_response(serde_json::json!({"answer": 42})) {
            AgentStreamEvent::AgentComplete { response, .. } => {
                assert_eq!(response.as_ref().unwrap()["answer"], 42);
            }
            _ => panic!("Expected AgentComplete"),
        }
    }

    #[test]
    fn test_llm_call_events() {
        let messages = vec![Message::user("hi")];

        match AgentStreamEvent::llm_call_start(3, messages.clone()) {
            AgentStreamEvent::LLMCallStart {
                iteration,
                messages,
                ..
            } => {
                assert_eq!(iteration, 3);
                assert_eq!(messages.len(), 1);
            }
            _ => panic!("Expected LLMCallStart"),
        }

        match AgentStreamEvent::llm_call_complete(
            "qwen3.6-plus".to_string(),
            Some(TokenUsage {
                prompt_tokens: 5,
                completion_tokens: 7,
                total_tokens: 12,
                cached_tokens: None,
            }),
        ) {
            AgentStreamEvent::LLMCallComplete { model, usage, .. } => {
                assert_eq!(model, "qwen3.6-plus");
                assert_eq!(usage.unwrap().total_tokens, 12);
            }
            _ => panic!("Expected LLMCallComplete"),
        }

        match AgentStreamEvent::llm_call_error("boom".to_string()) {
            AgentStreamEvent::LLMCallError { error, .. } => {
                assert_eq!(error, "boom");
            }
            _ => panic!("Expected LLMCallError"),
        }
    }

    #[test]
    fn test_thinking_events() {
        match AgentStreamEvent::thinking_start() {
            AgentStreamEvent::ThinkingStart { .. } => {}
            _ => panic!("Expected ThinkingStart"),
        }
        match AgentStreamEvent::thinking_delta("step 1".to_string()) {
            AgentStreamEvent::ThinkingDelta { delta, .. } => {
                assert_eq!(delta, "step 1");
            }
            _ => panic!("Expected ThinkingDelta"),
        }
        match AgentStreamEvent::thinking_complete("full reasoning".to_string()) {
            AgentStreamEvent::ThinkingComplete { thinking, .. } => {
                assert_eq!(thinking, "full reasoning");
            }
            _ => panic!("Expected ThinkingComplete"),
        }
    }

    #[test]
    fn test_content_events() {
        match AgentStreamEvent::content_start() {
            AgentStreamEvent::ContentStart { .. } => {}
            _ => panic!("Expected ContentStart"),
        }
        match AgentStreamEvent::content_delta("Hello".to_string()) {
            AgentStreamEvent::ContentDelta { delta, .. } => {
                assert_eq!(delta, "Hello");
            }
            _ => panic!("Expected ContentDelta"),
        }
        match AgentStreamEvent::content_complete("Hello world".to_string()) {
            AgentStreamEvent::ContentComplete { content, .. } => {
                assert_eq!(content, "Hello world");
            }
            _ => panic!("Expected ContentComplete"),
        }
    }

    #[test]
    fn test_tool_result_events() {
        match AgentStreamEvent::tool_call_complete(
            "call_1".to_string(),
            "get_weather".to_string(),
            "{\"temp\": 20}".to_string(),
            Some(120),
        ) {
            AgentStreamEvent::ToolCallComplete {
                tool_call_id,
                tool_name,
                result,
                duration_ms,
                ..
            } => {
                assert_eq!(tool_call_id, "call_1");
                assert_eq!(tool_name, "get_weather");
                assert_eq!(result, "{\"temp\": 20}");
                assert_eq!(duration_ms, Some(120));
            }
            _ => panic!("Expected ToolCallComplete"),
        }

        match AgentStreamEvent::tool_call_error(
            "call_2".to_string(),
            "bash".to_string(),
            "exit code 1".to_string(),
            None,
        ) {
            AgentStreamEvent::ToolCallError {
                tool_call_id,
                tool_name,
                error,
                duration_ms,
                ..
            } => {
                assert_eq!(tool_call_id, "call_2");
                assert_eq!(tool_name, "bash");
                assert_eq!(error, "exit code 1");
                assert_eq!(duration_ms, None);
            }
            _ => panic!("Expected ToolCallError"),
        }

        match AgentStreamEvent::tool_call_skipped(
            "call_3".to_string(),
            "rm".to_string(),
            "disallowed".to_string(),
            Some(0),
        ) {
            AgentStreamEvent::ToolCallSkipped {
                tool_call_id,
                tool_name,
                reason,
                duration_ms,
                ..
            } => {
                assert_eq!(tool_call_id, "call_3");
                assert_eq!(tool_name, "rm");
                assert_eq!(reason, "disallowed");
                assert_eq!(duration_ms, Some(0));
            }
            _ => panic!("Expected ToolCallSkipped"),
        }

        match AgentStreamEvent::tool_call_argument_delta(
            "call_4".to_string(),
            "get_weather".to_string(),
            "{\"city\": ".to_string(),
        ) {
            AgentStreamEvent::ToolCallArgumentDelta {
                tool_call_id,
                tool_name,
                delta,
                ..
            } => {
                assert_eq!(tool_call_id, "call_4");
                assert_eq!(tool_name, "get_weather");
                assert_eq!(delta, "{\"city\": ");
            }
            _ => panic!("Expected ToolCallArgumentDelta"),
        }
    }

    fn all_event_variants() -> Vec<AgentStreamEvent> {
        vec![
            AgentStreamEvent::agent_start("input".to_string()),
            AgentStreamEvent::agent_complete(),
            AgentStreamEvent::agent_complete_with_response(serde_json::json!({})),
            AgentStreamEvent::agent_aborted("reason".to_string()),
            AgentStreamEvent::max_iterations_reached(1, 5),
            AgentStreamEvent::iteration_continued(3),
            AgentStreamEvent::llm_call_start(1, vec![]),
            AgentStreamEvent::llm_call_complete("m".to_string(), None),
            AgentStreamEvent::llm_call_error("e".to_string()),
            AgentStreamEvent::thinking_start(),
            AgentStreamEvent::thinking_delta("d".to_string()),
            AgentStreamEvent::thinking_complete("t".to_string()),
            AgentStreamEvent::content_start(),
            AgentStreamEvent::content_delta("d".to_string()),
            AgentStreamEvent::content_complete("c".to_string()),
            AgentStreamEvent::tool_call_begin("id".to_string(), "n".to_string(), "{}".to_string()),
            AgentStreamEvent::tool_call_complete(
                "id".to_string(),
                "n".to_string(),
                "r".to_string(),
                None,
            ),
            AgentStreamEvent::tool_call_error(
                "id".to_string(),
                "n".to_string(),
                "e".to_string(),
                None,
            ),
            AgentStreamEvent::tool_call_skipped(
                "id".to_string(),
                "n".to_string(),
                "s".to_string(),
                None,
            ),
            AgentStreamEvent::tool_call_argument_delta(
                "id".to_string(),
                "n".to_string(),
                "d".to_string(),
            ),
            AgentStreamEvent::iteration_complete(2, vec![], None),
            AgentStreamEvent::plugin_event("p".to_string(), serde_json::Map::new()),
        ]
    }

    #[test]
    fn test_timestamp_returns_value_for_all_variants() {
        let before = chrono::Utc::now();
        let events = all_event_variants();
        let after = chrono::Utc::now();
        for event in events {
            let ts = event.timestamp();
            assert!(
                ts >= before && ts <= after,
                "expected a real timestamp for {}, got {ts}",
                event.event_name()
            );
        }
    }

    #[test]
    fn test_event_name_all_variants() {
        let expected = [
            "AgentStart",
            "AgentComplete",
            "AgentComplete",
            "AgentAborted",
            "MaxIterationsReached",
            "IterationContinued",
            "LLMCallStart",
            "LLMCallComplete",
            "LLMCallError",
            "ThinkingStart",
            "ThinkingDelta",
            "ThinkingComplete",
            "ContentStart",
            "ContentDelta",
            "ContentComplete",
            "ToolCallBegin",
            "ToolCallComplete",
            "ToolCallError",
            "ToolCallSkipped",
            "ToolCallArgumentDelta",
            "IterationComplete",
            "PluginEvent",
        ];
        let variants = all_event_variants();
        for (event, name) in variants.iter().zip(expected.iter()) {
            assert_eq!(event.event_name(), *name);
        }
    }

    #[test]
    fn test_stream_event_data_serde_roundtrip() {
        let cases = vec![
            StreamEventData::ResponseStart {
                model: "m".to_string(),
            },
            StreamEventData::ResponseComplete {
                finish_reason: FinishReason::Stop,
            },
            StreamEventData::ContentDelta {
                delta: "d".to_string(),
            },
            StreamEventData::ContentComplete {
                content: "c".to_string(),
            },
            StreamEventData::ThinkingDelta {
                thinking: "t".to_string(),
            },
            StreamEventData::ThinkingComplete {
                thinking: "t".to_string(),
            },
            StreamEventData::ToolCallComplete {
                tool_call: ToolCall {
                    id: "id".to_string(),
                    name: "n".to_string(),
                    arguments: "{}".to_string(),
                    r#type: "function".to_string(),
                },
            },
            StreamEventData::ToolCallArgumentDelta {
                tool_call_id: "id".to_string(),
                tool_name: "n".to_string(),
                delta: "d".to_string(),
            },
            StreamEventData::UsageUpdate {
                usage: TokenUsage::default(),
            },
            StreamEventData::Error {
                code: "E1".to_string(),
                message: "msg".to_string(),
            },
        ];
        for data in cases {
            let json = serde_json::to_string(&data).unwrap();
            let parsed: StreamEventData = serde_json::from_str(&json).unwrap();
            match (data, parsed) {
                (
                    StreamEventData::ResponseStart { model: m1 },
                    StreamEventData::ResponseStart { model: m2 },
                ) => {
                    assert_eq!(m1, m2);
                }
                (
                    StreamEventData::Error {
                        code: c1,
                        message: m1,
                    },
                    StreamEventData::Error {
                        code: c2,
                        message: m2,
                    },
                ) => {
                    assert_eq!(c1, c2);
                    assert_eq!(m1, m2);
                }
                (
                    StreamEventData::ToolCallComplete { tool_call: t1 },
                    StreamEventData::ToolCallComplete { tool_call: t2 },
                ) => {
                    assert_eq!(t1.id, t2.id);
                    assert_eq!(t1.name, t2.name);
                    assert_eq!(t1.arguments, t2.arguments);
                }
                (other1, other2) => {
                    // All other variants are single-field and can be compared via debug
                    assert_eq!(format!("{other1:?}"), format!("{other2:?}"));
                }
            }
        }
    }

    #[test]
    fn test_stream_event_serde_roundtrip() {
        let event = StreamEvent {
            id: "evt_1".to_string(),
            data: StreamEventData::ContentDelta {
                delta: "hi".to_string(),
            },
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""type":"content_delta""#));
        let parsed: StreamEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "evt_1");
        assert_eq!(format!("{:?}", parsed.data), format!("{:?}", event.data));
    }

    #[tokio::test]
    async fn test_stream_receiver_recv() {
        let (tx, rx) = tokio::sync::mpsc::channel(4);
        let mut receiver = StreamReceiver::new(rx);

        let event = StreamEvent {
            id: "evt_1".to_string(),
            data: StreamEventData::ContentDelta {
                delta: "hi".to_string(),
            },
        };
        tx.send(Ok(event.clone())).await.unwrap();
        drop(tx); // close channel so recv returns None afterwards

        let received = receiver.recv().await.unwrap().unwrap();
        assert_eq!(received.id, "evt_1");
        assert_eq!(format!("{:?}", received.data), format!("{:?}", event.data));
        // Channel closed => no more events
        assert!(receiver.recv().await.is_none());
    }

    #[tokio::test]
    async fn test_stream_receiver_forwards_error() {
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        let mut receiver = StreamReceiver::new(rx);
        tx.send(Err(crate::LLMError::Timeout("boom".to_string())))
            .await
            .unwrap();
        drop(tx);
        let err = receiver.recv().await.unwrap().unwrap_err();
        assert!(err.to_string().contains("timeout"));
    }
}
