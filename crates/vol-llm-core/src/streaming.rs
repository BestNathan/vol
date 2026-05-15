//! Common streaming logic for LLM providers.
//!
//! Provides shared infrastructure for parsing SSE responses and accumulating
//! streaming chunks into complete events.

use crate::{FinishReason, LLMError, StreamEvent, StreamEventData, TokenUsage, ToolCall};

/// Internal event type that protocol parsers produce
pub enum ParsedEvent {
    ContentDelta(String),
    ContentComplete(String),
    ThinkingDelta(String),
    ThinkingComplete(String),
    ToolCallStart {
        index: usize,
        id: Option<String>,
        name: Option<String>,
    },
    ToolCallDelta { index: usize, delta: String },
    ToolCallComplete(ToolCall),
    Usage(TokenUsage),
    ResponseStart { model: String },
    ResponseComplete { finish_reason: FinishReason },
    /// Signals end of a content block; apply() finalizes pending tool_call/thinking
    ContentBlockStop,
}

/// Protocol-specific SSE parser
pub trait StreamProtocol: Send {
    fn parse_line(&self, line: &str) -> Option<Result<ParsedEvent, LLMError>>;
}

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
    pub(crate) index: usize,
    pub(crate) id: Option<String>,
    pub(crate) name: Option<String>,
    pub(crate) arguments: String,
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

    /// Apply a parsed event and return any stream events to emit
    pub fn apply(&mut self, event: &ParsedEvent) -> Vec<Result<StreamEvent, LLMError>> {
        match event {
            ParsedEvent::ContentDelta(text) => {
                self.content_buffer.push_str(text);
                vec![Ok(StreamEvent {
                    id: self.next_id(),
                    data: StreamEventData::ContentDelta { delta: text.clone() },
                })]
            }
            ParsedEvent::ContentComplete(content) => {
                vec![Ok(StreamEvent {
                    id: self.next_id(),
                    data: StreamEventData::ContentComplete {
                        content: content.clone(),
                    },
                })]
            }
            ParsedEvent::ThinkingDelta(text) => {
                self.thinking_buffer.push_str(text);
                vec![Ok(StreamEvent {
                    id: self.next_id(),
                    data: StreamEventData::ThinkingDelta {
                        thinking: text.clone(),
                    },
                })]
            }
            ParsedEvent::ThinkingComplete(text) => {
                vec![Ok(StreamEvent {
                    id: self.next_id(),
                    data: StreamEventData::ThinkingComplete {
                        thinking: text.clone(),
                    },
                })]
            }
            ParsedEvent::ToolCallStart { id, name, .. } => {
                self.current_tool_call = Some(ToolCallBuilder {
                    index: self.tool_call_index,
                    id: id.clone(),
                    name: name.clone(),
                    arguments: String::new(),
                });
                self.tool_call_index += 1;
                Vec::new()
            }
            ParsedEvent::ToolCallDelta { delta, .. } => {
                if let Some(ref mut builder) = self.current_tool_call {
                    builder.arguments.push_str(delta);
                }
                Vec::new()
            }
            ParsedEvent::ToolCallComplete(tool_call) => {
                vec![Ok(StreamEvent {
                    id: self.next_id(),
                    data: StreamEventData::ToolCallComplete {
                        tool_call: tool_call.clone(),
                    },
                })]
            }
            ParsedEvent::Usage(usage) => {
                vec![Ok(StreamEvent {
                    id: self.next_id(),
                    data: StreamEventData::UsageUpdate {
                        usage: usage.clone(),
                    },
                })]
            }
            ParsedEvent::ResponseStart { model } => {
                vec![Ok(StreamEvent {
                    id: self.next_id(),
                    data: StreamEventData::ResponseStart { model: model.clone() },
                })]
            }
            ParsedEvent::ContentBlockStop => {
                let mut events = Vec::new();
                // Finalize any tool call that just completed
                if let Some(builder) = self.current_tool_call.take() {
                    if let Some(tool_call) = builder.build() {
                        events.push(Ok(StreamEvent {
                            id: self.next_id(),
                            data: StreamEventData::ToolCallComplete { tool_call },
                        }));
                    }
                }
                // Finalize any thinking that just completed
                if !self.thinking_buffer.is_empty() {
                    let thinking = std::mem::take(&mut self.thinking_buffer);
                    events.push(Ok(StreamEvent {
                        id: self.next_id(),
                        data: StreamEventData::ThinkingComplete { thinking },
                    }));
                }
                events
            }
            ParsedEvent::ResponseComplete { finish_reason } => {
                vec![Ok(StreamEvent {
                    id: self.next_id(),
                    data: StreamEventData::ResponseComplete {
                        finish_reason: *finish_reason,
                    },
                })]
            }
        }
    }

    /// Process a raw SSE line through a protocol parser
    pub fn process_sse(
        &mut self,
        protocol: &impl StreamProtocol,
        line: &str,
    ) -> Vec<Result<StreamEvent, LLMError>> {
        match protocol.parse_line(line) {
            Some(Ok(event)) => self.apply(&event),
            Some(Err(e)) => vec![Err(e)],
            None => Vec::new(),
        }
    }

    /// Process a single SSE line from Anthropic API (backward compat wrapper)
    pub fn process_anthropic_sse(&mut self, line: &str) -> Vec<Result<StreamEvent, LLMError>> {
        self.process_sse(&AnthropicProtocol, line)
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

/// Anthropic-specific SSE parser
pub struct AnthropicProtocol;

impl StreamProtocol for AnthropicProtocol {
    fn parse_line(&self, line: &str) -> Option<Result<ParsedEvent, LLMError>> {
        let line = line.trim();
        if line.is_empty() || !line.starts_with("data:") {
            return None;
        }
        let json_str = line.strip_prefix("data:")?.trim();
        let data: serde_json::Value = match serde_json::from_str(json_str) {
            Ok(v) => v,
            Err(_) => return None,
        };
        let event_type = data["type"].as_str()?;

        match event_type {
            "message_start" => Some(Ok(ParsedEvent::ResponseStart {
                model: data["message"]["model"]
                    .as_str()
                    .unwrap_or("unknown")
                    .to_string(),
            })),
            "content_block_start" => {
                let block_type = data["content_block"]["type"].as_str()?;
                match block_type {
                    "tool_use" => Some(Ok(ParsedEvent::ToolCallStart {
                        index: 0,
                        id: data["content_block"]["id"].as_str().map(|s| s.to_string()),
                        name: data["content_block"]["name"].as_str().map(|s| s.to_string()),
                    })),
                    _ => None,
                }
            }
            "content_block_delta" => {
                if let Some(thinking) = data["delta"]["thinking"].as_str() {
                    Some(Ok(ParsedEvent::ThinkingDelta(thinking.to_string())))
                } else if let Some(text) = data["delta"]["text"].as_str() {
                    Some(Ok(ParsedEvent::ContentDelta(text.to_string())))
                } else if let Some(input) = data["delta"]["partial_json"].as_str() {
                    Some(Ok(ParsedEvent::ToolCallDelta {
                        index: 0,
                        delta: input.to_string(),
                    }))
                } else {
                    None
                }
            }
            "content_block_stop" => Some(Ok(ParsedEvent::ContentBlockStop)),
            "message_delta" => {
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
                Some(Ok(ParsedEvent::Usage(usage)))
            }
            "message_stop" => {
                let finish_reason = match data["stop_reason"].as_str() {
                    Some("end_turn") | Some("stop_sequence") => FinishReason::Stop,
                    Some("max_tokens") => FinishReason::Length,
                    Some("tool_use") => FinishReason::ToolCalls,
                    _ => FinishReason::Other,
                };
                Some(Ok(ParsedEvent::ResponseComplete { finish_reason }))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_streaming_session_accumulates_content() {
        let mut session = StreamingSession::new();
        let protocol = AnthropicProtocol;

        let delta1 = r#"data: {"type": "content_block_delta", "delta": {"text": "Hello"}}"#;
        let delta2 = r#"data: {"type": "content_block_delta", "delta": {"text": " world"}}"#;

        let events1 = session.process_sse(&protocol, delta1);
        let events2 = session.process_sse(&protocol, delta2);

        assert!(!events1.is_empty());
        assert!(!events2.is_empty());
        assert_eq!(session.content_buffer, "Hello world");
    }

    #[test]
    fn test_tool_call_builder_accumulates_arguments() {
        let mut session = StreamingSession::new();
        let protocol = AnthropicProtocol;

        let start = r#"data: {"type": "content_block_start", "content_block": {"type": "tool_use", "id": "tool_1", "name": "get_weather"}}"#;
        session.process_sse(&protocol, start);

        let delta1 = r#"data: {"type": "content_block_delta", "delta": {"partial_json": "{\"city\": \"Beijing"}}"#;
        let delta2 = r#"data: {"type": "content_block_delta", "delta": {"partial_json": "\"}"}}"#;
        session.process_sse(&protocol, delta1);
        session.process_sse(&protocol, delta2);

        let stop = r#"data: {"type": "content_block_stop"}"#;
        let events = session.process_sse(&protocol, stop);

        assert!(!events.is_empty(), "Expected events from stop");
        let finalize_events = session.finalize();
        let all_events: Vec<_> = events.into_iter().chain(finalize_events).collect();

        let tool_call_event = all_events.iter().find(|e| {
            matches!(
                e,
                Ok(StreamEvent {
                    data: StreamEventData::ToolCallComplete { .. },
                    ..
                })
            )
        });
        assert!(tool_call_event.is_some(), "Expected ToolCallComplete event");
    }

    #[test]
    fn test_parse_message_start() {
        let mut session = StreamingSession::new();
        let protocol = AnthropicProtocol;

        let start = r#"data: {"type": "message_start", "message": {"model": "qwen3.5-plus"}}"#;
        let events = session.process_sse(&protocol, start);

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
        let protocol = AnthropicProtocol;

        assert!(session.process_sse(&protocol, "").is_empty());
        assert!(session.process_sse(&protocol, "   ").is_empty());
        assert!(session.process_sse(&protocol, ": ping").is_empty());
        assert!(session.process_sse(&protocol, "data: {invalid json}").is_empty());
    }

    #[test]
    fn test_apply_content_delta() {
        let mut session = StreamingSession::new();
        let events = session.apply(&ParsedEvent::ContentDelta("hello".to_string()));
        assert_eq!(events.len(), 1);
        assert_eq!(session.content_buffer, "hello");
    }

    #[test]
    fn test_process_sse_with_openai_style_delta() {
        let mut session = StreamingSession::new();
        let events = session.apply(&ParsedEvent::ContentDelta("test".to_string()));
        assert_eq!(session.content_buffer, "test");
        assert_eq!(events.len(), 1);
    }
}
