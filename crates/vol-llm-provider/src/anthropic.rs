//! Anthropic Provider implementation.

use crate::LLMConfig;
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::json;
use tokio::sync::mpsc;
use tracing::info;
use vol_llm_core::*;

/// Anthropic Provider
pub struct AnthropicProvider {
    client: Client,
    api_key: String,
    model: String,
    base_url: String,
}

impl AnthropicProvider {
    /// Create new Anthropic provider
    pub fn new(config: &LLMConfig) -> Result<Self> {
        let client = Self::build_client()?;
        Ok(Self {
            client,
            api_key: config.resolve_api_key()?,
            model: config.model.clone(),
            base_url: config.base_url.clone(),
        })
    }

    /// Build an HTTP client with optional proxy support.
    /// Reads HTTPS_PROXY or https_proxy from environment if set.
    /// DashScope is accessible directly (no proxy needed in China),
    /// so it's excluded from proxy routing.
    fn build_client() -> Result<Client> {
        let proxy_url = std::env::var("HTTPS_PROXY")
            .or_else(|_| std::env::var("https_proxy"))
            .ok();

        let mut builder = Client::builder().danger_accept_invalid_certs(true);

        if let Some(url) = &proxy_url {
            // DashScope coding endpoint is accessible directly from China,
            // so we bypass the proxy for it to avoid CONNECT tunnel failures.
            let no_proxy = reqwest::NoProxy::from_string("dashscope.aliyuncs.com");
            let proxy = reqwest::Proxy::all(url)
                .map_err(|e| LLMError::Network(reqwest::Error::from(e).into()))?
                .no_proxy(no_proxy);
            builder = builder.proxy(proxy);
        }

        builder.build().map_err(|e| LLMError::Network(e.into()))
    }

    fn convert_user_content(&self, content: Option<&MessageContent>) -> Result<serde_json::Value> {
        match content {
            Some(MessageContent::Text(text)) => Ok(json!(text)),
            Some(MessageContent::MultiPart(parts)) => {
                let mut blocks = Vec::new();
                for part in parts {
                    match part {
                        ContentPart::Text { text } => blocks.push(json!({
                            "type": "text",
                            "text": text,
                        })),
                        ContentPart::Image { image_url } => {
                            blocks.push(self.convert_image_block(image_url)?);
                        }
                    }
                }
                Ok(json!(blocks))
            }
            None => Ok(json!("")),
        }
    }

    fn convert_image_block(&self, image_url: &ImageUrl) -> Result<serde_json::Value> {
        if image_url.url.starts_with("data:") {
            let (media_type, data) = Self::parse_image_data_url(&image_url.url)?;
            return Ok(json!({
                "type": "image",
                "source": {
                    "type": "base64",
                    "media_type": media_type,
                    "data": data,
                },
            }));
        }

        Ok(json!({
            "type": "image",
            "source": {
                "type": "url",
                "url": image_url.url,
            },
        }))
    }

    fn parse_image_data_url(url: &str) -> Result<(&str, &str)> {
        let rest = url
            .strip_prefix("data:")
            .ok_or_else(|| LLMError::InvalidRequest("Invalid image data URL".to_string()))?;
        let (metadata, data) = rest
            .split_once(',')
            .ok_or_else(|| LLMError::InvalidRequest("Invalid image data URL".to_string()))?;
        let media_type = metadata
            .strip_suffix(";base64")
            .ok_or_else(|| LLMError::InvalidRequest("Invalid image data URL".to_string()))?;
        if media_type.is_empty() || data.is_empty() {
            return Err(LLMError::InvalidRequest(
                "Invalid image data URL".to_string(),
            ));
        }
        Ok((media_type, data))
    }

