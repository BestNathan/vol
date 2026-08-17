//! Integration tests for non-streaming `converse` and OpenAI streaming,
//! using a mock HTTP server that captures the request body so we can assert
//! on the exact wire payload the provider sends.

use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use vol_llm_core::{
    ConversationRequest, FinishReason, LLMClient, LLMError, LLMProvider, Message, ModelConfig,
    StreamEventData, ToolDefinition,
};
use vol_llm_provider::{AnthropicProvider, LLMConfig, OpenaiProvider, Secret};

/// Build an HTTP/1.1 response string with Content-Length.
fn http_response(status: &str, content_type: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

/// Spawn a one-shot mock HTTP server. Returns (base_url, body_receiver).
/// The server reads the full request (headers + body), sends the raw request
/// bytes on the channel, then responds and closes.
async fn spawn_mock_server(response: String) -> (String, oneshot::Receiver<Vec<u8>>) {
    spawn_mock_server_bytes(response.into_bytes()).await
}

/// Like [`spawn_mock_server`] but with a raw byte response body, allowing
/// responses that are not valid UTF-8.
async fn spawn_mock_server_bytes(response: Vec<u8>) -> (String, oneshot::Receiver<Vec<u8>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let addr = format!("http://127.0.0.1:{port}");
    let (tx, rx) = oneshot::channel();

    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = vec![0u8; 16 * 1024];
        let mut received = 0usize;
        let mut header_end: Option<usize> = None;
        let mut content_length = 0usize;

        loop {
            match socket.read(&mut buf[received..]).await {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    received += n;
                    if header_end.is_none() {
                        if let Some(pos) = buf[..received].windows(4).position(|w| w == b"\r\n\r\n")
                        {
                            header_end = Some(pos + 4);
                            let headers = String::from_utf8_lossy(&buf[..pos]);
                            content_length = headers
                                .lines()
                                .find_map(|line| {
                                    let line = line.trim();
                                    let lower = line.to_ascii_lowercase();
                                    lower
                                        .strip_prefix("content-length:")
                                        .and_then(|v| v.trim().parse::<usize>().ok())
                                })
                                .unwrap_or(0);
                        }
                    }
                    let complete = match header_end {
                        Some(end) if content_length > 0 => received >= end + content_length,
                        Some(_) => true,
                        None => false,
                    };
                    if complete {
                        break;
                    }
                }
            }
        }

        let _ = tx.send(buf[..received].to_vec());
        let _ = socket.write_all(&response).await;
        let _ = socket.shutdown().await;
    });

    (addr, rx)
}

/// Wait for the captured request and parse its JSON body.
async fn captured_json_body(rx: oneshot::Receiver<Vec<u8>>) -> serde_json::Value {
    let raw = tokio::time::timeout(Duration::from_secs(10), rx)
        .await
        .expect("mock server timed out")
        .expect("mock server closed without capturing request");
    let text = String::from_utf8_lossy(&raw);
    let body = text.split_once("\r\n\r\n").map(|(_, b)| b).unwrap_or(&text);
    serde_json::from_str(body).expect("request body must be valid JSON")
}

fn anthropic_config(base_url: String) -> LLMConfig {
    LLMConfig {
        provider: LLMProvider::Anthropic,
        model: "claude-test".to_string(),
        base_url,
        api_key: Secret::literal("test-key"),
        body: None,
        headers: None,
    }
}

fn openai_config(base_url: String) -> LLMConfig {
    LLMConfig {
        provider: LLMProvider::OpenAI,
        model: "gpt-4o".to_string(),
        base_url,
        api_key: Secret::literal("test-key"),
        body: None,
        headers: None,
    }
}

fn body_map(value: serde_json::Value) -> std::collections::HashMap<String, serde_json::Value> {
    serde_json::from_value(value).unwrap()
}

fn weather_tool() -> ToolDefinition {
    ToolDefinition {
        name: "get_weather".to_string(),
        description: Some("Get weather for a city".to_string()),
        parameters: Some(serde_json::json!({
            "type": "object",
            "properties": {"city": {"type": "string"}},
        })),
    }
}

// ---------------------------------------------------------------------------
// Anthropic converse
// ---------------------------------------------------------------------------

