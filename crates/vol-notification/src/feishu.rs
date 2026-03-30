//! Feishu/Lark notification handler using the new FeishuClient.
//!
//! Reference: https://open.feishu.cn/document/server-docs/api-call-guide/calling-process/get-access-token

use vol_core::{NotificationHandler, Alert, Result, VolError, Tenor};
use vol_feishu::FeishuClient;
use vol_config::FeishuConfig;
use tracing::{info, warn};

/// Feishu/Lark notification handler
pub struct FeishuNotification {
    client: FeishuClient,
    receive_id: String,
    message_template: String,
}

impl FeishuNotification {
    pub fn new(config: FeishuConfig) -> Self {
        let client = FeishuClient::new(
            config.app_id.unwrap_or_default(),
            config.app_secret.unwrap_or_default(),
        );

        Self {
            client,
            receive_id: config.receive_id.unwrap_or_default(),
            message_template: config.message_template,
        }
    }

    fn format_message(&self, alert: &Alert) -> String {
        self.message_template
            .replace("{tenor}", &alert.tenor.to_string())
            .replace("{alert_type}", &alert.alert_type.to_string())
            .replace("{symbol}", &alert.symbol)
            .replace("{value}", &format!("{:.1}%", alert.iv * 100.0))
            .replace("{strike}", &alert.message)
            .replace("{dte}", &alert.message)
    }

    /// Format alert as an interactive card (rich message)
    fn format_interactive_card(&self, alert: &Alert) -> String {
        let title = match &alert.alert_type {
            vol_core::AlertType::AbsoluteIv { .. } => "🚨 IV 阈值告警",
            vol_core::AlertType::RateChange { .. } => "📈 IV 快速变化告警",
            vol_core::AlertType::TermStructure { .. } => "📊 期限结构异常告警",
            vol_core::AlertType::Skew { .. } => "⚖️ Skew 偏离告警",
        };

        let tenor_cn = match alert.tenor {
            Tenor::Short => "短期",
            Tenor::Medium => "中期",
            Tenor::Long => "长期",
        };

        let option_type_cn = match alert.message.contains("C") {
            true => "Call",
            false => "Put",
        };

        serde_json::to_string(&serde_json::json!({
            "config": {
                "wide_screen_mode": true
            },
            "header": {
                "title": {
                    "tag": "plain_text",
                    "content": title
                },
                "template": "red"
            },
            "elements": [
                {
                    "tag": "div",
                    "text": {
                        "tag": "lark_md",
                        "content": format!(
                            "**合约**: {}\n**类型**: {} | **方向**: {}\n**IV**: {:.1}%",
                            alert.symbol,
                            tenor_cn,
                            option_type_cn,
                            alert.iv * 100.0
                        )
                    }
                },
                {
                    "tag": "hr"
                },
                {
                    "tag": "note",
                    "elements": [
                        {
                            "tag": "plain_text",
                            "content": "Deribit Volatility Monitor"
                        }
                    ]
                }
            ]
        })).unwrap_or_default()
    }
}

#[async_trait::async_trait]
impl NotificationHandler for FeishuNotification {
    fn name(&self) -> &str {
        "feishu"
    }

    async fn send(&self, alert: &Alert) -> Result<()> {
        // Format message
        let text_message = self.format_message(alert);
        let card_content = self.format_interactive_card(alert);

        // Try to send interactive card first (richer experience)
        match self.client.send_interactive_card(&self.receive_id, &card_content).await {
            Ok(response) => {
                info!("Feishu card message sent: {}", response.data.message_id);
                return Ok(());
            }
            Err(e) => {
                warn!("Failed to send Feishu card message: {:?}, falling back to text", e);
            }
        }

        // Fallback to simple text message
        match self.client.send_text(&self.receive_id, &text_message).await {
            Ok(response) => {
                info!("Feishu text message sent: {}", response.data.message_id);
                Ok(())
            }
            Err(_e) => Err(VolError::Notification(
                "Feishu notification failed".to_string()
            )),
        }
    }
}