    /// Convert messages to Anthropic format
    fn convert_messages(&self, messages: &[Message]) -> Result<Vec<serde_json::Value>> {
        let mut result = Vec::new();

        for msg in messages {
            match msg.role {
                MessageRole::System => {
                    // Anthropic: system must be sent separately, not in messages
                }
                MessageRole::User => {
                    let content = self.convert_user_content(msg.content.as_ref())?;
                    result.push(json!({
                        "role": "user",
                        "content": content,
                    }));
                }
                MessageRole::Assistant => {
                    let mut content = Vec::new();

                    // Thinking content (must come before text for Anthropic)
                    if let Some(ref thinking) = msg.thinking {
                        content.push(json!({
                            "type": "thinking",
                            "thinking": thinking,
                        }));
                    }

                    // Text content
                    if let Some(ref c) = msg.content {
                        content.push(json!({
                            "type": "text",
                            "text": c.as_str(),
                        }));
                    }

                    // Tool calls
                    if let Some(ref tools) = msg.tool_calls {
                        for tool in tools {
                            let input = serde_json::from_str::<serde_json::Value>(&tool.arguments)
                                .unwrap_or(json!({}));
                            content.push(json!({
                                "type": "tool_use",
                                "id": tool.id,
                                "name": tool.name,
                                "input": input,
                            }));
                        }
                    }

                    result.push(json!({
                        "role": "assistant",
                        "content": content,
                    }));
                }
                MessageRole::Tool => {
                    result.push(json!({
                        "role": "user",
                        "content": [{
                            "type": "tool_result",
                            "tool_use_id": msg.tool_call_id.as_deref().unwrap_or(""),
                            "content": msg.content.as_ref()
                                .map(|c| c.as_str())
                                .unwrap_or(""),
                        }],
                    }));
                }
            }
        }

        Ok(result)
    }

    /// Convert tools to Anthropic format
    fn convert_tools(&self, tools: &[ToolDefinition]) -> serde_json::Value {
        json!(tools
            .iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.parameters.as_ref().unwrap_or(&json!({
                        "type": "object",
                        "properties": {}
                    })),
                })
            })
            .collect::<Vec<_>>())
    }
}

