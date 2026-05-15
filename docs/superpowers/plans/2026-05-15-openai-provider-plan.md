# OpenAI Provider Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an OpenAI-compatible provider that implements the OpenAI chat completions API format, supporting both streaming and non-streaming, loadable via TOML config.

**Architecture:** Follow the same pattern as `AnthropicProvider`. Extract a `StreamProtocol` trait in the shared `StreamingSession` so protocol-specific SSE parsers (Anthropic, OpenAI) can plug in. New `OpenaiProvider` struct implements `LLMClient` with message conversion, body defaults merge, and full parameter mapping.

**Tech Stack:** Rust, serde/serde_json, reqwest, tokio, toml, tracing

---

### Task 1: Extract StreamProtocol trait in StreamingSession

**Files:**
- Modify: `crates/vol-llm-core/src/streaming.rs`
- Test: `crates/vol-llm-core/src/streaming.rs` (inline tests)

The current `StreamingSession` has Anthropic-specific parsing hardcoded. Extract a `StreamProtocol` trait and an internal `ParsedEvent` enum. `StreamingSession` gains an `apply()` method that takes `ParsedEvent` and produces `StreamEvent`s, and a `process_sse()` method that takes a `&impl StreamProtocol`. The existing `process_anthropic_sse()` stays as a convenience wrapper.

- [ ] **Step 1: Add the ParsedEvent enum and StreamProtocol trait**

Add these types at the top of `streaming.rs` (after imports, before `StreamingSession`):

```rust
/// Internal event type that protocol parsers produce
pub enum ParsedEvent {
    ContentDelta(String),
    ContentComplete(String),
    ThinkingDelta(String),
    ThinkingComplete(String),
    ToolCallStart { index: usize, id: Option<String>, name: Option<String> },
    ToolCallDelta { index: usize, delta: String },
    ToolCallComplete(ToolCall),
    Usage(TokenUsage),
    ResponseStart { model: String },
    ResponseComplete { finish_reason: FinishReason },
}

/// Protocol-specific SSE parser
pub trait StreamProtocol: Send {
    fn parse_line(&self, line: &str) -> Option<Result<ParsedEvent, LLMError>>;
}
```

- [ ] **Step 2: Add the apply() method to StreamingSession**

Add this method inside `impl StreamingSession`. It takes a `ParsedEvent` and accumulates state / produces `StreamEvent`s:

```rust
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
                    data: StreamEventData::ContentComplete { content: content.clone() },
                })]
            }
            ParsedEvent::ThinkingDelta(text) => {
                self.thinking_buffer.push_str(text);
                vec![Ok(StreamEvent {
                    id: self.next_id(),
                    data: StreamEventData::ThinkingDelta { thinking: text.clone() },
                })]
            }
            ParsedEvent::ThinkingComplete(text) => {
                vec![Ok(StreamEvent {
                    id: self.next_id(),
                    data: StreamEventData::ThinkingComplete { thinking: text.clone() },
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
                    data: StreamEventData::ToolCallComplete { tool_call: tool_call.clone() },
                })]
            }
            ParsedEvent::Usage(usage) => {
                vec![Ok(StreamEvent {
                    id: self.next_id(),
                    data: StreamEventData::UsageUpdate { usage: usage.clone() },
                })]
            }
            ParsedEvent::ResponseStart { model } => {
                vec![Ok(StreamEvent {
                    id: self.next_id(),
                    data: StreamEventData::ResponseStart { model: model.clone() },
                })]
            }
            ParsedEvent::ResponseComplete { finish_reason } => {
                vec![Ok(StreamEvent {
                    id: self.next_id(),
                    data: StreamEventData::ResponseComplete { finish_reason: *finish_reason },
                })]
            }
        }
    }
```

- [ ] **Step 3: Add the process_sse() method to StreamingSession**

```rust
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
```

- [ ] **Step 4: Create AnthropicProtocol as a StreamProtocol impl**

Add this struct and impl at the bottom of the file (before `#[cfg(test)]`):

```rust
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
            "content_block_stop" => None,
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
```

- [ ] **Step 5: Refactor process_anthropic_sse() to use the protocol**

