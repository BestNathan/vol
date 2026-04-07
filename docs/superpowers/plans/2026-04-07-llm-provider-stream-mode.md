# LLM Provider Stream Mode Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement streaming response mode for Anthropic provider with common streaming infrastructure in vol-llm-core.

**Architecture:** 
1. Extend `vol-llm-core/src/stream.rs` with unified `StreamEventData` enum (merges event type and data)
2. Create `vol-llm-core/src/streaming.rs` with `StreamingSession` and `ToolCallBuilder` for common SSE parsing/accumulation
3. Implement `AnthropicProvider::converse_stream()` using the new streaming infrastructure
4. Add unit tests for streaming logic and integration tests with mock SSE responses

**Tech Stack:** Rust, tokio (async runtime), reqwest (HTTP client), serde_json (JSON parsing), tokio::sync::mpsc (channels)

---

## File Structure

**Files to Create:**
- `crates/vol-llm-core/src/streaming.rs` - Common streaming logic (StreamingSession, ToolCallBuilder)
- `crates/vol-llm-provider/tests/stream_integration.rs` - Integration tests with mock SSE

**Files to Modify:**
- `crates/vol-llm-core/src/stream.rs` - Refactor to unified StreamEventData enum, add aggregate events
- `crates/vol-llm-core/src/lib.rs` - Export streaming module
- `crates/vol-llm-provider/src/anthropic.rs` - Implement converse_stream method

---

### Task 1: Refactor vol-llm-core/src/stream.rs to Unified Enum

**Files:**
- Modify: `crates/vol-llm-core/src/stream.rs`

- [ ] **Step 1: Update stream.rs with unified StreamEventData enum**

Replace the current separated `StreamEventType` and `StreamEventData` with a single unified enum that combines event type and data together.

```rust
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
```

- [ ] **Step 2: Run tests to verify refactoring compiles**

```bash
cd crates/vol-llm-core && cargo test stream -- --nocapture
```

Expected: Tests pass, existing code may have compilation errors (to be fixed in subsequent tasks)

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-core/src/stream.rs
git commit -m "refactor: unify StreamEventData enum with tag attribute"
```

---

### Task 2: Create vol-llm-core/src/streaming.rs Module

**Files:**
- Create: `crates/vol-llm-core/src/streaming.rs`
- Modify: `crates/vol-llm-core/src/lib.rs`

- [ ] **Step 1: Create streaming.rs with StreamingSession and ToolCallBuilder**

```rust
//! Common streaming logic for LLM providers.
//!
//! Provides shared infrastructure for parsing SSE responses and accumulating
//! streaming chunks into complete events.