const ANTHROPIC_TEXT_RESPONSE: &str = r#"{
    "id": "msg_1",
    "model": "claude-3-5-sonnet",
    "content": [
        {"type": "text", "text": "Hello there"},
        {"type": "text", "text": "How can I help?"}
    ],
    "usage": {"input_tokens": 10, "output_tokens": 5},
    "stop_reason": "end_turn"
}"#;

#[tokio::test]
async fn anthropic_converse_builds_full_request_and_parses_text() {
    let (base_url, rx) = spawn_mock_server(http_response(
        "200 OK",
        "application/json",
        ANTHROPIC_TEXT_RESPONSE,
    ))
    .await;
    let provider = AnthropicProvider::new(&anthropic_config(base_url)).unwrap();

    let request = ConversationRequest {
        system: Some("You are helpful".to_string()),
        messages: vec![Message::system("ignored system"), Message::user("Hello")],
        model_config: ModelConfig {
            max_tokens: Some(4096),
            temperature: Some(0.7),
            top_p: Some(0.9),
            ..Default::default()
        },
        tools: Some(vec![weather_tool()]),
        ..Default::default()
    };

    let response = provider.converse(request).await.unwrap();

    // Wire payload assertions
    let body = captured_json_body(rx).await;
    assert_eq!(body["model"], "claude-test");
    assert_eq!(body["max_tokens"], 4096);
    assert_eq!(body["system"], "You are helpful");
    assert_eq!(body["temperature"], 0.7);
    assert_eq!(body["top_p"], 0.9);
    assert_eq!(body["tools"][0]["name"], "get_weather");
    // System messages must NOT appear in the messages array
    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["role"], "user");

    // Response assertions
    assert_eq!(response.message.role, vol_llm_core::MessageRole::Assistant);
    assert_eq!(
        response.message.content.as_ref().unwrap().as_str(),
        "Hello there\n\nHow can I help?"
    );
    assert_eq!(response.model, "claude-3-5-sonnet");
    assert_eq!(response.usage.prompt_tokens, 10);
    assert_eq!(response.usage.completion_tokens, 5);
    assert_eq!(response.usage.total_tokens, 15);
    assert_eq!(response.finish_reason, FinishReason::Stop);
    assert!(response.raw.is_some());
}

const ANTHROPIC_TOOL_RESPONSE: &str = r#"{
    "id": "msg_2",
    "model": "claude-3-5-sonnet",
    "content": [
        {"type": "text", "text": "Let me check."},
        {"type": "tool_use", "id": "tool_1", "name": "get_weather", "input": {"city": "Beijing"}}
    ],
    "usage": {"input_tokens": 20, "output_tokens": 6},
    "stop_reason": "tool_use"
}"#;

