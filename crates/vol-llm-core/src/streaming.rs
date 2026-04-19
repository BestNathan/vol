//! Common streaming logic for LLM providers.
//!
//! Provides shared infrastructure for parsing SSE responses and accumulating
//! streaming chunks into complete events.

use crate::{FinishReason, LLMError, StreamEvent, StreamEventData, TokenUsage, ToolCall};
use serde_json::Value;

/// Manages state for a streaming session
pub struct StreamingSession {
    /// Accumulated text content
    content_buffer: String,
    /// Accumulated thinking content
    thinking_buffer: String,
    /// Current tool call being built (if any)
    current_tool_call: Option<ToolCallBuilder>,
    /// Index for next tool call
    tool_call_index: usize,
    /// Event ID counter
    event_id: usize,
}

/// Builder for accumulating tool call chunks
#[allow(dead_code)]
pub struct ToolCallBuilder {
    index: usize,
    id: Option<String>,
    name: Option<String>,
    arguments: String,
}

impl StreamingSession {
    /// Create a new streaming session
    pub fn new() -> Self {
        Self {
            content_buffer: String::new(),
            thinking_buffer: String::new(),
            current_tool_call: None,
            tool_call_index: 0,
            event_id: 0,
        }
    }

    /// Generate next event ID
    fn next_id(&mut self) -> String {
        self.event_id += 1;
        format!("evt_{}", self.event_id)
    }

    /// Process a single SSE line from Anthropic API
    /// Returns a vector of StreamEvents (may be empty if the line produces no events)
    pub fn process_anthropic_sse(&mut self, line: &str) -> Vec<Result<StreamEvent, LLMError>> {
        // Skip empty lines and non-data lines
        let line = line.trim();
        if line.is_empty() || !line.starts_with("data:") {
            return Vec::new();
        }

        // Extract JSON from "data: {...}"
        let json_str = match line.strip_prefix("data:") {
            Some(s) => s.trim(),
            None => return Vec::new(),
        };
        let data: Value = match serde_json::from_str(json_str) {
            Ok(v) => v,
            Err(_) => return Vec::new(),
        };
        let event_type = match data["type"].as_str() {
            Some(s) => s,
            None => return Vec::new(),
        };

        match event_type {
            "message_start" => vec![self.handle_message_start(&data)],
            "content_block_start" => self.handle_content_block_start(&data).into_iter().collect(),
            "content_block_delta" => self.handle_content_block_delta(&data).into_iter().collect(),
            "content_block_stop" => self.handle_content_block_stop(&data),
            "message_delta" => vec![self.handle_message_delta(&data)],
            "message_stop" => self.handle_message_stop(&data),
            _ => Vec::new(),
        }
    }

