# LLM Provider Stream Mode Design

**日期**: 2026-04-07  
**作者**: Claude (with user collaboration)  
**状态**: Approved

## Overview

为 `vol-llm-provider` 实现流式响应模式，支持 Agent ReAct 循环中的实时输出和简单对话流式输出。

## Goals

1. 实现 `AnthropicProvider::converse_stream()` 方法
2. 支持实时流式输出（Content/Thinking/ToolCall deltas）
3. 支持聚合事件（ContentComplete/ThinkingComplete）
4. 提取公共流式处理逻辑，便于扩展到 OpenAI 等其他 Provider

## Non-Goals

- 不实现 HTTP SSE endpoint（保持内部 channel 模式）
- 不修改 Agent 层的调用逻辑（向后兼容）

---

## Architecture

### Module Structure

```
vol-llm-core/
├── src/
│   ├── client.rs       # LLMClient trait (已有)
│   ├── stream.rs       # StreamEvent 类型 (扩展)
│   ├── streaming.rs    # NEW: 流式处理公共逻辑
│   └── lib.rs          # 导出新模块

vol-llm-provider/
├── src/
│   ├── anthropic.rs    # 实现 converse_stream
│   └── ...
```

### Core Components

**1. `vol-llm-core/src/streaming.rs` (新增)**

| Component | Description |
|-----------|-------------|
| `StreamingSession` | 管理流式会话状态（累积 content/thinking/tool_calls） |
| `ToolCallBuilder` | 累积 tool call 的 id/name/arguments |
| `parse_sse_line()` | 解析 SSE 原始行到中间事件 |
| `emit_event()` | 发送 `StreamEvent` 到 channel |

**2. `vol-llm-core/src/stream.rs` (扩展)**

合并 `StreamEventType` 和 `StreamEventData` 为单一枚举。

---

## Data Model

### StreamEvent

```rust
pub struct StreamEvent {
    pub id: String,
    pub data: StreamEventData,
}
```

### StreamEventData

```rust
pub enum StreamEventData {
    // Lifecycle
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
```

### StreamingSession

```rust
struct StreamingSession {
    // Accumulators
    content_buffer: String,
    thinking_buffer: String,
    current_tool_call: Option<ToolCallBuilder>,
    
    // Counters
    tool_call_index: usize,
}

struct ToolCallBuilder {
    index: usize,
    id: Option<String>,
    name: Option<String>,
    arguments: String,  // Incrementally built JSON
}
```

---

## Data Flow

### Anthropic SSE Event Mapping

| Anthropic Event | Converted Event | Notes |
|----------------|-----------------|-------|
| `message_start` | `ResponseStart` | Extract model |
| `content_block_start` (text) | - | Initialize accumulator |
| `content_block_delta` (text) | `ContentDelta` | Emit incrementally |
| `content_block_stop` (text) | `ContentComplete` | Emit aggregated content |
| `content_block_start` (thinking) | - | Initialize accumulator |
| `content_block_delta` (thinking) | `ThinkingDelta` | Emit incrementally |
| `content_block_stop` (thinking) | `ThinkingComplete` | Emit aggregated thinking |
| `content_block_start` (tool_use) | - | Create `ToolCallBuilder` |
| `content_block_delta` (tool_use) | - | Accumulate into builder |
| `content_block_stop` (tool_use) | `ToolCallComplete` | Emit complete `ToolCall` |
| `message_delta` | `UsageUpdate` | Extract usage stats |
| `message_stop` | `ResponseComplete` | Extract finish reason |

### Example Event Stream

```
1. ResponseStart { model: "qwen3.5-plus" }
2. ContentDelta { delta: "Hello" }
3. ContentDelta { delta: " world" }
4. ContentComplete { content: "Hello world" }
5. ThinkingDelta { thinking: "Let me" }
6. ThinkingDelta { thinking: " think about" }
7. ThinkingDelta { thinking: " this" }
8. ThinkingComplete { thinking: "Let me think about this" }
9. ToolCallComplete { tool_call: ToolCall { id: "tool_1", name: "query_iv", ... } }
10. UsageUpdate { usage: TokenUsage { prompt_tokens: 100, completion_tokens: 50, ... } }
11. ResponseComplete { finish_reason: Stop }
```

---

## Error Handling

### Two Levels of Errors

| Level | Type | Description |
|-------|------|-------------|
| Channel/Transport | `Result<StreamEvent, LLMError>` | Network failures, channel closed |
| Application | `StreamEventData::Error` | API errors within stream (rate limit, etc.) |

### Error Scenarios

| Scenario | Return Type | Handling |
|----------|-------------|----------|
| Connection failure | `Err(LLMError::Network)` | Return before stream starts |
| Authentication failure | `Err(LLMError::Api)` | Return before stream starts |
| Rate limit during stream | `Ok(StreamEvent { Error { ... } })` | Emit error event, may have partial response |
| SSE parse error | `Ok(StreamEvent { Error { ... } })` or `Err(LLMError::Parse)` | Depends on recoverability |
| Receiver dropped | `Err(LLMError::Channel)` | Stop sending |

