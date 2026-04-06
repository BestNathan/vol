//! Anthropic Provider implementation.

use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use tracing::info;
use vol_llm_core::*;
use crate::LLMConfig;

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
        Ok(Self {
            client: Client::new(),
            api_key: config.api_key()?,
            model: config.model.clone(),
            base_url: config.endpoint.clone().unwrap_or_else(|| "https://api.anthropic.com".to_string()),
        })
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
                    let content = msg.content.as_ref()
                        .map(|c| c.as_str())
                        .unwrap_or("");
                    result.push(json!({
                        "role": "user",
                        "content": content,
                    }));
                }
                MessageRole::Assistant => {
                    let mut content = Vec::new();

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
        json!(tools.iter().map(|t| {
            json!({
                "name": t.name,
                "description": t.description,
                "input_schema": t.parameters.as_ref().unwrap_or(&json!({
                    "type": "object",
                    "properties": {}
                })),
            })
        }).collect::<Vec<_>>())
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
        let max_tokens = request.model_config.max_tokens.unwrap_or(1024);

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

        let response = self.client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
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

            return Err(LLMError::Api { status, message: error_text });
        }

        // Parse response
        let result: serde_json::Value = response.json().await?;

        // Extract content
        let content = result["content"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|item| item["text"].as_str())
            .unwrap_or("")
            .to_string();

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
            total_tokens: (
                result["usage"]["input_tokens"].as_u64().unwrap_or(0) +
                result["usage"]["output_tokens"].as_u64().unwrap_or(0)
            ) as u32,
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

    async fn converse_stream(&self, _request: ConversationRequest) -> Result<StreamReceiver> {
        Err(LLMError::Parse("Streaming not implemented".to_string()))
    }
}