    fn handle_message_start(&mut self, data: &Value) -> Result<StreamEvent, LLMError> {
        let model = data["message"]["model"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();

        Ok(StreamEvent {
            id: self.next_id(),
            data: StreamEventData::ResponseStart { model },
        })
    }

    fn handle_content_block_start(
        &mut self,
        data: &Value,
    ) -> Option<Result<StreamEvent, LLMError>> {
        let block_type = data["content_block"]["type"].as_str()?;

        match block_type {
            "thinking" => {
                // Initialize thinking accumulator
                None
            }
            "tool_use" => {
                // Start building a new tool call
                let id = data["content_block"]["id"].as_str().map(|s| s.to_string());
                let name = data["content_block"]["name"]
                    .as_str()
                    .map(|s| s.to_string());

                self.current_tool_call = Some(ToolCallBuilder {
                    index: self.tool_call_index,
                    id,
                    name,
                    arguments: String::new(),
                });
                self.tool_call_index += 1;
                None
            }
            "text" | _ => None,
        }
    }

    fn handle_content_block_delta(
        &mut self,
        data: &Value,
    ) -> Option<Result<StreamEvent, LLMError>> {
        // Check for thinking delta
        if let Some(thinking) = data["delta"]["thinking"].as_str() {
            self.thinking_buffer.push_str(thinking);
            return Some(Ok(StreamEvent {
                id: self.next_id(),
                data: StreamEventData::ThinkingDelta {
                    thinking: thinking.to_string(),
                },
            }));
        }

        // Check for text delta
        if let Some(text) = data["delta"]["text"].as_str() {
            self.content_buffer.push_str(text);
            return Some(Ok(StreamEvent {
                id: self.next_id(),
                data: StreamEventData::ContentDelta {
                    delta: text.to_string(),
                },
            }));
        }

        // Check for tool_use input delta (partial JSON)
        if let Some(input) = data["delta"]["partial_json"].as_str() {
            if let Some(ref mut builder) = self.current_tool_call {
                builder.arguments.push_str(input);
                let tool_call_id = builder.id.clone().unwrap_or_default();
                let tool_name = builder.name.clone().unwrap_or_default();
                return Some(Ok(StreamEvent {
                    id: self.next_id(),
                    data: StreamEventData::ToolCallArgumentDelta {
                        tool_call_id,
                        tool_name,
                        delta: input.to_string(),
                    },
                }));
            }
        }

        None
    }

    fn handle_content_block_stop(&mut self, _data: &Value) -> Vec<Result<StreamEvent, LLMError>> {
        let mut events = Vec::new();

        // Check if thinking just completed
        if !self.thinking_buffer.is_empty() {
            let thinking = std::mem::take(&mut self.thinking_buffer);
            events.push(Ok(StreamEvent {
                id: self.next_id(),
                data: StreamEventData::ThinkingComplete { thinking },
            }));
        }

        // Check if tool call just completed
        if let Some(builder) = self.current_tool_call.take() {
            if let Some(tool_call) = builder.build() {
                events.push(Ok(StreamEvent {
                    id: self.next_id(),
                    data: StreamEventData::ToolCallComplete { tool_call },
                }));
            }
        }

        events
    }

    fn handle_message_delta(&mut self, data: &Value) -> Result<StreamEvent, LLMError> {
        let usage = if let Some(usage_data) = data.get("usage") {
            TokenUsage {
                prompt_tokens: usage_data["input_tokens"].as_u64().unwrap_or(0) as u32,
                completion_tokens: usage_data["output_tokens"].as_u64().unwrap_or(0) as u32,
                total_tokens: (usage_data["input_tokens"].as_u64().unwrap_or(0)
                    + usage_data["output_tokens"].as_u64().unwrap_or(0))
                    as u32,
                cached_tokens: None,
            }
        } else {
            TokenUsage::default()
        };

        Ok(StreamEvent {
            id: self.next_id(),
            data: StreamEventData::UsageUpdate { usage },
        })
    }

    fn handle_message_stop(&mut self, data: &Value) -> Vec<Result<StreamEvent, LLMError>> {
        let mut events = Vec::new();

        if !self.content_buffer.is_empty() {
            let content = std::mem::take(&mut self.content_buffer);
            events.push(Ok(StreamEvent {
                id: self.next_id(),
                data: StreamEventData::ContentComplete { content },
            }));
        }

        let finish_reason = match data["stop_reason"].as_str() {
            Some("end_turn") | Some("stop_sequence") => FinishReason::Stop,
            Some("max_tokens") => FinishReason::Length,
            Some("tool_use") => FinishReason::ToolCalls,
            _ => FinishReason::Other,
        };

        events.push(Ok(StreamEvent {
            id: self.next_id(),
            data: StreamEventData::ResponseComplete { finish_reason },
        }));

        events
    }

    /// Finalize the session and emit any remaining aggregate events
    pub fn finalize(&mut self) -> Vec<Result<StreamEvent, LLMError>> {
        let mut events = Vec::new();

        if !self.content_buffer.is_empty() {
            let content = std::mem::take(&mut self.content_buffer);
            events.push(Ok(StreamEvent {
                id: self.next_id(),
                data: StreamEventData::ContentComplete { content },
            }));
        }

        if !self.thinking_buffer.is_empty() {
            let thinking = std::mem::take(&mut self.thinking_buffer);
            events.push(Ok(StreamEvent {
                id: self.next_id(),
                data: StreamEventData::ThinkingComplete { thinking },
            }));
        }

        events
    }
}

impl Default for StreamingSession {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolCallBuilder {
    /// Build the complete ToolCall from accumulated chunks
    pub fn build(self) -> Option<ToolCall> {
        Some(ToolCall {
            id: self.id?,
            name: self.name?,
            arguments: self.arguments,
            r#type: "function".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_streaming_session_accumulates_content() {
        let mut session = StreamingSession::new();

        let delta1 = r#"data: {"type": "content_block_delta", "delta": {"text": "Hello"}}"#;
        let delta2 = r#"data: {"type": "content_block_delta", "delta": {"text": " world"}}"#;

        let events1 = session.process_anthropic_sse(delta1);
        let events2 = session.process_anthropic_sse(delta2);

        assert!(!events1.is_empty());
        assert!(!events2.is_empty());
        assert_eq!(session.content_buffer, "Hello world");
    }

    #[test]
    fn test_tool_call_builder_accumulates_arguments() {
        let mut session = StreamingSession::new();

        let start = r#"data: {"type": "content_block_start", "content_block": {"type": "tool_use", "id": "tool_1", "name": "get_weather"}}"#;
        session.process_anthropic_sse(start);

        let delta1 = r#"data: {"type": "content_block_delta", "delta": {"partial_json": "{\"city\": \"Beijing"}}"#;
        let delta2 = r#"data: {"type": "content_block_delta", "delta": {"partial_json": "\"}"}}"#;
        session.process_anthropic_sse(delta1);
        session.process_anthropic_sse(delta2);

        let stop = r#"data: {"type": "content_block_stop"}"#;
        let events = session.process_anthropic_sse(stop);

        assert!(!events.is_empty());
        if let Ok(StreamEvent {
            data: StreamEventData::ToolCallComplete { tool_call },
            ..
        }) = &events[0]
        {
            assert_eq!(tool_call.id, "tool_1");
            assert_eq!(tool_call.name, "get_weather");
            assert_eq!(tool_call.arguments, r#"{"city": "Beijing"}"#);
        } else {
            panic!("Expected ToolCallComplete event");
        }
    }

    #[test]
    fn test_parse_sse_line() {
        let mut session = StreamingSession::new();

        let start = r#"data: {"type": "message_start", "message": {"model": "qwen3.5-plus"}}"#;
        let events = session.process_anthropic_sse(start);

        assert!(!events.is_empty());
        if let Ok(StreamEvent {
            data: StreamEventData::ResponseStart { model },
            ..
        }) = &events[0]
        {
            assert_eq!(model, "qwen3.5-plus");
        } else {
            panic!("Expected ResponseStart event");
        }
    }

    #[test]
    fn test_empty_and_malformed_lines() {
        let mut session = StreamingSession::new();

        assert!(session.process_anthropic_sse("").is_empty());
        assert!(session.process_anthropic_sse("   ").is_empty());
        assert!(session.process_anthropic_sse(": ping").is_empty());
        assert!(session
            .process_anthropic_sse("data: {invalid json}")
            .is_empty());
    }

    #[test]
    fn test_tool_call_argument_delta_emitted() {
        let mut session = StreamingSession::new();

        let start = r#"data: {"type": "content_block_start", "content_block": {"type": "tool_use", "id": "call_1", "name": "get_weather"}}"#;
        session.process_anthropic_sse(start);

        let delta = r#"data: {"type": "content_block_delta", "delta": {"partial_json": "{\"city\": \"Beijing\"}"}}"#;
        let events = session.process_anthropic_sse(delta);

        assert!(!events.is_empty(), "Expected ToolCallArgumentDelta event");
        if let Ok(StreamEvent {
            data: StreamEventData::ToolCallArgumentDelta {
                tool_call_id,
                tool_name,
                delta,
            },
            ..
        }) = &events[0]
        {
            assert_eq!(tool_call_id, "call_1");
            assert_eq!(tool_name, "get_weather");
            assert_eq!(delta, r#"{"city": "Beijing"}"#);
        } else {
            panic!("Expected ToolCallArgumentDelta, got: {:?}", events[0]);
        }
    }
}