Replace the existing `process_anthropic_sse()` method body with:

```rust
    /// Process a single SSE line from Anthropic API (backward compat wrapper)
    pub fn process_anthropic_sse(&mut self, line: &str) -> Vec<Result<StreamEvent, LLMError>> {
        self.process_sse(&AnthropicProtocol, line)
    }
```

Also update `handle_message_start`, `handle_content_block_start`, `handle_content_block_delta`, `handle_content_block_stop`, `handle_message_delta`, `handle_message_stop` — remove them entirely since the `AnthropicProtocol` now handles all parsing. Keep `finalize()` as-is (it's state-based, not protocol-specific).

- [ ] **Step 6: Update the streaming tests to work with the new structure**

Replace the existing tests with these:

```rust
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
        // Finalize to get accumulated tool call
        let finalize_events = session.finalize();
        let all_events: Vec<_> = events.into_iter().chain(finalize_events).collect();

        let tool_call_event = all_events.iter().find(|e| {
            matches!(
                e,
                Ok(StreamEvent { data: StreamEventData::ToolCallComplete { .. }, .. })
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
        // Test that the generic process_sse works with any protocol
        let mut session = StreamingSession::new();
        let events = session.apply(&ParsedEvent::ContentDelta("test".to_string()));
        assert_eq!(session.content_buffer, "test");
        assert_eq!(events.len(), 1);
    }
}
```

- [ ] **Step 7: Run tests and commit**

Run: `cargo test -p vol-llm-core -- streaming --nocapture`
Expected: All 6 tests pass

```bash
git add crates/vol-llm-core/src/streaming.rs
git commit -m "refactor(streaming): extract StreamProtocol trait for pluggable SSE parsers

Add ParsedEvent enum, StreamProtocol trait, and AnthropicProtocol impl.
StreamingSession now has generic process_sse() + apply() methods.
process_anthropic_sse() remains as backward-compat wrapper."
```

---

### Task 2: Implement OpenaiStreamParser

**Files:**
- Create: `crates/vol-llm-provider/src/openai_streaming.rs`
- Test: `crates/vol-llm-provider/src/openai_streaming.rs` (inline tests)

Implements `StreamProtocol` for OpenAI's SSE format (`data: {...}` with `choices[0].delta` and `[DONE]` sentinel).

- [ ] **Step 1: Write the failing test for [DONE] sentinel parsing**

```rust
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
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vol-llm-provider openai_streaming 2>&1`
Expected: FAIL with "unresolved module `openai_streaming`"

- [ ] **Step 3: Implement OpenaiStreamParser**

Create `crates/vol-llm-provider/src/openai_streaming.rs`:

```rust
//! OpenAI-compatible SSE stream parser.
//!
//! Parses OpenAI chat completions SSE format: `data: {...}` lines with
//! `[DONE]` sentinel. Each event contains `choices[0].delta` for content
//! and tool call streaming.

use vol_llm_core::{FinishReason, LLMError, ParsedEvent, StreamProtocol, TokenUsage, ToolCall};
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

        let mut events = Vec::new();

        // Extract model from first chunk
        if let Some(model) = data["model"].as_str() {
            events.push(Ok(ParsedEvent::ResponseStart { model: model.to_string() }));
        }

        // Extract usage if present
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
                events.push(Ok(ParsedEvent::Usage(usage)));
            }
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
                        events.push(Ok(ParsedEvent::ResponseComplete { finish_reason }));
                    }
                }

                // Handle content delta
                if let Some(content) = choice["delta"]["content"].as_str() {
                    events.push(Ok(ParsedEvent::ContentDelta(content.to_string())));
                }

                // Handle tool calls delta
                if let Some(tool_calls) = choice["delta"]["tool_calls"].as_array() {
                    for tc in tool_calls {
                        let index = tc["index"].as_u64().unwrap_or(0) as usize;
                        let id = tc["id"].as_str().map(|s| s.to_string());
                        let name = tc["function"]["name"].as_str().map(|s| s.to_string());
                        let args = tc["function"]["arguments"].as_str().map(|s| s.to_string());

                        if id.is_some() || name.is_some() {
                            // First chunk of this tool call — start building
                            events.push(Ok(ParsedEvent::ToolCallStart { index, id, name }));
                        }
                        if let Some(arg_delta) = args {
                            events.push(Ok(ParsedEvent::ToolCallDelta {
                                index,
                                delta: arg_delta,
                            }));
                        }
                    }
                }
            }
        }

        // Return first event (or None if no events produced)
        events.into_iter().next()
    }
}
```

- [ ] **Step 4: Register the module in lib.rs**

Add to `crates/vol-llm-provider/src/lib.rs`:

```rust
pub mod openai;
pub mod openai_streaming;
pub use openai::OpenaiProvider;
pub use openai_streaming::OpenaiStreamParser;
```

- [ ] **Step 5: Add remaining tests for the parser**

Add these tests to `openai_streaming.rs`:

```rust
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
        assert!(matches!(event, Ok(ParsedEvent::ResponseComplete { finish_reason, .. })));
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
}
```

- [ ] **Step 6: Run tests and commit**

Run: `cargo test -p vol-llm-provider openai_streaming 2>&1`
Expected: All 6 tests pass

```bash
git add crates/vol-llm-provider/src/openai_streaming.rs crates/vol-llm-provider/src/lib.rs
git commit -m "feat: add OpenaiStreamParser implementing StreamProtocol

Parses OpenAI SSE format: data: {...} with [DONE] sentinel.
Supports content deltas, tool call deltas, usage, and finish reasons."
```

---

### Task 3: Implement OpenaiProvider

**Files:**
- Create: `crates/vol-llm-provider/src/openai.rs`
- Test: `crates/vol-llm-provider/src/openai.rs` (inline tests)

The main provider struct implementing `LLMClient`.

- [ ] **Step 1: Write failing tests for message conversion**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_core::{LLMProvider, Message, MessageRole, ToolDefinition};

    fn make_provider() -> OpenaiProvider {
        std::env::set_var("TEST_OPENAI_KEY", "test-key");
        let config = LLMConfig::with_env_key(
            LLMProvider::OpenAI,
            "gpt-4o",
            "TEST_OPENAI_KEY",
            "https://api.openai.com",
        );
        OpenaiProvider::new(&config).unwrap()
    }

    #[test]
    fn test_convert_messages_user() {
        let provider = make_provider();
        let messages = vec![Message::user("Hello")];
        let result = provider.convert_messages(&messages);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["role"], "user");
        assert_eq!(result[0]["content"], "Hello");
    }

    #[test]
    fn test_convert_messages_system() {
        let provider = make_provider();
        let messages = vec![Message::system("You are helpful")];
        let result = provider.convert_messages(&messages);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["role"], "system");
        assert_eq!(result[0]["content"], "You are helpful");
    }

    #[test]
    fn test_convert_messages_tool() {
        let provider = make_provider();
        let msg = Message::tool("result", "call_123".to_string());
        let result = provider.convert_messages(&[msg]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["role"], "tool");
        assert_eq!(result[0]["tool_call_id"], "call_123");
        assert_eq!(result[0]["content"], "result");
    }

    #[test]
    fn test_convert_messages_assistant_with_tools() {
        let provider = make_provider();
        let tool_call = ToolCall {
            id: "call_123".to_string(),
            name: "get_weather".to_string(),
            arguments: r#"{"city": "Beijing"}"#.to_string(),
            r#type: "function".to_string(),
        };
        let msg = Message::assistant_with_tools("Checking weather...", vec![tool_call]);
        let result = provider.convert_messages(&[msg]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["role"], "assistant");
        let tools = result[0]["tool_calls"].as_array().unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["id"], "call_123");
        assert_eq!(tools[0]["type"], "function");
        assert_eq!(tools[0]["function"]["name"], "get_weather");
    }

    #[test]
    fn test_convert_tools_basic() {
        let provider = make_provider();
        let tools = vec![ToolDefinition {
            name: "get_weather".to_string(),
            description: "Get weather for a city".to_string(),
            parameters: Some(serde_json::json!({
                "type": "object",
                "properties": {"city": {"type": "string"}},
            })),
        }];
        let result = provider.convert_tools(&tools);
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["type"], "function");
        assert_eq!(arr[0]["function"]["name"], "get_weather");
        assert_eq!(arr[0]["function"]["description"], "Get weather for a city");
    }

    #[test]
    fn test_convert_messages_multiple() {
        let provider = make_provider();
        let messages = vec![
            Message::system("You are helpful"),
            Message::user("Hello"),
            Message::assistant("Hi there"),
        ];
        let result = provider.convert_messages(&messages);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0]["role"], "system");
        assert_eq!(result[1]["role"], "user");
        assert_eq!(result[2]["role"], "assistant");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vol-llm-provider openai 2>&1`
Expected: FAIL — `openai` module doesn't exist yet

- [ ] **Step 3: Implement OpenaiProvider**

Create `crates/vol-llm-provider/src/openai.rs`:

```rust
//! OpenAI-compatible provider implementation.
//!
//! Supports any endpoint following the OpenAI chat completions API format:
//! OpenAI, Azure OpenAI, vLLM, Ollama, LM Studio, etc.