use serde_json::Value;
use crate::{StreamEvent, StreamEventData, TokenUsage, ToolCall, FinishReason, LLMError};

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
    /// Returns Some(StreamEvent) if the line produces an event, None otherwise
    pub fn process_anthropic_sse(&mut self, line: &str) -> Option<Result<StreamEvent, LLMError>> {
        // Skip empty lines and non-data lines
        let line = line.trim();
        if line.is_empty() || !line.starts_with("data:") {
            return None;
        }

        // Extract JSON from "data: {...}"
        let json_str = line.strip_prefix("data:")?.trim();
        let data: Value = serde_json::from_str(json_str).ok()?;
        let event_type = data["type"].as_str()?;

        match event_type {
            "message_start" => Some(self.handle_message_start(&data)),
            "content_block_start" => self.handle_content_block_start(&data),
            "content_block_delta" => self.handle_content_block_delta(&data),
            "content_block_stop" => self.handle_content_block_stop(&data),
            "message_delta" => Some(self.handle_message_delta(&data)),
            "message_stop" => Some(self.handle_message_stop(&data)),
            _ => None,
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

    fn handle_content_block_start(&mut self, data: &Value) -> Option<Result<StreamEvent, LLMError>> {
        let block_type = data["content_block"]["type"].as_str()?;
        
        match block_type {
            "thinking" => {
                // Initialize thinking accumulator
                // The actual thinking content comes in delta events
                None
            }
            "tool_use" => {
                // Start building a new tool call
                let id = data["content_block"]["id"].as_str().map(|s| s.to_string());
                let name = data["content_block"]["name"].as_str().map(|s| s.to_string());
                
                self.current_tool_call = Some(ToolCallBuilder {
                    index: self.tool_call_index,
                    id,
                    name,
                    arguments: String::new(),
                });
                self.tool_call_index += 1;
                None
            }
            "text" | _ => {
                // Text blocks don't emit events on start
                None
            }
        }
    }

    fn handle_content_block_delta(&mut self, data: &Value) -> Option<Result<StreamEvent, LLMError>> {
        // Check for thinking delta
        if let Some(thinking) = data["delta"]["thinking"].as_str() {
            self.thinking_buffer.push_str(thinking);
            return Some(Ok(StreamEvent {
                id: self.next_id(),
                data: StreamEventData::ThinkingDelta { thinking: thinking.to_string() },
            }));
        }

        // Check for text delta
        if let Some(text) = data["delta"]["text"].as_str() {
            self.content_buffer.push_str(text);
            return Some(Ok(StreamEvent {
                id: self.next_id(),
                data: StreamEventData::ContentDelta { delta: text.to_string() },
            }));
        }

        // Check for tool_use input delta (partial JSON)
        if let Some(input) = data["delta"]["partial_json"].as_str() {
            if let Some(ref mut builder) = self.current_tool_call {
                builder.arguments.push_str(input);
            }
            // Don't emit tool call delta events (per design: accumulate then emit complete)
            return None;
        }

        None
    }

    fn handle_content_block_stop(&mut self, _data: &Value) -> Option<Result<StreamEvent, LLMError>> {
        // Emit aggregate events based on what just finished
        
        let mut events = Vec::new();

        // Check if thinking just completed
        if !self.thinking_buffer.is_empty() {
            let thinking = std::mem::take(&mut self.thinking_buffer);
            events.push(Ok(StreamEvent {
                id: self.next_id(),
                data: StreamEventData::ThinkingComplete { thinking },
            }));
        }

        // Check if content just completed (text block ended)
        // Note: We emit ContentComplete at the end of the message, not each text block
        // For now, we'll emit when thinking completes as a proxy

        // Check if tool call just completed
        if let Some(builder) = self.current_tool_call.take() {
            if let Some(tool_call) = builder.build() {
                events.push(Ok(StreamEvent {
                    id: self.next_id(),
                    data: StreamEventData::ToolCallComplete { tool_call },
                }));
            }
        }

        // Return the most important event (tool call takes priority)
        events.pop()
    }

    fn handle_message_delta(&mut self, data: &Value) -> Result<StreamEvent, LLMError> {
        // Extract usage if present
        let usage = if let Some(usage_data) = data.get("usage") {
            TokenUsage {
                prompt_tokens: usage_data["input_tokens"].as_u64().unwrap_or(0) as u32,
                completion_tokens: usage_data["output_tokens"].as_u64().unwrap_or(0) as u32,
                total_tokens: (
                    usage_data["input_tokens"].as_u64().unwrap_or(0) +
                    usage_data["output_tokens"].as_u64().unwrap_or(0)
                ) as u32,
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

    fn handle_message_stop(&mut self, data: &Value) -> Result<StreamEvent, LLMError> {
        // Emit ContentComplete if we have accumulated content
        let mut events = Vec::new();

        if !self.content_buffer.is_empty() {
            let content = std::mem::take(&mut self.content_buffer);
            events.push(StreamEvent {
                id: self.next_id(),
                data: StreamEventData::ContentComplete { content },
            });
        }

        // Extract finish reason
        let finish_reason = match data["stop_reason"].as_str() {
            Some("end_turn") | Some("stop_sequence") => FinishReason::Stop,
            Some("max_tokens") => FinishReason::Length,
            Some("tool_use") => FinishReason::ToolCalls,
            _ => FinishReason::Other,
        };

        events.push(StreamEvent {
            id: self.next_id(),
            data: StreamEventData::ResponseComplete { finish_reason },
        });

        // Return the last event (ResponseComplete)
        events.pop().map(Ok).unwrap_or_else(|| {
            Ok(StreamEvent {
                id: self.next_id(),
                data: StreamEventData::ResponseComplete { finish_reason },
            })
        })
    }

    /// Finalize the session and emit any remaining aggregate events
    pub fn finalize(&mut self) -> Vec<Result<StreamEvent, LLMError>> {
        let mut events = Vec::new();

        // Emit remaining content
        if !self.content_buffer.is_empty() {
            let content = std::mem::take(&mut self.content_buffer);
            events.push(Ok(StreamEvent {
                id: self.next_id(),
                data: StreamEventData::ContentComplete { content },
            }));
        }

        // Emit remaining thinking
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
        
        // Simulate content deltas
        let delta1 = r#"data: {"type": "content_block_delta", "delta": {"text": "Hello"}}"#;
        let delta2 = r#"data: {"type": "content_block_delta", "delta": {"text": " world"}}"#;
        
        let event1 = session.process_anthropic_sse(delta1);
        let event2 = session.process_anthropic_sse(delta2);
        
        assert!(event1.is_some());
        assert!(event2.is_some());
        
        // Verify content was accumulated
        assert_eq!(session.content_buffer, "Hello world");
    }

    #[test]
    fn test_tool_call_builder_accumulates_arguments() {
        let mut session = StreamingSession::new();
        
        // Start tool block
        let start = r#"data: {"type": "content_block_start", "content_block": {"type": "tool_use", "id": "tool_1", "name": "get_weather"}}"#;
        session.process_anthropic_sse(start);
        
        // Partial JSON deltas
        let delta1 = r#"data: {"type": "content_block_delta", "delta": {"partial_json": "{\"city\": \"Beijing"}}"#;
        let delta2 = r#"data: {"type": "content_block_delta", "delta": {"partial_json": "\"}"}}"#;
        session.process_anthropic_sse(delta1);
        session.process_anthropic_sse(delta2);
        
        // Stop block (should emit ToolCallComplete)
        let stop = r#"data: {"type": "content_block_stop"}"#;
        let event = session.process_anthropic_sse(stop);
        
        assert!(event.is_some());
        if let Some(Ok(StreamEvent { data: StreamEventData::ToolCallComplete { tool_call }, .. })) = event {
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
        
        // Test message_start
        let start = r#"data: {"type": "message_start", "message": {"model": "qwen3.5-plus"}}"#;
        let event = session.process_anthropic_sse(start);
        
        assert!(event.is_some());
        if let Some(Ok(StreamEvent { data: StreamEventData::ResponseStart { model }, .. })) = event {
            assert_eq!(model, "qwen3.5-plus");
        } else {
            panic!("Expected ResponseStart event");
        }
    }

    #[test]
    fn test_empty_and_malformed_lines() {
        let mut session = StreamingSession::new();
        
        // Empty line
        assert!(session.process_anthropic_sse("").is_none());
        assert!(session.process_anthropic_sse("   ").is_none());
        
        // Non-data line
        assert!(session.process_anthropic_sse(": ping").is_none());
        
        // Malformed JSON
        assert!(session.process_anthropic_sse("data: {invalid json}").is_none());
    }
}
```

- [ ] **Step 2: Update lib.rs to export streaming module**

Add the new streaming module to `crates/vol-llm-core/src/lib.rs`:

```rust
//! vol-llm-core: Core protocol types for LLM interaction.

pub mod provider;
pub mod message;
pub mod tool;
pub mod model;
pub mod conversation;
pub mod stream;
pub mod streaming;  // NEW
pub mod client;
pub mod error;

pub use provider::*;
pub use message::*;
pub use tool::*;
pub use model::*;
pub use conversation::*;
pub use stream::*;
pub use streaming::*;  // NEW
pub use client::*;
pub use error::*;
```

- [ ] **Step 3: Run tests to verify streaming module compiles**

```bash
cd crates/vol-llm-core && cargo test streaming -- --nocapture
```

Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-core/src/streaming.rs crates/vol-llm-core/src/lib.rs
git commit -m "feat: add common streaming module with StreamingSession"
```

---

### Task 3: Implement AnthropicProvider::converse_stream

**Files:**
- Modify: `crates/vol-llm-provider/src/anthropic.rs`

- [ ] **Step 1: Replace converse_stream placeholder with full implementation**

In `crates/vol-llm-provider/src/anthropic.rs`, replace the existing placeholder:

```rust
async fn converse_stream(&self, _request: ConversationRequest) -> Result<StreamReceiver> {
    Err(LLMError::Parse("Streaming not implemented".to_string()))
}
```

With this implementation:

```rust
async fn converse_stream(&self, request: ConversationRequest) -> Result<StreamReceiver> {
    use tokio::sync::mpsc;
    use futures_util::StreamExt;
    
    // max_tokens is required for Anthropic
    let max_tokens = request.model_config.max_tokens.unwrap_or(1024);

    // Convert messages
    let anthropic_messages = self.convert_messages(&request.messages)?;

    // Build request body
    let mut body = json!({
        "model": self.model,
        "max_tokens": max_tokens,
        "messages": anthropic_messages,
        "stream": true,  // Enable streaming
    });

    // System message separately
    if let Some(system) = request.system {
        body["system"] = json!(system);
    }

    // Optional parameters
    if let Some(temp) = request.model_config.temperature {
        body["temperature"] = json!(temp);
    }
    if let Some(top_p) = request.model_config.top_p {
        body["top_p"] = json!(top_p);
    }
    if let Some(tools) = request.tools {
        body["tools"] = self.convert_tools(&tools);
    }

    // Send request
    let url = format!("{}/v1/messages", self.base_url);

    let response = self.client
        .post(&url)
        .header("x-api-key", &self.api_key)
        .header("anthropic-version", "2023-06-01")
        .header("Content-Type", "application/json")
        .header("User-Agent", "claude-code/1.0.0")
        .json(&body)
        .send()
        .await
        .map_err(LLMError::Network)?;

    // Handle non-success status
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

        return Err(LLMError::Api { status, message: error_text });
    }

    // Create channel for streaming events
    let (tx, rx) = mpsc::channel(100);

    // Spawn async task to process SSE stream
    let mut session = StreamingSession::new();
    
    tokio::spawn(async move {
        let mut stream = response.bytes_stream();
        
        let mut buffer = String::new();
        
        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(chunk) => {
                    // Decode chunk to string
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
                        let line = buffer[..newline_pos].trim();
                        buffer.drain(..=newline_pos);
                        
                        // Process SSE line
                        if let Some(event_result) = session.process_anthropic_sse(line) {
                            match event_result {
                                Ok(event) => {
                                    if tx.send(Ok(event)).await.is_err() {
                                        // Receiver dropped, stop processing
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
        
        // Emit any remaining events (finalization)
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
```

- [ ] **Step 2: Add futures-util dependency to vol-llm-provider**

In `crates/vol-llm-provider/Cargo.toml`, add:

```toml
[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true }
async-trait = { workspace = true }
reqwest = { workspace = true, features = ["stream"] }  # Ensure stream feature is enabled
tracing = { workspace = true }
toml = { workspace = true }
vol-llm-core = { path = "../vol-llm-core" }
futures-util = "0.3"  # NEW: for StreamExt trait
```

- [ ] **Step 3: Run cargo check to verify implementation compiles**

```bash
cd crates/vol-llm-provider && cargo check
```

Expected: Compiles without errors

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-provider/src/anthropic.rs crates/vol-llm-provider/Cargo.toml
git commit -m "feat: implement AnthropicProvider::converse_stream"
```

---

### Task 4: Add Integration Tests for Stream Mode

**Files:**
- Create: `crates/vol-llm-provider/tests/stream_integration.rs`

- [ ] **Step 1: Create integration test file with mock HTTP server**

```rust
//! Integration tests for streaming with mock HTTP server.

use vol_llm_provider::{AnthropicProvider, LLMConfig};
use vol_llm_core::{ConversationRequest, StreamEventData, Message};
use tokio::net::TcpListener;
use tokio::io::AsyncWriteExt;

/// Simple mock HTTP server that returns SSE stream
async fn spawn_mock_sse_server(response: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let addr = format!("http://127.0.0.1:{}", port);

    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        
        // Read request (and ignore it)
        let mut buffer = [0u8; 1024];
        let _ = socket.readable().await;
        let _ = socket.try_read(&mut buffer);
        
        // Send response
        let response_body = format!(
            "HTTP/1.1 200 OK\r\n\
             Content-Type: text/event-stream\r\n\
             Cache-Control: no-cache\r\n\
             Connection: keep-alive\r\n\
             \r\n\
             {}",
            response
        );
        
        let _ = socket.writable().await;
        let _ = socket.try_write(response_body.as_bytes());
        
        // Give time for client to read
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    });

    addr
}

#[tokio::test]
async fn test_anthropic_stream_basic() {
    // Mock SSE response with simple text content
    const MOCK_RESPONSE: &str = r#"data: {"type": "message_start", "message": {"id": "msg_1", "model": "qwen3.5-plus"}}
data: {"type": "content_block_start", "content_block": {"type": "text"}}
data: {"type": "content_block_delta", "delta": {"text": "Hello"}}
data: {"type": "content_block_delta", "delta": {"text": " world"}}
data: {"type": "content_block_stop"}
data: {"type": "message_delta", "usage": {"input_tokens": 10, "output_tokens": 5}}
data: {"type": "message_stop", "stop_reason": "end_turn"}
"#;

    let base_url = spawn_mock_sse_server(MOCK_RESPONSE).await;

    let config = LLMConfig {
        provider: vol_llm_core::LLMProvider::Anthropic,
        model: "qwen3.5-plus".to_string(),
        base_url,
        api_key: Some("test-key".to_string()),
    };

    let provider = AnthropicProvider::new(&config).unwrap();

    let request = ConversationRequest::simple("Test");
    let mut receiver = provider.converse_stream(request).await.unwrap();

    let mut events = Vec::new();
    while let Some(result) = receiver.recv().await {
        match result {
            Ok(event) => events.push(event),
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    // Verify we got the expected events
    assert!(events.iter().any(|e| matches!(e.data, StreamEventData::ResponseStart { .. })));
    assert!(events.iter().any(|e| matches!(e.data, StreamEventData::ContentDelta { .. })));
    assert!(events.iter().any(|e| matches!(e.data, StreamEventData::ContentComplete { .. })));
    assert!(events.iter().any(|e| matches!(e.data, StreamEventData::ResponseComplete { .. })));
}

#[tokio::test]
async fn test_anthropic_stream_with_tool_call() {
    // Mock SSE response with tool call
    const MOCK_RESPONSE: &str = r#"data: {"type": "message_start", "message": {"id": "msg_1", "model": "qwen3.5-plus"}}
data: {"type": "content_block_start", "content_block": {"type": "tool_use", "id": "tool_1", "name": "get_weather"}}
data: {"type": "content_block_delta", "delta": {"partial_json": "{\"city\": \"Beijing"}}"
data: {"type": "content_block_delta", "delta": {"partial_json": "\"}"}}
data: {"type": "content_block_stop"}
data: {"type": "message_delta", "usage": {"input_tokens": 10, "output_tokens": 5}}
data: {"type": "message_stop", "stop_reason": "tool_use"}
"#;

    let base_url = spawn_mock_sse_server(MOCK_RESPONSE).await;

    let config = LLMConfig {
        provider: vol_llm_core::LLMProvider::Anthropic,
        model: "qwen3.5-plus".to_string(),
        base_url,
        api_key: Some("test-key".to_string()),
    };

    let provider = AnthropicProvider::new(&config).unwrap();

    let request = ConversationRequest::simple("What's the weather in Beijing?");
    let mut receiver = provider.converse_stream(request).await.unwrap();

    let mut events = Vec::new();
    while let Some(result) = receiver.recv().await {
        match result {
            Ok(event) => events.push(event),
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    // Verify we got ToolCallComplete event
    let tool_call_event = events.iter().find(|e| matches!(e.data, StreamEventData::ToolCallComplete { .. }));
    assert!(tool_call_event.is_some(), "Expected ToolCallComplete event");

    if let Some(StreamEventData::ToolCallComplete { tool_call }) = tool_call_event.map(|e| &e.data) {
        assert_eq!(tool_call.id, "tool_1");
        assert_eq!(tool_call.name, "get_weather");
        assert_eq!(tool_call.arguments, r#"{"city": "Beijing"}"#);
    }
}

#[tokio::test]
async fn test_anthropic_stream_error_handling() {
    // Test that network errors are properly propagated
    let config = LLMConfig {
        provider: vol_llm_core::LLMProvider::Anthropic,
        model: "qwen3.5-plus".to_string(),
        base_url: "http://127.0.0.1:1".to_string(),  // Invalid port
        api_key: Some("test-key".to_string()),
    };

    let provider = AnthropicProvider::new(&config).unwrap();
    let request = ConversationRequest::simple("Test");
    
    // Should return error (connection refused)
    let result = provider.converse_stream(request).await;
    assert!(result.is_err());
}
```

- [ ] **Step 2: Run integration tests**

```bash
cd crates/vol-llm-provider && cargo test stream_integration -- --nocapture
```

Expected: All integration tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-provider/tests/stream_integration.rs
git commit -m "test: add integration tests for streaming"
```

---

### Task 5: Update vol-llm-agent to Support Streaming (Optional)

**Files:**
- Modify: `crates/vol-llm-agent/src/agent.rs`

**Note:** This task is optional and can be deferred. The streaming infrastructure is complete without it. This task demonstrates using streaming in the Agent ReAct loop.

- [ ] **Step 1: Add streaming mode to AgentConfig**

Add a `streaming: bool` field to `AgentConfig` to control whether the agent uses streaming mode.

- [ ] **Step 2: Implement streaming run method**

Create a new `run_stream` method or modify `run` to optionally use streaming.

- [ ] **Step 3: Commit**

---

## Self-Review Checklist

**1. Spec Coverage:**
- [x] Unified StreamEventData enum - Task 1
- [x] StreamingSession and ToolCallBuilder - Task 2
- [x] AnthropicProvider::converse_stream implementation - Task 3
- [x] Integration tests with mock SSE - Task 4
- [ ] Agent streaming support (optional) - Task 5

**2. Placeholder Scan:**
- No TBD/TODO in steps
- All code steps contain actual code
- All test steps contain actual test code

**3. Type Consistency:**
- `StreamEventData` variants match across all tasks
- `StreamingSession` methods match usage in Task 3
- `StreamReceiver` API consistent

---

Plan complete and saved to `docs/superpowers/plans/2026-04-07-llm-provider-stream-mode.md`. Two execution options:

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

**Which approach?**
