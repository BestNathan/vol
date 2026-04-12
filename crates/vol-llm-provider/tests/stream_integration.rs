//! Integration tests for streaming with mock HTTP server.

use tokio::net::TcpListener;
use vol_llm_core::{ConversationRequest, LLMClient, LLMProvider, StreamEventData};
use vol_llm_provider::{AnthropicProvider, LLMConfig, Secret};

/// Simple mock HTTP server that returns SSE stream
async fn spawn_mock_sse_server(response: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let addr = format!("http://127.0.0.1:{}", port);

    tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();

        // Read request (and ignore it)
        let mut buffer = [0u8; 2048];
        let _ = socket.readable().await;
        let _ = socket.try_read(&mut buffer);

        // Send response - each line must end with \n
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

        // Keep connection open longer to ensure client reads all data
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
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
        provider: LLMProvider::Anthropic,
        model: "qwen3.5-plus".to_string(),
        base_url,
        api_key: Secret::literal("test-key"),
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
    assert!(events
        .iter()
        .any(|e| matches!(e.data, StreamEventData::ResponseStart { .. })));
    assert!(events
        .iter()
        .any(|e| matches!(e.data, StreamEventData::ContentDelta { .. })));
    assert!(events
        .iter()
        .any(|e| matches!(e.data, StreamEventData::ContentComplete { .. })));
    assert!(events
        .iter()
        .any(|e| matches!(e.data, StreamEventData::ResponseComplete { .. })));
}

#[tokio::test]
async fn test_anthropic_stream_with_tool_call() {
    // Mock SSE response with tool call
    // Tool call arguments should be: {"city": "Beijing"}
    // Split into chunks: {"city": "Beijing  +  "}
    const MOCK_RESPONSE: &str = r#"data: {"type": "message_start", "message": {"id": "msg_1", "model": "qwen3.5-plus"}}
data: {"type": "content_block_start", "content_block": {"type": "tool_use", "id": "tool_1", "name": "get_weather"}}
data: {"type": "content_block_delta", "delta": {"partial_json": "{\"city\": \"Beijing\""}}
data: {"type": "content_block_delta", "delta": {"partial_json": "}"}}
data: {"type": "content_block_stop"}
data: {"type": "message_delta", "usage": {"input_tokens": 10, "output_tokens": 5}}
data: {"type": "message_stop", "stop_reason": "tool_use"}
"#;

    let base_url = spawn_mock_sse_server(MOCK_RESPONSE).await;

    let config = LLMConfig {
        provider: LLMProvider::Anthropic,
        model: "qwen3.5-plus".to_string(),
        base_url,
        api_key: Secret::literal("test-key"),
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
    let tool_call_event = events
        .iter()
        .find(|e| matches!(e.data, StreamEventData::ToolCallComplete { .. }));
    assert!(tool_call_event.is_some(), "Expected ToolCallComplete event");

    if let Some(StreamEventData::ToolCallComplete { tool_call }) = tool_call_event.map(|e| &e.data)
    {
        assert_eq!(tool_call.id, "tool_1");
        assert_eq!(tool_call.name, "get_weather");
        assert_eq!(tool_call.arguments, r#"{"city": "Beijing"}"#);
    }
}

#[tokio::test]
async fn test_anthropic_stream_error_handling() {
    // Test that network errors are properly propagated
    let config = LLMConfig {
        provider: LLMProvider::Anthropic,
        model: "qwen3.5-plus".to_string(),
        base_url: "http://127.0.0.1:1".to_string(), // Invalid port
        api_key: Secret::literal("test-key"),
    };

    let provider = AnthropicProvider::new(&config).unwrap();
    let request = ConversationRequest::simple("Test");

    // Should return error (connection refused)
    let result = provider.converse_stream(request).await;
    assert!(result.is_err());
}