#[tokio::test]
async fn anthropic_converse_parses_tool_use_and_finish_reason() {
    let (base_url, rx) = spawn_mock_server(http_response(
        "200 OK",
        "application/json",
        ANTHROPIC_TOOL_RESPONSE,
    ))
    .await;
    let provider = AnthropicProvider::new(&anthropic_config(base_url)).unwrap();

    let response = provider
        .converse(ConversationRequest::simple("Weather?"))
        .await
        .unwrap();

    // Wire payload: max_tokens defaults to 8192 when not configured
    let body = captured_json_body(rx).await;
    assert_eq!(body["max_tokens"], 8192);

    let tool_calls = response.message.tool_calls.as_ref().unwrap();
    assert_eq!(tool_calls.len(), 1);
    assert_eq!(tool_calls[0].id, "tool_1");
    assert_eq!(tool_calls[0].name, "get_weather");
    assert_eq!(tool_calls[0].arguments, r#"{"city":"Beijing"}"#);
    assert_eq!(response.finish_reason, FinishReason::ToolCalls);
    // Content is still captured alongside tool calls
    assert_eq!(
        response.message.content.as_ref().unwrap().as_str(),
        "Let me check."
    );
}

#[tokio::test]
async fn anthropic_converse_applies_body_defaults_and_request_wins() {
    // Case 1: request has no temperature -> body default temperature applied,
    // max_tokens from body default used.
    let (base_url, rx) = spawn_mock_server(http_response(
        "200 OK",
        "application/json",
        ANTHROPIC_TEXT_RESPONSE,
    ))
    .await;
    let mut config = anthropic_config(base_url);
    config.body = Some(body_map(serde_json::json!({
        "max_tokens": 777,
        "temperature": 0.5,
        "custom_param": "x"
    })));
    let provider = AnthropicProvider::new(&config).unwrap();

    let _ = provider
        .converse(ConversationRequest::simple("hi"))
        .await
        .unwrap();
    let body = captured_json_body(rx).await;
    assert_eq!(body["max_tokens"], 777);
    assert_eq!(body["temperature"], 0.5);
    assert_eq!(body["custom_param"], "x");

    // Case 2: request provides temperature -> request wins over body default.
    let (base_url, rx) = spawn_mock_server(http_response(
        "200 OK",
        "application/json",
        ANTHROPIC_TEXT_RESPONSE,
    ))
    .await;
    let mut config = anthropic_config(base_url);
    config.body = Some(body_map(
        serde_json::json!({"temperature": 0.5, "custom_param": "x"}),
    ));
    let provider = AnthropicProvider::new(&config).unwrap();

    let request = ConversationRequest {
        messages: vec![Message::user("hi")],
        model_config: ModelConfig {
            temperature: Some(0.1),
            ..Default::default()
        },
        ..Default::default()
    };
    let _ = provider.converse(request).await.unwrap();
    let body = captured_json_body(rx).await;
    assert_eq!(body["temperature"], 0.1);
    assert_eq!(body["custom_param"], "x");
    assert_eq!(body["max_tokens"], 8192);
}

#[tokio::test]
async fn anthropic_converse_maps_length_and_unknown_finish_reasons() {
    let (base_url, rx) = spawn_mock_server(http_response(
        "200 OK",
        "application/json",
        r#"{"content": [{"type": "text", "text": "cut off"}], "usage": {}, "stop_reason": "max_tokens"}"#,
    ))
    .await;
    let provider = AnthropicProvider::new(&anthropic_config(base_url)).unwrap();
    let response = provider
        .converse(ConversationRequest::simple("hi"))
        .await
        .unwrap();
    let _ = captured_json_body(rx).await;
    assert_eq!(response.finish_reason, FinishReason::Length);

    let (base_url, rx) = spawn_mock_server(http_response(
        "200 OK",
        "application/json",
        r#"{"content": [], "stop_reason": "weird_reason"}"#,
    ))
    .await;
    let provider = AnthropicProvider::new(&anthropic_config(base_url)).unwrap();
    let response = provider
        .converse(ConversationRequest::simple("hi"))
        .await
        .unwrap();
    let _ = captured_json_body(rx).await;
    assert_eq!(response.finish_reason, FinishReason::Other);
    assert_eq!(response.message.content.as_ref().unwrap().as_str(), "");
}

#[tokio::test]
async fn anthropic_converse_returns_api_error_with_json_message() {
    let (base_url, rx) = spawn_mock_server(http_response(
        "401 Unauthorized",
        "application/json",
        r#"{"error": {"type": "authentication_error", "message": "invalid x-api-key"}}"#,
    ))
    .await;
    let provider = AnthropicProvider::new(&anthropic_config(base_url)).unwrap();

    let err = provider
        .converse(ConversationRequest::simple("hi"))
        .await
        .err()
        .unwrap();
    let _ = captured_json_body(rx).await;
    match err {
        LLMError::Api { status, message } => {
            assert_eq!(status, 401);
            assert_eq!(message, "invalid x-api-key");
        }
        other => panic!("expected Api error, got {other:?}"),
    }
}

#[tokio::test]
async fn anthropic_converse_returns_api_error_with_plain_text() {
    let (base_url, rx) = spawn_mock_server(http_response(
        "500 Internal Server Error",
        "text/plain",
        "Upstream exploded",
    ))
    .await;
    let provider = AnthropicProvider::new(&anthropic_config(base_url)).unwrap();

    let err = provider
        .converse(ConversationRequest::simple("hi"))
        .await
        .err()
        .unwrap();
    let _ = captured_json_body(rx).await;
    match err {
        LLMError::Api { status, message } => {
            assert_eq!(status, 500);
            assert_eq!(message, "Upstream exploded");
        }
        other => panic!("expected Api error, got {other:?}"),
    }
}

#[tokio::test]
async fn anthropic_converse_stream_returns_api_error() {
    let (base_url, rx) = spawn_mock_server(http_response(
        "429 Too Many Requests",
        "application/json",
        r#"{"error": {"message": "rate limited"}}"#,
    ))
    .await;
    let provider = AnthropicProvider::new(&anthropic_config(base_url)).unwrap();

    let err = provider
        .converse_stream(ConversationRequest::simple("hi"))
        .await
        .err()
        .unwrap();
    let _ = captured_json_body(rx).await;
    match err {
        LLMError::Api { status, message } => {
            assert_eq!(status, 429);
            assert_eq!(message, "rate limited");
        }
        other => panic!("expected Api error, got {other:?}"),
    }
}

#[tokio::test]
async fn anthropic_converse_stream_propagates_invalid_utf8_chunk() {
    // Server sends a body that is not valid UTF-8: the streaming task must
    // surface a Parse error on the receiver.
    let mut body = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        2 + " broken\r\n\r\ndata: [DONE]".len()
    )
    .into_bytes();
    body.extend_from_slice(&[0xff, 0xfe]);
    body.extend_from_slice(b" broken\r\n\r\ndata: [DONE]");
    let (base_url, _rx) = spawn_mock_server_bytes(body).await;
    let provider = AnthropicProvider::new(&anthropic_config(base_url)).unwrap();

    let mut receiver = provider
        .converse_stream(ConversationRequest::simple("hi"))
        .await
        .unwrap();

    let result = tokio::time::timeout(Duration::from_secs(10), receiver.recv())
        .await
        .expect("stream timed out")
        .expect("stream closed without error");
    match result {
        Err(LLMError::Parse(msg)) => assert!(!msg.is_empty()),
        other => panic!("expected Parse error, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// OpenAI converse
// ---------------------------------------------------------------------------

const OPENAI_TOOL_RESPONSE: &str = r#"{
    "id": "chatcmpl-1",
    "model": "gpt-4o",
    "choices": [{
        "index": 0,
        "message": {
            "role": "assistant",
            "content": "Checking weather...",
            "tool_calls": [{
                "id": "call_1",
                "type": "function",
                "function": {"name": "get_weather", "arguments": "{\"city\":\"Beijing\"}"}
            }]
        },
        "finish_reason": "tool_calls"
    }],
    "usage": {"prompt_tokens": 20, "completion_tokens": 8, "total_tokens": 28}
}"#;

#[tokio::test]
async fn openai_converse_builds_full_request_and_parses_tool_calls() {
    let (base_url, rx) = spawn_mock_server(http_response(
        "200 OK",
        "application/json",
        OPENAI_TOOL_RESPONSE,
    ))
    .await;
    let provider = OpenaiProvider::new(&openai_config(base_url)).unwrap();

    let request = ConversationRequest {
        messages: vec![
            Message::system("You are helpful"),
            Message::user("Weather?"),
        ],
        model_config: ModelConfig {
            max_tokens: Some(2048),
            temperature: Some(0.7),
            top_p: Some(0.9),
            top_k: Some(40),
            frequency_penalty: Some(-0.5),
            presence_penalty: Some(1.5),
            stop: Some(vec!["END".to_string()]),
            seed: Some(42),
            logprobs: Some(5),
            ..Default::default()
        },
        tools: Some(vec![weather_tool()]),
        ..Default::default()
    };

    let response = provider.converse(request).await.unwrap();

    // Wire payload assertions
    let body = captured_json_body(rx).await;
    assert_eq!(body["model"], "gpt-4o");
    assert_eq!(body["max_tokens"], 2048);
    assert_eq!(body["temperature"], 0.7);
    assert_eq!(body["top_p"], 0.9);
    assert_eq!(body["top_k"], 40);
    assert_eq!(body["frequency_penalty"], -0.5);
    assert_eq!(body["presence_penalty"], 1.5);
    assert_eq!(body["stop"][0], "END");
    assert_eq!(body["seed"], 42);
    assert_eq!(body["logprobs"], 5);
    assert_eq!(body["tools"][0]["type"], "function");
    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages[0]["role"], "system");
    assert_eq!(messages[1]["role"], "user");

    // Response assertions
    assert_eq!(
        response.message.content.as_ref().unwrap().as_str(),
        "Checking weather..."
    );
    let tool_calls = response.message.tool_calls.as_ref().unwrap();
    assert_eq!(tool_calls[0].id, "call_1");
    assert_eq!(tool_calls[0].name, "get_weather");
    // `function.arguments` is a plain JSON string of the arguments; a string
    // value must pass through as its raw content, not the JSON-quoted form.
    assert_eq!(tool_calls[0].arguments, r#"{"city":"Beijing"}"#);
    assert_eq!(response.usage.prompt_tokens, 20);
    assert_eq!(response.usage.completion_tokens, 8);
    assert_eq!(response.usage.total_tokens, 28);
    assert_eq!(response.finish_reason, FinishReason::ToolCalls);
    assert_eq!(response.model, "gpt-4o");
}

#[tokio::test]
async fn openai_converse_maps_stop_and_content_filter_reasons() {
    let (base_url, rx) = spawn_mock_server(http_response(
        "200 OK",
        "application/json",
        r#"{"choices": [{"index": 0, "message": {"role": "assistant", "content": "Done"}, "finish_reason": "stop"}], "usage": {}}"#,
    ))
    .await;
    let provider = OpenaiProvider::new(&openai_config(base_url)).unwrap();
    let response = provider
        .converse(ConversationRequest::simple("hi"))
        .await
        .unwrap();
    let _ = captured_json_body(rx).await;
    assert_eq!(response.finish_reason, FinishReason::Stop);

    let (base_url, rx) = spawn_mock_server(http_response(
        "200 OK",
        "application/json",
        r#"{"choices": [{"index": 0, "message": {"role": "assistant", "content": null}, "finish_reason": "content_filter"}], "usage": {}}"#,
    ))
    .await;
    let provider = OpenaiProvider::new(&openai_config(base_url)).unwrap();
    let response = provider
        .converse(ConversationRequest::simple("hi"))
        .await
        .unwrap();
    let _ = captured_json_body(rx).await;
    assert_eq!(response.finish_reason, FinishReason::ContentFilter);
    // content: null -> empty string
    assert_eq!(response.message.content.as_ref().unwrap().as_str(), "");
}

#[tokio::test]
async fn openai_converse_maps_length_and_unknown_reasons() {
    let (base_url, rx) = spawn_mock_server(http_response(
        "200 OK",
        "application/json",
        r#"{"choices": [{"index": 0, "message": {"role": "assistant", "content": "cut"}, "finish_reason": "length"}], "usage": {}}"#,
    ))
    .await;
    let provider = OpenaiProvider::new(&openai_config(base_url)).unwrap();
    let response = provider
        .converse(ConversationRequest::simple("hi"))
        .await
        .unwrap();
    let _ = captured_json_body(rx).await;
    assert_eq!(response.finish_reason, FinishReason::Length);

    let (base_url, rx) = spawn_mock_server(http_response(
        "200 OK",
        "application/json",
        r#"{"choices": [{"index": 0, "message": {"role": "assistant", "content": "x"}, "finish_reason": "weird"}], "usage": {}}"#,
    ))
    .await;
    let provider = OpenaiProvider::new(&openai_config(base_url)).unwrap();
    let response = provider
        .converse(ConversationRequest::simple("hi"))
        .await
        .unwrap();
    let _ = captured_json_body(rx).await;
    assert_eq!(response.finish_reason, FinishReason::Other);
}

#[tokio::test]
async fn openai_converse_applies_body_defaults_and_request_wins() {
    // Defaults applied when request omits parameters.
    let (base_url, rx) = spawn_mock_server(http_response(
        "200 OK",
        "application/json",
        r#"{"choices": [{"index": 0, "message": {"role": "assistant", "content": "ok"}, "finish_reason": "stop"}], "usage": {}}"#,
    ))
    .await;
    let mut config = openai_config(base_url);
    config.body = Some(body_map(serde_json::json!({
        "max_tokens": 999,
        "temperature": 0.5,
        "frequency_penalty": 0.2
    })));
    let provider = OpenaiProvider::new(&config).unwrap();

    let _ = provider
        .converse(ConversationRequest::simple("hi"))
        .await
        .unwrap();
    let body = captured_json_body(rx).await;
    assert_eq!(body["max_tokens"], 999);
    assert_eq!(body["temperature"], 0.5);
    assert_eq!(body["frequency_penalty"], 0.2);

    // Request-provided temperature overrides the body default.
    let (base_url, rx) = spawn_mock_server(http_response(
        "200 OK",
        "application/json",
        r#"{"choices": [{"index": 0, "message": {"role": "assistant", "content": "ok"}, "finish_reason": "stop"}], "usage": {}}"#,
    ))
    .await;
    let mut config = openai_config(base_url);
    config.body = Some(body_map(
        serde_json::json!({"temperature": 0.5, "custom": "v"}),
    ));
    let provider = OpenaiProvider::new(&config).unwrap();

    let request = ConversationRequest {
        messages: vec![Message::user("hi")],
        model_config: ModelConfig {
            temperature: Some(0.9),
            ..Default::default()
        },
        ..Default::default()
    };
    let _ = provider.converse(request).await.unwrap();
    let body = captured_json_body(rx).await;
    assert_eq!(body["temperature"], 0.9);
    assert_eq!(body["custom"], "v");
    // max_tokens default of 4096 used since none set
    assert_eq!(body["max_tokens"], 4096);
}

#[tokio::test]
async fn openai_converse_returns_api_error_with_json_message() {
    let (base_url, rx) = spawn_mock_server(http_response(
        "401 Unauthorized",
        "application/json",
        r#"{"error": {"message": "Incorrect API key provided"}}"#,
    ))
    .await;
    let provider = OpenaiProvider::new(&openai_config(base_url)).unwrap();

    let err = provider
        .converse(ConversationRequest::simple("hi"))
        .await
        .err()
        .unwrap();
    let _ = captured_json_body(rx).await;
    match err {
        LLMError::Api { status, message } => {
            assert_eq!(status, 401);
            assert_eq!(message, "Incorrect API key provided");
        }
        other => panic!("expected Api error, got {other:?}"),
    }
}

#[tokio::test]
async fn openai_converse_returns_api_error_with_plain_text() {
    let (base_url, rx) = spawn_mock_server(http_response(
        "503 Service Unavailable",
        "text/plain",
        "try again later",
    ))
    .await;
    let provider = OpenaiProvider::new(&openai_config(base_url)).unwrap();

    let err = provider
        .converse(ConversationRequest::simple("hi"))
        .await
        .err()
        .unwrap();
    let _ = captured_json_body(rx).await;
    match err {
        LLMError::Api { status, message } => {
            assert_eq!(status, 503);
            assert_eq!(message, "try again later");
        }
        other => panic!("expected Api error, got {other:?}"),
    }
}

#[tokio::test]
async fn openai_converse_drops_request_system_field() {
    // NOTE: This test PINNS the current wire behavior: the OpenAI provider
    // never serializes `ConversationRequest.system` onto the wire — the field
    // is silently ignored by `converse`/`converse_stream`, which only convert
    // `request.messages`. The `convert_messages` doc comment says "System
    // prompt is sent as the first message with role: system", so a caller must
    // embed the system prompt in the messages array (e.g. `Message::system`)
    // for it to reach the API. This may be a deliberate convention or a bug;
    // either way the test documents what ships. If the provider ever starts
    // forwarding `request.system`, this test must be updated.
    let (base_url, rx) = spawn_mock_server(http_response(
        "200 OK",
        "application/json",
        OPENAI_TOOL_RESPONSE,
    ))
    .await;
    let provider = OpenaiProvider::new(&openai_config(base_url)).unwrap();

    let request = ConversationRequest {
        system: Some("PINNED-SYSTEM-PROMPT".to_string()),
        messages: vec![Message::user("hi")],
        ..Default::default()
    };
    let response = provider.converse(request).await.unwrap();
    assert_eq!(
        response.message.content.as_ref().unwrap().as_str(),
        "Checking weather..."
    );

    // Wire payload: `system` must not appear anywhere in the request JSON and
    // the messages array must be exactly the caller-provided messages.
    let body = captured_json_body(rx).await;
    assert!(
        body.get("system").is_none(),
        "request.system must not be serialized to the wire"
    );
    assert!(
        !body.to_string().contains("PINNED-SYSTEM-PROMPT"),
        "request.system content must not leak into the wire payload"
    );
    assert_eq!(
        body["messages"],
        serde_json::json!([{"role": "user", "content": "hi"}])
    );
}

// ---------------------------------------------------------------------------
// OpenAI converse_stream
// ---------------------------------------------------------------------------

const OPENAI_SSE_BODY: &str = r#"data: {"id":"chatcmpl-1","model":"gpt-4o","choices":[{"index":0,"delta":{},"finish_reason":null}]}
data: {"id":"chatcmpl-1","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}
data: {"id":"chatcmpl-1","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"get_weather","arguments":""}}],"content":""},"finish_reason":null}]}
data: {"id":"chatcmpl-1","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"city\":\"Beijing\"}"}}]},"finish_reason":null}]}
data: {"id":"chatcmpl-1","usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}}
data: {"id":"chatcmpl-1","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}
data: [DONE]
"#;

#[tokio::test]
async fn openai_converse_stream_full_sse() {
    let (base_url, rx) = spawn_mock_server(http_response(
        "200 OK",
        "text/event-stream",
        OPENAI_SSE_BODY,
    ))
    .await;
    let provider = OpenaiProvider::new(&openai_config(base_url)).unwrap();

    let request = ConversationRequest {
        messages: vec![Message::user("hi")],
        model_config: ModelConfig {
            max_tokens: Some(512),
            ..Default::default()
        },
        ..Default::default()
    };
    let mut receiver = provider.converse_stream(request).await.unwrap();

    // Wire payload: stream + stream_options must be present.
    let body = captured_json_body(rx).await;
    assert_eq!(body["stream"], true);
    assert_eq!(body["stream_options"]["include_usage"], true);
    assert_eq!(body["max_tokens"], 512);

    let mut saw_start = false;
    let mut saw_content = false;
    let mut saw_content_complete = false;
    let mut saw_tool_arg_delta = false;
    let mut saw_usage = false;
    let mut saw_complete = false;

    while let Some(result) = tokio::time::timeout(Duration::from_secs(10), receiver.recv())
        .await
        .expect("stream timed out")
    {
        let event = result.expect("no error expected in stream");
        match event.data {
            StreamEventData::ResponseStart { model } => {
                assert_eq!(model, "gpt-4o");
                saw_start = true;
            }
            StreamEventData::ContentDelta { delta } => {
                assert_eq!(delta, "Hello");
                saw_content = true;
            }
            StreamEventData::ContentComplete { content } => {
                assert_eq!(content, "Hello");
                saw_content_complete = true;
            }
            StreamEventData::ToolCallArgumentDelta {
                tool_call_id,
                tool_name,
                delta,
            } => {
                assert_eq!(tool_call_id, "call_1");
                assert_eq!(tool_name, "get_weather");
                assert_eq!(delta, r#"{"city":"Beijing"}"#);
                saw_tool_arg_delta = true;
            }
            StreamEventData::UsageUpdate { usage } => {
                assert_eq!(usage.prompt_tokens, 10);
                assert_eq!(usage.completion_tokens, 5);
                assert_eq!(usage.total_tokens, 15);
                saw_usage = true;
            }
            StreamEventData::ResponseComplete { finish_reason } => {
                assert_eq!(finish_reason, FinishReason::Stop);
                saw_complete = true;
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    assert!(saw_start, "expected ResponseStart");
    assert!(saw_content, "expected ContentDelta");
    assert!(saw_content_complete, "expected ContentComplete");
    assert!(saw_tool_arg_delta, "expected ToolCallArgumentDelta");
    assert!(saw_usage, "expected UsageUpdate");
    assert!(saw_complete, "expected ResponseComplete");
}

#[tokio::test]
async fn openai_converse_stream_returns_api_error() {
    let (base_url, rx) = spawn_mock_server(http_response(
        "400 Bad Request",
        "application/json",
        r#"{"error": {"message": "invalid model"}}"#,
    ))
    .await;
    let provider = OpenaiProvider::new(&openai_config(base_url)).unwrap();

    let err = provider
        .converse_stream(ConversationRequest::simple("hi"))
        .await
        .err()
        .unwrap();
    let _ = captured_json_body(rx).await;
    match err {
        LLMError::Api { status, message } => {
            assert_eq!(status, 400);
            assert_eq!(message, "invalid model");
        }
        other => panic!("expected Api error, got {other:?}"),
    }
}

#[tokio::test]
async fn openai_converse_stream_propagates_invalid_utf8_chunk() {
    let mut body = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        2 + " broken".len()
    )
    .into_bytes();
    body.extend_from_slice(&[0xff, 0xfe]);
    body.extend_from_slice(b" broken");
    let (base_url, _rx) = spawn_mock_server_bytes(body).await;
    let provider = OpenaiProvider::new(&openai_config(base_url)).unwrap();

    let mut receiver = provider
        .converse_stream(ConversationRequest::simple("hi"))
        .await
        .unwrap();

    let result = tokio::time::timeout(Duration::from_secs(10), receiver.recv())
        .await
        .expect("stream timed out")
        .expect("stream closed without error");
    match result {
        Err(LLMError::Parse(msg)) => assert!(!msg.is_empty()),
        other => panic!("expected Parse error, got {other:?}"),
    }
}