use crate::LLMConfig;
use crate::openai_streaming::OpenaiStreamParser;
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::json;
use std::collections::HashMap;
use tokio::sync::mpsc;
use tracing::info;
use vol_llm_core::*;

/// OpenAI-compatible Provider
pub struct OpenaiProvider {
    client: Client,
    api_key: String,
    model: String,
    base_url: String,
    body_defaults: HashMap<String, serde_json::Value>,
    headers: HashMap<String, String>,
}

impl OpenaiProvider {
    /// Create new OpenAI provider
    pub fn new(config: &LLMConfig) -> Result<Self> {
        let client = Self::build_client()?;
        Ok(Self {
            client,
            api_key: config.resolve_api_key()?,
            model: config.model.clone(),
            base_url: config.base_url.clone(),
            body_defaults: config.body.clone().unwrap_or_default(),
            headers: config.headers.clone().unwrap_or_default(),
        })
    }

    /// Build an HTTP client with optional proxy support.
    fn build_client() -> Result<Client> {
        let proxy_url = std::env::var("HTTPS_PROXY")
            .or_else(|_| std::env::var("https_proxy"))
            .ok();

        let mut builder = Client::builder()
            .danger_accept_invalid_certs(true);

        if let Some(url) = &proxy_url {
            let no_proxy = reqwest::NoProxy::from_string("api.openai.com");
            let proxy = reqwest::Proxy::all(url)
                .map_err(|e| LLMError::Network(reqwest::Error::from(e).into()))?
                .no_proxy(no_proxy);
            builder = builder.proxy(proxy);
        }

        builder
            .build()
            .map_err(|e| LLMError::Network(e.into()))
    }

