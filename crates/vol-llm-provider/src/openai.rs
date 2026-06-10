//! OpenAI Provider implementation.

use crate::LLMConfig;
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::json;
use std::collections::HashMap;
use tokio::sync::mpsc;
use tracing::info;
use vol_llm_core::*;

/// OpenAI Provider
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
    /// Reads HTTPS_PROXY or https_proxy from environment if set.
    fn build_client() -> Result<Client> {
        let proxy_url = std::env::var("HTTPS_PROXY")
            .or_else(|_| std::env::var("https_proxy"))
            .ok();

        let mut builder = Client::builder().danger_accept_invalid_certs(true);

        if let Some(url) = &proxy_url {
            let no_proxy = reqwest::NoProxy::from_string("api.openai.com");
            let proxy = reqwest::Proxy::all(url)
                .map_err(|e| LLMError::Network(reqwest::Error::from(e).into()))?
                .no_proxy(no_proxy);
            builder = builder.proxy(proxy);
        }

        builder.build().map_err(|e| LLMError::Network(e.into()))
    }

    /// Convert messages to OpenAI format.
    /// System prompt is sent as the first message with role: "system".
    fn convert_messages(&self, messages: &[Message]) -> Vec<serde_json::Value> {
        messages
            .iter()
            .map(|msg| match msg.role {
                MessageRole::System => {
                    let content = msg.content.as_ref().map(|c| c.as_str()).unwrap_or("");
                    json!({
                        "role": "system",
                        "content": content,
                    })
                }
                MessageRole::User => {
                    let content = msg.content.as_ref().map(|c| c.as_str()).unwrap_or("");
                    json!({
                        "role": "user",
                        "content": content,
                    })
                }
                MessageRole::Assistant => {
                    let mut obj = serde_json::Map::new();
                    obj.insert("role".to_string(), json!("assistant"));

                    // Text content
                    if let Some(ref c) = msg.content {
                        obj.insert("content".to_string(), json!(c.as_str()));
                    } else {
                        obj.insert("content".to_string(), serde_json::Value::Null);
                    }

                    // Tool calls
                    if let Some(ref tools) = msg.tool_calls {
                        let tool_calls: Vec<_> = tools
                            .iter()
                            .map(|tool| {
                                json!({
                                    "id": tool.id,
                                    "type": "function",
                                    "function": {
                                        "name": tool.name,
                                        "arguments": tool.arguments,
                                    },
                                })
                            })
                            .collect();
                        obj.insert("tool_calls".to_string(), json!(tool_calls));
                    }

                    serde_json::Value::Object(obj)
                }
                MessageRole::Tool => {
                    let content = msg.content.as_ref().map(|c| c.as_str()).unwrap_or("");
                    let tool_call_id = msg.tool_call_id.as_deref().unwrap_or("");
                    json!({
                        "role": "tool",
                        "tool_call_id": tool_call_id,
                        "content": content,
                    })
                }
            })
            .collect()
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
        // Convert messages
        let openai_messages = self.convert_messages(&request.messages);

        // Build request body
        let mut body = json!({
            "model": self.model,
            "messages": openai_messages,
        });

        // Max tokens
        let max_tokens = request
            .model_config
            .max_tokens
            .or_else(|| {
                self.body_defaults
                    .get("max_tokens")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u32)
            })
            .unwrap_or(4096);
        body["max_tokens"] = json!(max_tokens);

        // Optional parameters
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

        // Apply body defaults (skip keys already set)
        for (key, value) in &self.body_defaults {
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
                "model" | "messages" | "tools" | "tool_choice" | "stream" | "stream_options"
                | "max_tokens" => true,
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
            .header("User-Agent", "claude-code/1.0.0")
            .json(&body);

        for (key, value) in &self.headers {
            req = req.header(key, value);
        }

        let response = req.send().await.map_err(LLMError::Network)?;

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

        // Extract content from choices[0].message.content
        let content = result["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        // Extract tool calls from choices[0].message.tool_calls
        let tool_calls = result["choices"][0]["message"]["tool_calls"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .map(|item| ToolCall {
                        id: item["id"].as_str().unwrap_or("").to_string(),
                        name: item["function"]["name"].as_str().unwrap_or("").to_string(),
                        arguments: item["function"]["arguments"].to_string(),
                        r#type: "function".to_string(),
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
        // Convert messages
        let openai_messages = self.convert_messages(&request.messages);

        // Build request body
        let mut body = json!({
            "model": self.model,
            "messages": openai_messages,
            "stream": true,
            "stream_options": {"include_usage": true},
        });

        // Max tokens
        let max_tokens = request
            .model_config
            .max_tokens
            .or_else(|| {
                self.body_defaults
                    .get("max_tokens")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u32)
            })
            .unwrap_or(4096);
        body["max_tokens"] = json!(max_tokens);

        // Optional parameters
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

        // Apply body defaults
        for (key, value) in &self.body_defaults {
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
                "model" | "messages" | "tools" | "tool_choice" | "stream" | "stream_options"
                | "max_tokens" => true,
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
            .header("User-Agent", "claude-code/1.0.0")
            .json(&body);

        for (key, value) in &self.headers {
            req = req.header(key, value);
        }

        let response = req.send().await.map_err(LLMError::Network)?;

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
        let parser = crate::openai_streaming::OpenaiStreamParser;

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
    use vol_llm_core::{LLMProvider, Message, ToolDefinition};

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
            description: Some("Get weather for a city".to_string()),
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

    #[test]
    fn test_body_defaults_merge_in_converse_body() {
        std::env::set_var("TEST_OPENAI_MERGE", "test-key");
        let mut body = HashMap::new();
        body.insert("temperature".to_string(), serde_json::json!(0.9));
        body.insert(
            "custom_param".to_string(),
            serde_json::json!("custom_value"),
        );

        let config = LLMConfig::with_env_key(
            LLMProvider::OpenAI,
            "gpt-4o",
            "TEST_OPENAI_MERGE",
            "https://api.openai.com",
        )
        .with_body(body);

        let provider = OpenaiProvider::new(&config).unwrap();

        // Verify body defaults are stored
        assert_eq!(
            provider.body_defaults.get("temperature").unwrap(),
            &serde_json::json!(0.9)
        );
        assert_eq!(
            provider.body_defaults.get("custom_param").unwrap(),
            &serde_json::json!("custom_value")
        );
    }
}
