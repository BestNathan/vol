//! Notification configuration types.

use serde::{Deserialize, Serialize};

/// Stdout notification configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StdoutNotificationConfig {
    pub id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// Feishu/Lark notification configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FeishuNotificationConfig {
    pub id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub app_id: String,
    pub app_secret: String,
    pub receive_id: String,
    #[serde(default = "default_template")]
    pub message_template: String,
}

/// Feishu config for backwards compatibility with vol-notification
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FeishuConfig {
    /// App ID for OAuth 2.0 authentication
    #[serde(default)]
    pub app_id: Option<String>,
    /// App Secret for OAuth 2.0 authentication
    #[serde(default)]
    pub app_secret: Option<String>,
    /// Receive ID (chat_id or user_id)
    #[serde(default)]
    pub receive_id: Option<String>,
    /// Message template for text notifications
    #[serde(default = "default_message_template")]
    pub message_template: String,
}

fn default_message_template() -> String {
    "🚨 {tenor} {alert_type}: {symbol} | IV={value:.1}% | 指数={index_price} | DTE={dte}天 | {option_type} | 价格={mark_price_coin} ({mark_price_usd} USD)".to_string()
}

/// Notification configuration enum
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum NotificationConfig {
    Stdout(StdoutNotificationConfig),
    Feishu(FeishuNotificationConfig),
}

impl NotificationConfig {
    pub fn id(&self) -> &str {
        match self {
            NotificationConfig::Stdout(c) => &c.id,
            NotificationConfig::Feishu(c) => &c.id,
        }
    }

    pub fn enabled(&self) -> bool {
        match self {
            NotificationConfig::Stdout(c) => c.enabled,
            NotificationConfig::Feishu(c) => c.enabled,
        }
    }
}

fn default_true() -> bool { true }
fn default_template() -> String {
    "🚨 {tenor} {alert_type}: {symbol} | IV={value:.1}% | 指数={index_price} | DTE={dte}天 | {option_type} | 价格={mark_price_coin} ({mark_price_usd} USD)".to_string()
}