    /// Convert messages to OpenAI format
    fn convert_messages(&self, messages: &[Message]) -> Vec<serde_json::Value> {
        let mut result = Vec::new();

        for msg in messages {
            match msg.role {
                MessageRole::System => {
                    let content = msg.content.as_ref().map(|c| c.as_str()).unwrap_or("");
                    result.push(json!({
                        "role": "system",
                        "content": content,
                    }));
                }
                MessageRole::User => {
                    let content = msg.content.as_ref().map(|c| c.as_str()).unwrap_or("");
                    result.push(json!({
                        "role": "user",
                        "content": content,
                    }));
                }
                MessageRole::Assistant => {
                    let mut obj = json!({
                        "role": "assistant",
                    });

                    // Text content
                    if let Some(ref c) = msg.content {
                        obj["content"] = json!(c.as_str());
                    } else {
                        obj["content"] = serde_json::Value::Null;
                    }

                    // Tool calls
                    if let Some(ref tools) = msg.tool_calls {
                        obj["tool_calls"] = json!(tools.iter().map(|t| {
                            let input = serde_json::from_str::<serde_json::Value>(&t.arguments)
                                .unwrap_or(json!({}));
                            json!({
                                "id": t.id,
                                "type": "function",
                                "function": {
                                    "name": t.name,
                                    "arguments": input.to_string(),
                                },
                            })
                        }).collect::<Vec<_>>());
                    }

                    result.push(obj);
                }
                MessageRole::Tool => {
                    result.push(json!({
                        "role": "tool",
                        "tool_call_id": msg.tool_call_id.as_deref().unwrap_or(""),
                        "content": msg.content.as_ref()
                            .map(|c| c.as_str())
                            .unwrap_or(""),
                    }));
                }
            }
        }

        result
    }

