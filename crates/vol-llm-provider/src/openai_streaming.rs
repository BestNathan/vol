//! OpenAI-compatible SSE stream parser.
//!
//! Parses OpenAI chat completions SSE format: `data: {...}` lines with
//! `[DONE]` sentinel. Each event contains `choices[0].delta` for content
//! and tool call streaming.

use vol_llm_core::{FinishReason, LLMError, ParsedEvent, StreamProtocol, TokenUsage};
use serde_json::Value;

/// OpenAI-specific SSE protocol parser
pub struct OpenaiStreamParser;

impl StreamProtocol for OpenaiStreamParser {
    fn parse_line(&self, line: &str) -> Option<Result<ParsedEvent, LLMError>> {
        let line = line.trim();
        if line.is_empty() || !line.starts_with("data:") {
            return None;
        }

        let data_str = line.strip_prefix("data:")?.trim();

        // Check for [DONE] sentinel
        if data_str == "[DONE]" {
            return Some(Ok(ParsedEvent::ResponseComplete {
                finish_reason: FinishReason::Stop,
            }));
        }

        // Parse JSON
        let data: Value = match serde_json::from_str(data_str) {
            Ok(v) => v,
            Err(_) => return None,
        };

        // Extract usage if present (and not null)
        if let Some(usage_data) = data.get("usage") {
            if !usage_data.is_null() {
                let usage = TokenUsage {
                    prompt_tokens: usage_data["prompt_tokens"].as_u64().unwrap_or(0) as u32,
                    completion_tokens: usage_data["completion_tokens"].as_u64().unwrap_or(0) as u32,
                    total_tokens: (usage_data["prompt_tokens"].as_u64().unwrap_or(0)
                        + usage_data["completion_tokens"].as_u64().unwrap_or(0))
                        as u32,
                    cached_tokens: None,
                };
                return Some(Ok(ParsedEvent::Usage(usage)));
            }
        }

        // Extract model if present
        if let Some(model) = data["model"].as_str() {
            return Some(Ok(ParsedEvent::ResponseStart { model: model.to_string() }));
        }

        // Extract deltas from choices
        if let Some(choices) = data["choices"].as_array() {
            for choice in choices {
                // Handle finish_reason on last chunk
                if let Some(reason) = choice["finish_reason"].as_str() {
                    if reason != "null" && !reason.is_empty() {
                        let finish_reason = match reason {
                            "stop" => FinishReason::Stop,
                            "length" => FinishReason::Length,
                            "tool_calls" => FinishReason::ToolCalls,
                            "content_filter" => FinishReason::ContentFilter,
                            "function_call" => FinishReason::ToolCalls,
                            _ => FinishReason::Other,
                        };
                        return Some(Ok(ParsedEvent::ResponseComplete { finish_reason }));
                    }
                }

                // Handle content delta (skip empty strings)
                if let Some(content) = choice["delta"]["content"].as_str() {
                    if !content.is_empty() {
                        return Some(Ok(ParsedEvent::ContentDelta(content.to_string())));
                    }
                }

                // Handle tool calls delta
                if let Some(tool_calls) = choice["delta"]["tool_calls"].as_array() {
                    for tc in tool_calls {
                        let index = tc["index"].as_u64().unwrap_or(0) as usize;
                        let id = tc["id"].as_str().map(|s| s.to_string());
                        let name = tc["function"]["name"].as_str().map(|s| s.to_string());
                        let args = tc["function"]["arguments"].as_str().map(|s| s.to_string());

                        if id.is_some() || name.is_some() {
                            return Some(Ok(ParsedEvent::ToolCallStart { index, id, name }));
                        }
                        if let Some(arg_delta) = args {
                            return Some(Ok(ParsedEvent::ToolCallDelta {
                                index,
                                delta: arg_delta,
                            }));
                        }
                    }
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_core::FinishReason;

    #[test]
    fn test_parse_done_sentinel() {
        let parser = OpenaiStreamParser;
        let event = parser.parse_line("data: [DONE]").unwrap();
        assert!(matches!(event, Ok(ParsedEvent::ResponseComplete { .. })));
        if let Ok(ParsedEvent::ResponseComplete { finish_reason }) = event {
            assert_eq!(finish_reason, FinishReason::Stop);
        }
    }

    #[test]
    fn test_parse_content_delta() {
        let parser = OpenaiStreamParser;
        let line = r#"data: {"id":"chatcmpl-123","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}"#;
        let event = parser.parse_line(line).unwrap();
        assert!(matches!(event, Ok(ParsedEvent::ContentDelta(_))));
        if let Ok(ParsedEvent::ContentDelta(text)) = event {
            assert_eq!(text, "Hello");
        }
    }

    #[test]
    fn test_parse_tool_call_delta() {
        let parser = OpenaiStreamParser;
        let line = r#"data: {"id":"chatcmpl-123","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_abc","type":"function","function":{"name":"get_weather","arguments":""}}],"content":""},"finish_reason":null}]}"#;
        let event = parser.parse_line(line).unwrap();
        assert!(matches!(event, Ok(ParsedEvent::ToolCallStart { .. })));
    }

    #[test]
    fn test_parse_usage() {
        let parser = OpenaiStreamParser;
        let line = r#"data: {"id":"chatcmpl-123","usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}}"#;
        let event = parser.parse_line(line).unwrap();
        assert!(matches!(event, Ok(ParsedEvent::Usage(_))));
        if let Ok(ParsedEvent::Usage(usage)) = event {
            assert_eq!(usage.prompt_tokens, 10);
            assert_eq!(usage.completion_tokens, 5);
        }
    }

    #[test]
    fn test_parse_finish_reason_stop() {
        let parser = OpenaiStreamParser;
        let line = r#"data: {"id":"chatcmpl-123","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}"#;
        let event = parser.parse_line(line).unwrap();
        assert!(matches!(event, Ok(ParsedEvent::ResponseComplete { finish_reason: _, .. })));
        if let Ok(ParsedEvent::ResponseComplete { finish_reason, .. }) = event {
            assert_eq!(finish_reason, FinishReason::Stop);
        }
    }

    #[test]
    fn test_parse_empty_and_malformed() {
        let parser = OpenaiStreamParser;
        assert!(parser.parse_line("").is_none());
        assert!(parser.parse_line("   ").is_none());
        assert!(parser.parse_line(": ping").is_none());
        assert!(parser.parse_line("data: {bad json}").is_none());
    }

    #[test]
    fn test_parse_model_response_start() {
        let parser = OpenaiStreamParser;
        let line = r#"data: {"id":"chatcmpl-123","model":"gpt-4o","choices":[{"index":0,"delta":{"content":"Hi"},"finish_reason":null}]}"#;
        let event = parser.parse_line(line).unwrap();
        assert!(matches!(event, Ok(ParsedEvent::ResponseStart { .. })));
        if let Ok(ParsedEvent::ResponseStart { model }) = event {
            assert_eq!(model, "gpt-4o");
        }
    }

    #[test]
    fn test_parse_empty_content_skipped() {
        let parser = OpenaiStreamParser;
        let line = r#"data: {"id":"chatcmpl-123","choices":[{"index":0,"delta":{"content":""},"finish_reason":null}]}"#;
        // Empty content should be skipped, returning None since there's nothing else
        let result = parser.parse_line(line);
        assert!(result.is_none(), "Expected None for empty content with no other data");
    }

    #[test]
    fn test_parse_null_usage_skipped() {
        let parser = OpenaiStreamParser;
        let line = r#"data: {"id":"chatcmpl-123","usage":null,"choices":[{"index":0,"delta":{"content":"test"},"finish_reason":null}]}"#;
        let event = parser.parse_line(line).unwrap();
        assert!(matches!(event, Ok(ParsedEvent::ContentDelta(_))));
    }

    #[test]
    fn test_parse_finish_reason_tool_calls() {
        let parser = OpenaiStreamParser;
        let line = r#"data: {"id":"chatcmpl-123","choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}"#;
        let event = parser.parse_line(line).unwrap();
        if let Ok(ParsedEvent::ResponseComplete { finish_reason, .. }) = event {
            assert_eq!(finish_reason, FinishReason::ToolCalls);
        } else {
            panic!("Expected ResponseComplete with ToolCalls");
        }
    }
}