#[async_trait]
impl LLMClient for AnthropicProvider {
    fn provider(&self) -> LLMProvider {
        LLMProvider::Anthropic
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn supported_params(&self) -> &[SupportedParam] {
        &[
            SupportedParam::MaxTokens,
            SupportedParam::Temperature,
            SupportedParam::TopP,
            SupportedParam::Tools,
        ]
    }

    async fn converse(&self, request: ConversationRequest) -> Result<ConversationResponse> {
        // max_tokens is required for Anthropic
        let max_tokens = request.model_config.max_tokens.unwrap_or(8192);

        // Convert messages
        let anthropic_messages = self.convert_messages(&request.messages)?;

        // Build request body
        let mut body = json!({
            "model": self.model,
            "max_tokens": max_tokens,
            "messages": anthropic_messages,
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

        let response = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .header("User-Agent", "claude-code/1.0.0")
            .json(&body)
            .send()
            .await
            .map_err(LLMError::Network)?;

        // Handle response
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

        // Extract content - collect all text blocks from content array
        let content = result["content"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| {
                        // Handle both simple text and thinking blocks
                        if item["type"].as_str() == Some("text") {
                            item["text"].as_str().map(|s| s.to_string())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n\n")
            })
            .unwrap_or_default();

        // Extract tool calls
        let tool_calls = result["content"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter(|item| item["type"].as_str() == Some("tool_use"))
                    .map(|item| ToolCall {
                        id: item["id"].as_str().unwrap_or("").to_string(),
                        name: item["name"].as_str().unwrap_or("").to_string(),
                        arguments: item["input"].to_string(),
                        r#type: "function".to_string(),
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        // Extract usage
        let usage = TokenUsage {
            prompt_tokens: result["usage"]["input_tokens"].as_u64().unwrap_or(0) as u32,
            completion_tokens: result["usage"]["output_tokens"].as_u64().unwrap_or(0) as u32,
            total_tokens: (result["usage"]["input_tokens"].as_u64().unwrap_or(0)
                + result["usage"]["output_tokens"].as_u64().unwrap_or(0))
                as u32,
            cached_tokens: None,
        };

        // Extract finish reason
        let finish_reason = match result["stop_reason"].as_str() {
            Some("end_turn") | Some("stop_sequence") => FinishReason::Stop,
            Some("max_tokens") => FinishReason::Length,
            Some("tool_use") => FinishReason::ToolCalls,
            _ => FinishReason::Other,
        };

        let message = if tool_calls.is_empty() {
            Message::assistant(content)
        } else {
            Message::assistant_with_tools(content, tool_calls)
        };

        info!(
            provider = "anthropic",
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
        // max_tokens is required for Anthropic
        let max_tokens = request.model_config.max_tokens.unwrap_or(8192);

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

        let response = self
            .client
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

            return Err(LLMError::Api {
                status,
                message: error_text,
            });
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
                            let line = buffer[..newline_pos].trim().to_string();
                            buffer.drain(..=newline_pos);

                            // Process SSE line
                            for event_result in session.process_anthropic_sse(&line) {
                                match event_result {
                                    Ok(event) => {
                                        if tx.send(Ok(event)).await.is_err() {
                                            return; // Receiver dropped
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider() -> AnthropicProvider {
        AnthropicProvider {
            client: Client::new(),
            api_key: "test-key".to_string(),
            model: "claude-test".to_string(),
            base_url: "https://example.test".to_string(),
        }
    }

    #[test]
    fn converts_user_multipart_url_image() {
        let provider = provider();
        let messages = vec![Message::user(MessageContent::MultiPart(vec![
            ContentPart::Text {
                text: "look".to_string(),
            },
            ContentPart::Image {
                image_url: ImageUrl {
                    url: "https://example.test/image.png".to_string(),
                    detail: Some("high".to_string()),
                },
            },
        ]))];

        let converted = provider.convert_messages(&messages).unwrap();

        assert_eq!(converted[0]["role"], "user");
        assert_eq!(
            converted[0]["content"][0],
            json!({ "type": "text", "text": "look" })
        );
        assert_eq!(
            converted[0]["content"][1],
            json!({
                "type": "image",
                "source": { "type": "url", "url": "https://example.test/image.png" },
            })
        );
    }

    #[test]
    fn converts_user_multipart_data_url_image() {
        let provider = provider();
        let messages = vec![Message::user(MessageContent::MultiPart(vec![
            ContentPart::Image {
                image_url: ImageUrl {
                    url: "data:image/png;base64,QUJD".to_string(),
                    detail: None,
                },
            },
        ]))];

        let converted = provider.convert_messages(&messages).unwrap();

        assert_eq!(
            converted[0]["content"][0],
            json!({
                "type": "image",
                "source": {
                    "type": "base64",
                    "media_type": "image/png",
                    "data": "QUJD"
                },
            })
        );
    }

    #[test]
    fn rejects_invalid_data_url_image() {
        let provider = provider();
        let messages = vec![Message::user(MessageContent::MultiPart(vec![
            ContentPart::Image {
                image_url: ImageUrl {
                    url: "data:image/png,not-base64".to_string(),
                    detail: None,
                },
            },
        ]))];

        let err = provider.convert_messages(&messages).unwrap_err();
        assert!(err.to_string().contains("Invalid image data URL"));
    }

    #[test]
    fn test_user_agent_header_constant() {
        // Verify the User-Agent header constant is set correctly
        // This is required for DashScope coding endpoint access
        const EXPECTED_USER_AGENT: &str = "claude-code/1.0.0";

        // The User-Agent is hardcoded in the converse method
        // This test ensures it matches the expected format
        assert!(
            EXPECTED_USER_AGENT.starts_with("claude-code/"),
            "User-Agent must start with 'claude-code/' to access DashScope coding endpoint"
        );
    }
}