    /// Convert tools to OpenAI format
    fn convert_tools(&self, tools: &[ToolDefinition]) -> serde_json::Value {
        json!(tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters.as_ref().unwrap_or(&json!({
                            "type": "object",
                            "properties": {}
                        })),
                    },
                })
            })
            .collect::<Vec<_>>())
    }
}

#[async_trait]
impl LLMClient for OpenaiProvider {
    fn provider(&self) -> LLMProvider {
        LLMProvider::OpenAI
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn supported_params(&self) -> &[SupportedParam] {
        &[
            SupportedParam::MaxTokens,
            SupportedParam::Temperature,
            SupportedParam::TopP,
            SupportedParam::TopK,
            SupportedParam::FrequencyPenalty,
            SupportedParam::PresencePenalty,
            SupportedParam::Stop,
            SupportedParam::Seed,
            SupportedParam::LogProbs,
            SupportedParam::Tools,
        ]
    }

    async fn converse(&self, request: ConversationRequest) -> Result<ConversationResponse> {
        // max_tokens default
        let max_tokens = request.model_config.max_tokens
            .or_else(|| self.body_defaults.get("max_tokens").and_then(|v| v.as_u64()).map(|v| v as u32))
            .unwrap_or(8192);

        // Convert messages (system included as first message)
        let openai_messages = self.convert_messages(&request.messages);

        // Build request body
        let mut body = json!({
            "model": self.model,
            "max_tokens": max_tokens,
            "messages": openai_messages,
        });

        // Optional parameters from request
        if let Some(temp) = request.model_config.temperature {
            body["temperature"] = json!(temp);
        }
        if let Some(top_p) = request.model_config.top_p {
            body["top_p"] = json!(top_p);
        }
        if let Some(top_k) = request.model_config.top_k {
            body["top_k"] = json!(top_k);
        }
        if let Some(freq) = request.model_config.frequency_penalty {
            body["frequency_penalty"] = json!(freq);
        }
        if let Some(pres) = request.model_config.presence_penalty {
            body["presence_penalty"] = json!(pres);
        }
        if let Some(ref stop) = request.model_config.stop {
            body["stop"] = json!(stop);
        }
        if let Some(seed) = request.model_config.seed {
            body["seed"] = json!(seed);
        }
        if let Some(logprobs) = request.model_config.logprobs {
            body["logprobs"] = json!(logprobs);
        }
        if let Some(tools) = request.tools {
            body["tools"] = self.convert_tools(&tools);
        }

        // Apply body defaults (skip keys that are always set from request)
        for (key, value) in &self.body_defaults {
            if key == "max_tokens" {
                continue;
            }
            let overridden = match key.as_str() {
                "temperature" => request.model_config.temperature.is_some(),
                "top_p" => request.model_config.top_p.is_some(),
                "top_k" => request.model_config.top_k.is_some(),
                "frequency_penalty" => request.model_config.frequency_penalty.is_some(),
                "presence_penalty" => request.model_config.presence_penalty.is_some(),
                "stop" => request.model_config.stop.is_some(),
                "seed" => request.model_config.seed.is_some(),
                "logprobs" => request.model_config.logprobs.is_some(),
                // Always set from request — skip body defaults
                "model" | "messages" | "tools" | "tool_choice" | "stream"
                | "stream_options" | "max_tokens" => true,
                _ => false,
            };
            if !overridden {
                body[key] = value.clone();
            }
        }

        // Send request
        let url = format!("{}/v1/chat/completions", self.base_url);

        let mut req = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", &self.api_key))
            .header("Content-Type", "application/json")
            .json(&body);

        for (key, value) in &self.headers {
            req = req.header(key, value);
        }

        let response = req
            .send()
            .await
            .map_err(LLMError::Network)?;

        // Handle error response
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error_text = response.text().await.unwrap_or_default();

            if let Ok(error_json) = serde_json::from_str::<serde_json::Value>(&error_text) {
                let message = error_json["error"]["message"]
                    .as_str()
                    .unwrap_or(&error_text)
                    .to_string();
                return Err(LLMError::Api { status, message });
            }

            return Err(LLMError::Api {
                status,
                message: error_text,
            });
        }

        // Parse response
        let result: serde_json::Value = response.json().await?;

        // Extract content
        let content = result["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        // Extract tool calls
        let tool_calls = result["choices"][0]["message"]["tool_calls"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| {
                        Some(ToolCall {
                            id: item["id"].as_str()?.to_string(),
                            name: item["function"]["name"].as_str()?.to_string(),
                            arguments: item["function"]["arguments"].to_string(),
                            r#type: "function".to_string(),
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        // Extract usage
        let usage = TokenUsage {
            prompt_tokens: result["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as u32,
            completion_tokens: result["usage"]["completion_tokens"].as_u64().unwrap_or(0) as u32,
            total_tokens: (result["usage"]["prompt_tokens"].as_u64().unwrap_or(0)
                + result["usage"]["completion_tokens"].as_u64().unwrap_or(0))
                as u32,
            cached_tokens: None,
        };

        // Extract finish reason
        let finish_reason = match result["choices"][0]["finish_reason"].as_str() {
            Some("stop") => FinishReason::Stop,
            Some("length") => FinishReason::Length,
            Some("tool_calls") => FinishReason::ToolCalls,
            Some("content_filter") => FinishReason::ContentFilter,
            Some("function_call") => FinishReason::ToolCalls,
            _ => FinishReason::Other,
        };

        let message = if tool_calls.is_empty() {
            Message::assistant(content)
        } else {
            Message::assistant_with_tools(content, tool_calls)
        };

        info!(
            provider = "openai",
            model = %self.model,
            prompt_tokens = usage.prompt_tokens,
            completion_tokens = usage.completion_tokens,
            "LLM request completed"
        );

        Ok(ConversationResponse {
            message,
            model: result["model"].as_str().unwrap_or(&self.model).to_string(),
            usage,
            finish_reason,
            raw: Some(result),
        })
    }

    async fn converse_stream(&self, request: ConversationRequest) -> Result<StreamReceiver> {
        // max_tokens default
        let max_tokens = request.model_config.max_tokens
            .or_else(|| self.body_defaults.get("max_tokens").and_then(|v| v.as_u64()).map(|v| v as u32))
            .unwrap_or(8192);

        // Convert messages
        let openai_messages = self.convert_messages(&request.messages);

        // Build request body
        let mut body = json!({
            "model": self.model,
            "max_tokens": max_tokens,
            "messages": openai_messages,
            "stream": true,
            "stream_options": {"include_usage": true},
        });

        // Optional parameters (same as converse)
        if let Some(temp) = request.model_config.temperature {
            body["temperature"] = json!(temp);
        }
        if let Some(top_p) = request.model_config.top_p {
            body["top_p"] = json!(top_p);
        }
        if let Some(top_k) = request.model_config.top_k {
            body["top_k"] = json!(top_k);
        }
        if let Some(freq) = request.model_config.frequency_penalty {
            body["frequency_penalty"] = json!(freq);
        }
        if let Some(pres) = request.model_config.presence_penalty {
            body["presence_penalty"] = json!(pres);
        }
        if let Some(ref stop) = request.model_config.stop {
            body["stop"] = json!(stop);
        }
        if let Some(seed) = request.model_config.seed {
            body["seed"] = json!(seed);
        }
        if let Some(logprobs) = request.model_config.logprobs {
            body["logprobs"] = json!(logprobs);
        }
        if let Some(tools) = request.tools {
            body["tools"] = self.convert_tools(&tools);
        }

        // Apply body defaults (same skip list as converse)
        for (key, value) in &self.body_defaults {
            if key == "max_tokens" {
                continue;
            }
            let overridden = match key.as_str() {
                "temperature" => request.model_config.temperature.is_some(),
                "top_p" => request.model_config.top_p.is_some(),
                "top_k" => request.model_config.top_k.is_some(),
                "frequency_penalty" => request.model_config.frequency_penalty.is_some(),
                "presence_penalty" => request.model_config.presence_penalty.is_some(),
                "stop" => request.model_config.stop.is_some(),
                "seed" => request.model_config.seed.is_some(),
                "logprobs" => request.model_config.logprobs.is_some(),
                "model" | "messages" | "tools" | "tool_choice" | "stream"
                | "stream_options" | "max_tokens" => true,
                _ => false,
            };
            if !overridden {
                body[key] = value.clone();
            }
        }

        // Send request
        let url = format!("{}/v1/chat/completions", self.base_url);

        let mut req = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", &self.api_key))
            .header("Content-Type", "application/json")
            .json(&body);

        for (key, value) in &self.headers {
            req = req.header(key, value);
        }

        let response = req
            .send()
            .await
            .map_err(LLMError::Network)?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error_text = response.text().await.unwrap_or_default();

            if let Ok(error_json) = serde_json::from_str::<serde_json::Value>(&error_text) {
                let message = error_json["error"]["message"]
                    .as_str()
                    .unwrap_or(&error_text)
                    .to_string();
                return Err(LLMError::Api { status, message });
            }

            return Err(LLMError::Api {
                status,
                message: error_text,
            });
        }

        // Create channel for streaming events
        let (tx, rx) = mpsc::channel(100);
        let mut session = StreamingSession::new();
        let parser = OpenaiStreamParser;

        tokio::spawn(async move {
            let mut stream = response.bytes_stream();
            let mut buffer = String::new();

            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        let text = match std::str::from_utf8(&chunk) {
                            Ok(s) => s,
                            Err(e) => {
                                let _ = tx.send(Err(LLMError::Parse(e.to_string()))).await;
                                break;
                            }
                        };

                        buffer.push_str(text);

                        // Process complete lines
                        while let Some(newline_pos) = buffer.find('\n') {
                            let line = buffer[..newline_pos].trim().to_string();
                            buffer.drain(..=newline_pos);

                            for event_result in session.process_sse(&parser, &line) {
                                match event_result {
                                    Ok(event) => {
                                        if tx.send(Ok(event)).await.is_err() {
                                            return;
                                        }
                                    }
                                    Err(e) => {
                                        let _ = tx.send(Err(e)).await;
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(LLMError::Network(e))).await;
                        break;
                    }
                }
            }

            // Emit any remaining events
            for event_result in session.finalize() {
                match event_result {
                    Ok(event) => {
                        if tx.send(Ok(event)).await.is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(e)).await;
                    }
                }
            }
        });

        Ok(StreamReceiver::new(rx))
    }
}
```

- [ ] **Step 4: Update factory.rs to dispatch OpenAI**

Replace `crates/vol-llm-provider/src/factory.rs`:

```rust
//! Provider factory functions.

use crate::{AnthropicProvider, LLMConfig, OpenaiProvider};
use vol_llm_core::{LLMClient, LLMError, LLMProvider};

/// Create provider from config
pub fn create_provider(config: &LLMConfig) -> Result<Box<dyn LLMClient>, LLMError> {
    match config.provider {
        LLMProvider::Anthropic => Ok(Box::new(AnthropicProvider::new(config)?)),
        LLMProvider::OpenAI => Ok(Box::new(OpenaiProvider::new(config)?)),
    }
}
```

- [ ] **Step 5: Update lib.rs exports**

Ensure `crates/vol-llm-provider/src/lib.rs` contains:

```rust
//! vol-llm-provider: LLM Provider implementations.

pub mod anthropic;
pub mod config;
pub mod factory;
pub mod loader;
pub mod openai;
pub mod openai_streaming;
pub mod registry;
pub mod secret;

pub use anthropic::AnthropicProvider;
pub use config::{LLMConfig, ProviderFileConfig};
pub use factory::create_provider;
pub use loader::{NamedProviderConfig, ProviderLoader};
pub use openai::OpenaiProvider;
pub use openai_streaming::OpenaiStreamParser;
pub use registry::{LLMProviderConfig, LLMProviderRegistry};
pub use secret::Secret;
```

- [ ] **Step 6: Run tests and commit**

Run: `cargo test -p vol-llm-provider openai 2>&1`
Expected: All message/tool conversion tests pass

Run: `cargo test -p vol-llm-provider --lib 2>&1`
Expected: All tests pass, no new warnings

```bash
git add crates/vol-llm-provider/src/openai.rs crates/vol-llm-provider/src/factory.rs crates/vol-llm-provider/src/lib.rs
git commit -m "feat: add OpenaiProvider implementing LLMClient

Implements OpenAI chat completions API with message/tool conversion,
full parameter mapping, body defaults merge, SSE streaming, and
custom headers. Factory dispatch added for LLMProvider::OpenAI."
```

---

### Task 4: Add body defaults merge test for OpenaiProvider

**Files:**
- Modify: `crates/vol-llm-provider/src/openai.rs` (add test at bottom)

- [ ] **Step 1: Add the body defaults merge test**

Append this test to the `#[cfg(test)]` mod in `openai.rs`:

```rust
    #[test]
    fn test_body_defaults_merge_in_converse_body() {
        use vol_llm_core::LLMProvider;

        std::env::set_var("TEST_OPENAI_MERGE", "test-key");
        let mut body = HashMap::new();
        body.insert("temperature".to_string(), serde_json::json!(0.9));
        body.insert("custom_param".to_string(), serde_json::json!("custom_value"));

        let config = LLMConfig::with_env_key(
            LLMProvider::OpenAI,
            "gpt-4o",
            "TEST_OPENAI_MERGE",
            "https://api.openai.com",
        ).with_body(body);

        let provider = OpenaiProvider::new(&config).unwrap();

        // Verify body_defaults are stored
        assert_eq!(provider.body_defaults.get("temperature").unwrap(), &serde_json::json!(0.9));
        assert_eq!(provider.body_defaults.get("custom_param").unwrap(), &serde_json::json!("custom_value"));
    }
```

- [ ] **Step 2: Run test and commit**

Run: `cargo test -p vol-llm-provider test_body_defaults_merge_in_converse_body 2>&1`
Expected: PASS

```bash
git add crates/vol-llm-provider/src/openai.rs
git commit -m "test: add body defaults merge test for OpenaiProvider"
```

---

### Task 5: Add OpenAI provider TOML example config

**Files:**
- Create: `.agents/providers/openai-example.toml`

- [ ] **Step 1: Create example provider config**

```toml
provider = "openai"
model = "gpt-4o"
api_key = "${OPENAI_API_KEY}"
base_url = "https://api.openai.com"

[body]
max_tokens = 2048
temperature = 0.7

[headers]
# Custom headers (optional)
# X-Custom-Header = "value"
```

- [ ] **Step 2: Commit**

```bash
git add .agents/providers/openai-example.toml
git commit -m "docs: add example OpenAI provider TOML config"
```

---

### Task 6: Run full test suite and verify

**Files:**
- No new file changes

- [ ] **Step 1: Run all provider tests**

Run: `cargo test -p vol-llm-provider 2>&1`
Expected: All tests pass (existing + new OpenAI tests)

- [ ] **Step 2: Run vol-llm-core tests**

Run: `cargo test -p vol-llm-core 2>&1`
Expected: All tests pass (streaming refactor tests)

- [ ] **Step 3: Verify clean build**

Run: `cargo build -p vol-llm-provider 2>&1`
Expected: No errors, no new warnings

- [ ] **Step 4: Verify the example compiles**

Run: `cargo check --example provider_toml_chat -p vol-llm-provider 2>&1`
Expected: Compiles cleanly

- [ ] **Step 5: Commit any remaining changes**

```bash
git status
# If everything is committed:
echo "All tests pass, build clean, example compiles"
```
