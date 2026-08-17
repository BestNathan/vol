//! Tests for proxy support in provider HTTP client construction.
//!
//! These tests mutate process-global proxy environment variables, so they
//! live in their own test binary (own process) and are serialized behind a
//! static mutex to avoid races between tests in this file.

use std::sync::Mutex;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use vol_llm_core::{ConversationRequest, LLMClient, LLMError, LLMProvider};
use vol_llm_provider::{AnthropicProvider, LLMConfig, OpenaiProvider, Secret};

static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Spawn a tiny one-shot HTTP server returning an empty 200 response.
async fn spawn_ok_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let addr = format!("http://127.0.0.1:{port}");
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 4096];
        let _ = socket.read(&mut buf).await;
        let response = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}";
        let _ = socket.write_all(response.as_bytes()).await;
        let _ = socket.shutdown().await;
    });
    addr
}

fn clear_proxy_env() {
    for var in [
        "HTTPS_PROXY",
        "https_proxy",
        "HTTP_PROXY",
        "http_proxy",
        "NO_PROXY",
        "no_proxy",
    ] {
        std::env::remove_var(var);
    }
}

#[tokio::test]
async fn anthropic_build_client_uses_proxy_and_no_proxy_exclusions() {
    let _guard = ENV_LOCK.lock().unwrap();
    clear_proxy_env();

    // A proxy pointing at a dead port must make requests fail...
    std::env::set_var("HTTPS_PROXY", "http://127.0.0.1:9");
    std::env::remove_var("NO_PROXY");
    std::env::remove_var("no_proxy");

    let base_url = spawn_ok_server().await;
    let config = LLMConfig {
        provider: LLMProvider::Anthropic,
        model: "claude-test".to_string(),
        base_url,
        api_key: Secret::literal("k"),
        body: None,
        headers: None,
    };
    let provider = AnthropicProvider::new(&config).unwrap();
    let err = provider
        .converse(ConversationRequest::simple("hi"))
        .await
        .err()
        .unwrap();
    assert!(
        matches!(err, LLMError::Network(_)),
        "request through dead proxy must fail with Network error, got {err:?}"
    );

    // ...but NO_PROXY must bypass the proxy for the listed host.
    std::env::set_var("NO_PROXY", "127.0.0.1,example.com");
    let base_url = spawn_ok_server().await;
    let config = LLMConfig {
        provider: LLMProvider::Anthropic,
        model: "claude-test".to_string(),
        base_url,
        api_key: Secret::literal("k"),
        body: None,
        headers: None,
    };
    let provider = AnthropicProvider::new(&config).unwrap();
    provider
        .converse(ConversationRequest::simple("hi"))
        .await
        .expect("NO_PROXY entry must bypass the proxy");

    // Lowercase proxy variable also respected.
    std::env::remove_var("HTTPS_PROXY");
    std::env::set_var("https_proxy", "http://127.0.0.1:9");
    std::env::remove_var("NO_PROXY");
    let base_url = spawn_ok_server().await;
    let config = LLMConfig {
        provider: LLMProvider::Anthropic,
        model: "claude-test".to_string(),
        base_url,
        api_key: Secret::literal("k"),
        body: None,
        headers: None,
    };
    let provider = AnthropicProvider::new(&config).unwrap();
    let err = provider
        .converse(ConversationRequest::simple("hi"))
        .await
        .err()
        .unwrap();
    assert!(matches!(err, LLMError::Network(_)));

    clear_proxy_env();
}

#[tokio::test]
async fn anthropic_build_client_rejects_invalid_proxy_url() {
    let _guard = ENV_LOCK.lock().unwrap();
    clear_proxy_env();

    std::env::set_var("HTTPS_PROXY", "://not a valid proxy url");
    let config = LLMConfig {
        provider: LLMProvider::Anthropic,
        model: "claude-test".to_string(),
        base_url: "http://127.0.0.1:1".to_string(),
        api_key: Secret::literal("k"),
        body: None,
        headers: None,
    };
    let err = AnthropicProvider::new(&config).err().unwrap();
    assert!(
        matches!(err, LLMError::Network(_)),
        "invalid proxy URL must surface as Network error, got {err:?}"
    );
    clear_proxy_env();
}

#[tokio::test]
async fn openai_build_client_uses_proxy() {
    let _guard = ENV_LOCK.lock().unwrap();
    clear_proxy_env();

    // OpenAI's NO_PROXY exclusion list is hardcoded to api.openai.com, so a
    // configured proxy must apply to localhost too.
    std::env::set_var("HTTPS_PROXY", "http://127.0.0.1:9");
    let base_url = spawn_ok_server().await;
    let config = LLMConfig {
        provider: LLMProvider::OpenAI,
        model: "gpt-4o".to_string(),
        base_url,
        api_key: Secret::literal("k"),
        body: None,
        headers: None,
    };
    let provider = OpenaiProvider::new(&config).unwrap();
    let err = provider
        .converse(ConversationRequest::simple("hi"))
        .await
        .err()
        .unwrap();
    assert!(
        matches!(err, LLMError::Network(_)),
        "request through dead proxy must fail with Network error, got {err:?}"
    );

    // Without any proxy configured, the same request must succeed.
    clear_proxy_env();
    let base_url = spawn_ok_server().await;
    let config = LLMConfig {
        provider: LLMProvider::OpenAI,
        model: "gpt-4o".to_string(),
        base_url,
        api_key: Secret::literal("k"),
        body: None,
        headers: None,
    };
    let provider = OpenaiProvider::new(&config).unwrap();
    provider
        .converse(ConversationRequest::simple("hi"))
        .await
        .expect("request without proxy must succeed");
}
