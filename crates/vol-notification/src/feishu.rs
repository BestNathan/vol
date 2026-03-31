//! Feishu/Lark notification handler using the new FeishuClient.
//!
//! Reference: https://open.feishu.cn/document/server-docs/api-call-guide/calling-process/get-access-token

use vol_core::{NotificationHandler, Alert, Result, VolError, Tenor, OptionType};
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
            .replace("{index_price}", &format!("{:.2}", alert.index_price))
            .replace("{dte}", &alert.dte.to_string())
            .replace("{option_type}", &alert.option_type.to_string())
            .replace("{moneyness}", &format!(
                "{}{:.1}%",
                if alert.moneyness > 0.0 { "ITM +" } else { "OTM " },
                alert.moneyness.abs() * 100.0
            ))
            .replace("{mark_price}", &format!("{:.2}", alert.mark_price))
            .replace("{strike}", &alert.message)
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

        let option_type_cn = match alert.option_type {
            OptionType::Call => "Call",
            OptionType::Put => "Put",
        };

        let moneyness_str = if alert.moneyness > 0.0 {
            format!("ITM +{:.1}%", alert.moneyness * 100.0)
        } else {
            format!("OTM {:.1}%", alert.moneyness * 100.0)
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
                            "**合约**: {}\n**期限**: {} | **类型**: {}\n**IV**: {:.1}%\n**指数价格**: {:.2} USD\n**DTE**: {} 天\n**合约价格**: {:.2} USD\n**实虚值**: {}",
                            alert.symbol,
                            tenor_cn,
                            option_type_cn,
                            alert.iv * 100.0,
                            alert.index_price,
                            alert.dte,
                            alert.mark_price,
                            moneyness_str
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

#[cfg(test)]
mod tests {
    use super::*;
    use vol_core::{AlertType, OptionType};

    #[test]
    fn test_format_message_all_fields() {
        // Create a mock config
        let config = FeishuConfig {
            app_id: Some("test_app_id".to_string()),
            app_secret: Some("test_app_secret".to_string()),
            receive_id: Some("test_receive_id".to_string()),
            message_template: "{tenor} {alert_type} {symbol} IV={value} Index={index_price} DTE={dte} Type={option_type} Moneyness={moneyness} Price={mark_price}".to_string(),
        };

        let handler = FeishuNotification::new(config);

        // Create a test alert with all fields populated
        let alert = Alert {
            alert_type: AlertType::AbsoluteIv { threshold: 80.0 },
            tenor: Tenor::Short,
            symbol: "BTC-29MAR24-70000-C".to_string(),
            iv: 0.85,
            message: "IV exceeded threshold".to_string(),
            timestamp: 1234567890,
            source: "deribit".to_string(),
            index_price: 68500.50,
            dte: 28,
            option_type: OptionType::Call,
            moneyness: 0.02,
            mark_price: 1250.75,
        };

        // Use reflection or access the field directly for testing
        // Since format_message is private, we test via send() or make it pub(crate)
        // For unit test, we can access via a helper or test the output indirectly

        // Test the template replacement by calling format_message indirectly
        // We'll make format_message public for testing or test the actual output
        let formatted = handler.format_message(&alert);

        // Verify all 5 new fields are replaced correctly
        assert!(formatted.contains("85.0%"), "IV value should be formatted as percentage");
        assert!(formatted.contains("68500.50"), "index_price should be formatted with 2 decimals");
        assert!(formatted.contains("28"), "dte should be present");
        assert!(formatted.contains("C"), "option_type should be present");
        assert!(formatted.contains("ITM +2.0%"), "moneyness should be formatted as ITM/OTM with percentage");
        assert!(formatted.contains("1250.75"), "mark_price should be formatted with 2 decimals");

        // Verify original fields are also replaced
        assert!(formatted.contains("short"), "tenor should be present");
        assert!(formatted.contains("absolute_iv"), "alert_type should be present");
        assert!(formatted.contains("BTC-29MAR24-70000-C"), "symbol should be present");
    }
}
