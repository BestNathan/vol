//! Feishu/Lark API client.
//!
//! Implements OAuth 2.0 authentication and message sending.
//! Reference: https://open.feishu.cn/document/server-docs/api-call-guide/calling-process/get-access-token

use std::sync::Arc;
use std::time::{Duration, Instant};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::info;

/// Feishu API base URLs
pub const FEISHU_BASE_URL: &str = "https://open.feishu.cn";
pub const FEISHU_TOKEN_PATH: &str = "/open-apis/auth/v3/app_access_token/internal";
pub const FEISHU_MESSAGE_PATH: &str = "/open-apis/im/v1/messages";

/// App Access Token response
/// Feishu API returns flat structure: {code, msg, app_access_token, expire, tenant_access_token}
#[derive(Debug, Deserialize, Clone, Default)]
pub struct AppAccessTokenResponse {
    pub code: i64,
    pub msg: String,
    #[serde(default)]
    pub expire: u64,
    #[serde(default)]
    pub app_access_token: String,
    #[serde(default)]
    pub tenant_access_token: String,
}

/// Send message request
#[derive(Debug, Serialize)]
pub struct SendMessageRequest {
    pub receive_id: String,
    pub msg_type: String,
    pub content: String,
}

/// Send message response
#[derive(Debug, Deserialize)]
pub struct SendMessageResponse {
    pub code: i64,
    pub msg: String,
    #[serde(default)]
    pub data: MessageData,
}

#[derive(Debug, Deserialize, Default)]
pub struct MessageData {
    pub message_id: String,
}

/// Internal state for Feishu client
struct FeishuClientState {
    access_token: Option<String>,
    token_expire_time: Option<Instant>,
}

/// Feishu API client
#[derive(Clone)]
pub struct FeishuClient {
    client: Client,
    app_id: String,
    app_secret: String,
    state: Arc<Mutex<FeishuClientState>>,
    base_url: String,
}

impl FeishuClient {
    /// Create a new Feishu client
    pub fn new(app_id: String, app_secret: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
            app_id,
            app_secret,
            state: Arc::new(Mutex::new(FeishuClientState {
                access_token: None,
                token_expire_time: None,
            })),
            base_url: FEISHU_BASE_URL.to_string(),
        }
    }

    /// Create with custom base URL
    pub fn with_base_url(mut self, base_url: String) -> Self {
        self.base_url = base_url;
        self
    }

    /// Get app access token, caching to avoid frequent requests
    pub async fn get_access_token(&self) -> Result<String, FeishuError> {
        // Check if token is still valid (refresh 30s before expiry)
        {
            let state = self.state.lock().await;
            if let Some(expire_time) = state.token_expire_time {
                if expire_time > Instant::now() + Duration::from_secs(30) {
                    if let Some(ref token) = state.access_token {
                        info!("Using cached Feishu access token");
                        return Ok(token.clone());
                    }
                }
            }
        }

        // Request new token
        let url = format!("{}{}", self.base_url, FEISHU_TOKEN_PATH);

        let body = serde_json::json!({
            "app_id": self.app_id,
            "app_secret": self.app_secret,
        });

        let response = self.client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| FeishuError::Network(format!("Failed to request token: {}", e)))?;

        let result: AppAccessTokenResponse = response
            .json()
            .await
            .map_err(|e| FeishuError::Parse(format!("Failed to parse token response: {}", e)))?;

        if result.code != 0 {
            return Err(FeishuError::Api(format!("Token request failed: {} (code: {})", result.msg, result.code)));
        }

        let token = result.app_access_token.clone();
        let tenant_token = result.tenant_access_token.clone();
        let expire_secs = result.expire;

        // Cache token - use tenant_access_token for messaging APIs
        {
            let mut state = self.state.lock().await;
            // Prefer tenant_access_token if available, otherwise use app_access_token
            let cached_token = if tenant_token.is_empty() { token.clone() } else { tenant_token.clone() };
            state.access_token = Some(cached_token);
            state.token_expire_time = Some(Instant::now() + Duration::from_secs(expire_secs.saturating_sub(30)));
        }

        info!("Feishu access token cached, expires_in={}s", expire_secs);

        let state = self.state.lock().await;
        Ok(state.access_token.clone().unwrap())
    }

    /// Send a message to a chat
    pub async fn send_message(
        &self,
        receive_id: &str,
        msg_type: &str,
        content: &str,
    ) -> Result<SendMessageResponse, FeishuError> {
        // Determine receive_id_type based on receive_id prefix
        let receive_id_type = if receive_id.starts_with("oc_") {
            "chat_id"
        } else if receive_id.starts_with("ou_") {
            "open_id"
        } else if receive_id.starts_with("og_") {
            "group_id"
        } else {
            "chat_id" // default
        };

        let url = format!("{}{}?receive_id_type={}", self.base_url, FEISHU_MESSAGE_PATH, receive_id_type);

        // Get access token
        let token = self.get_access_token().await?;

        let body = SendMessageRequest {
            receive_id: receive_id.to_string(),
            msg_type: msg_type.to_string(),
            content: content.to_string(),
        };

        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| FeishuError::Network(format!("Failed to send message: {}", e)))?;

        let status = response.status();
        let text = response.text().await
            .map_err(|e| FeishuError::Parse(format!("Failed to read message response: {}", e)))?;
        info!("Message response: {} - {}", status, &text[..200.min(text.len())]);

        let result: SendMessageResponse = serde_json::from_str(&text)
            .map_err(|e| FeishuError::Parse(format!("Failed to parse message response: {}", e)))?;

        if result.code != 0 {
            return Err(FeishuError::Api(format!(
                "Message send failed: {} (code: {})",
                result.msg, result.code
            )));
        }

        Ok(result)
    }

    /// Send a text message with simple content
    pub async fn send_text(&self, receive_id: &str, text: &str) -> Result<SendMessageResponse, FeishuError> {
        let content = serde_json::json!({
            "text": text
        });

        self.send_message(receive_id, "text", &content.to_string()).await
    }

    /// Send an interactive card message
    pub async fn send_interactive_card(
        &self,
        receive_id: &str,
        card_content: &str,
    ) -> Result<SendMessageResponse, FeishuError> {
        self.send_message(receive_id, "interactive", card_content).await
    }
}

/// Feishu API error types
#[derive(Debug, thiserror::Error)]
pub enum FeishuError {
    #[error("Network error: {0}")]
    Network(String),
    #[error("API error: {0}")]
    Api(String),
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Token not available")]
    TokenNotAvailable,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_request_serialization() {
        let req = SendMessageRequest {
            receive_id: "test_chat_id".to_string(),
            msg_type: "text".to_string(),
            content: r#"{"text":"Hello"}"#.to_string(),
        };

        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("test_chat_id"));
        assert!(json.contains("text"));
    }
}