### Usage Pattern

```rust
let mut receiver = llm.converse_stream(request).await?;

while let Some(result) = receiver.recv().await {
    match result {
        Ok(StreamEvent { data: StreamEventData::Error { code, message }, .. }) => {
            tracing::error!("Stream error: {} - {}", code, message);
        }
        Ok(StreamEvent { data: StreamEventData::ContentComplete { content }, .. }) => {
            println!("Complete response: {}", content);
        }
        Err(e) => {
            tracing::error!("Channel error: {}", e);
            break;
        }
        _ => {}
    }
}
```

---

## Implementation Details

### AnthropicProvider::converse_stream

```rust
async fn converse_stream(&self, request: ConversationRequest) -> Result<StreamReceiver> {
    // 1. Build HTTP request with stream: true
    let url = format!("{}/v1/messages", self.base_url);
    let body = self.build_request_body(&request);
    
    // 2. Create channel
    let (tx, rx) = tokio::sync::mpsc::channel(100);
    
    // 3. Spawn async task to handle SSE stream
    tokio::spawn(async move {
        let response = client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await?;
        
        // 4. Read response line by line
        let mut session = StreamingSession::new();
        let mut lines = response.lines();
        
        while let Some(line) = lines.next_line().await? {
            if let Some(event) = session.process_sse_line(&line) {
                if tx.send(Ok(event)).await.is_err() {
                    break; // Receiver dropped
                }
            }
        }
    });
    
    Ok(StreamReceiver::new(rx))
}
```

### StreamingSession::process_sse_line

```rust
impl StreamingSession {
    pub fn process_sse_line(&mut self, line: &str) -> Option<StreamEvent> {
        // Parse SSE format: "data: {...}"
        let data = self.extract_json_from_sse(line)?;
        
        let event_type = data["type"].as_str()?;
        
        match event_type {
            "message_start" => Some(self.handle_message_start(data)),
            "content_block_delta" => self.handle_content_block_delta(data),
            "content_block_stop" => self.handle_content_block_stop(data),
            "message_delta" => Some(self.handle_message_delta(data)),
            "message_stop" => Some(self.handle_message_stop(data)),
            _ => None,
        }
    }
    
    fn handle_content_block_delta(&mut self, data: &serde_json::Value) -> Option<StreamEvent> {
        // Extract delta and emit ContentDelta or ThinkingDelta
    }
    
    fn handle_content_block_stop(&mut self, data: &serde_json::Value) -> Option<StreamEvent> {
        // Emit ContentComplete, ThinkingComplete, or ToolCallComplete
    }
}
```

---

## Testing Strategy

### Unit Tests (vol-llm-core/src/streaming.rs)

```rust
#[test]
fn test_streaming_session_accumulates_content() {
    // Verify content_buffer accumulates across multiple deltas
}

#[test]
fn test_tool_call_builder_accumulates_arguments() {
    // Verify JSON arguments are correctly built from chunks
}

#[test]
fn test_parse_sse_line() {
    // Verify SSE line parsing
}
```

### Integration Tests (vol-llm-provider/tests/)

```rust
#[tokio::test]
async fn test_anthropic_stream_basic() {
    // Mock SSE response, verify events emitted correctly
}

#[tokio::test]
async fn test_anthropic_stream_with_tool_call() {
    // Mock tool call streaming, verify ToolCallComplete event
}
```

### Mock Data

```rust
const MOCK_SSE_RESPONSE: &str = r#"
data: {"type": "message_start", "message": {"id": "msg_1", "model": "qwen3.5-plus"}}
data: {"type": "content_block_start", "content_block": {"type": "text"}}
data: {"type": "content_block_delta", "delta": {"text": "Hello"}}
data: {"type": "content_block_delta", "delta": {"text": " world"}}
data: {"type": "content_block_stop"}
data: {"type": "message_delta", "usage": {"output_tokens": 10}}
data: {"type": "message_stop"}
"#;
```

---

## Backward Compatibility

- Existing `converse()` method unchanged
- New `converse_stream()` method optional for implementors
- Default implementation can return `Err(LLMError::NotImplemented)`

---

## Future Work

1. Implement `OpenAIProvider::converse_stream()` using same `StreamingSession`
2. Add Agent support for streaming ReAct loop
3. Consider adding `StreamSender` wrapper for easier testing

---

## Appendix: Files to Modify

| File | Change Type |
|------|-------------|
| `crates/vol-llm-core/src/stream.rs` | Refactor to merge event type and data |
| `crates/vol-llm-core/src/streaming.rs` | NEW - Create |
| `crates/vol-llm-core/src/lib.rs` | Export streaming module |
| `crates/vol-llm-provider/src/anthropic.rs` | Implement converse_stream |
