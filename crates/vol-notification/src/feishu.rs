//! Feishu/Lark notification handler using openlark for auth and reqwest for HTTP.
//!
//! Reference: https://github.com/foxzool/openlark

use vol_core::{NotificationHandler, Alert, Result, VolError, Tenor, OptionType};
use vol_config::FeishuConfig;
use tracing::{info, warn};

/// Feishu/Lark notification handler
#[derive(Clone)]
pub struct FeishuNotification {
    app_id: String,
    app_secret: String,
    receive_id: String,
    message_template: String,
    http_client: reqwest::Client,
}

impl FeishuNotification {
    pub fn new(config: FeishuConfig) -> Self {
        Self {
            app_id: config.app_id.unwrap_or_default(),
            app_secret: config.app_secret.unwrap_or_default(),
            receive_id: config.receive_id.unwrap_or_default(),
            message_template: config.message_template,
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("Failed to create HTTP client"),
        }
    }

    /// Get access token using Feishu OAuth 2.0 client credentials flow
    async fn get_access_token(&self) -> Option<String> {
        let url = "https://open.feishu.cn/open-apis/auth/v3/app_access_token/internal";

        let body = serde_json::json!({
            "app_id": self.app_id,
            "app_secret": self.app_secret
        });

        match self.http_client
            .post(url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
        {
            Ok(response) => {
                if response.status().is_success() {
                    match response.json::<serde_json::Value>().await {
                        Ok(json) => {
                            json.get("app_access_token")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string())
                        }
                        Err(e) => {
                            warn!("Failed to parse token response: {:?}", e);
                            None
                        }
                    }
                } else {
                    warn!("Failed to get access token: {}", response.status());
                    None
                }
            }
            Err(e) => {
                warn!("Failed to get access token: {:?}", e);
                None
            }
        }
    }

    /// Format message using template
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
            .replace("{mark_price_coin}", &format!("{:.4}", alert.mark_price_coin))
            .replace("{mark_price_usd}", &format!("{:.2}", alert.mark_price_usd()))
            .replace("{strike}", &alert.message)
    }

    /// Format alert as an interactive card (rich message)
    fn format_interactive_card(&self, alert: &Alert) -> String {
        let title = match &alert.alert_type {
            vol_core::AlertType::AbsoluteIv { .. } => "🚨 IV 阈值告警",
            vol_core::AlertType::RateChange { .. } => "📈 IV 快速变化告警",
            vol_core::AlertType::TermStructure { .. } => "📊 期限结构异常告警",
            vol_core::AlertType::Skew { .. } => "⚖️ Skew 偏离告警",
            vol_core::AlertType::PortfolioMargin { .. } => "💰 保证金告警",
            vol_core::AlertType::PortfolioBalance { .. } => "💵 余额告警",
            vol_core::AlertType::PortfolioDelta { .. } => "📉 Delta 告警",
            vol_core::AlertType::PortfolioPnL { .. } => "📊 P&L 告警",
            vol_core::AlertType::PortfolioGreek { greek, .. } => {
                match greek.as_str() {
                    "gamma" => "🔧 Gamma 告警",
                    "theta" => "⏰ Theta 告警",
                    "vega" => "📊 Vega 告警",
                    _ => "📈 Greek 告警",
                }
            }
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
                            "**合约**: {}\n**期限**: {} | **类型**: {}\n**IV**: {:.1}%\n**指数价格**: {:.2} USD\n**DTE**: {} 天\n**合约价格**: {:.4} {} ({:.2} USD)\n**实虚值**: {}",
                            alert.symbol,
                            tenor_cn,
                            option_type_cn,
                            alert.iv * 100.0,
                            alert.index_price,
                            alert.dte,
                            alert.mark_price_coin,
                            alert.symbol.split('-').next().unwrap_or("BTC").to_uppercase(),
                            alert.mark_price_usd(),
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

    /// Send message to Feishu API
    async fn send_message(&self, msg_type: &str, content: &str) -> Result<()> {
        let receive_id_type = if self.receive_id.starts_with("oc_") {
            "chat_id"
        } else if self.receive_id.starts_with("ou_") {
            "open_id"
        } else if self.receive_id.starts_with("og_") {
            "group_id"
        } else {
            "chat_id"
        };

        let token = self.get_access_token().await.ok_or_else(|| {
            VolError::Notification("Failed to get access token".to_string())
        })?;

        let body = serde_json::json!({
            "receive_id": self.receive_id,
            "msg_type": msg_type,
            "content": content,
            "receive_id_type": receive_id_type,
        });

        let url = "https://open.feishu.cn/open-apis/im/v1/messages";

        match self.http_client.post(url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
        {
            Ok(response) => {
                let status = response.status();
                let text = response.text().await.unwrap_or_default();

                if status.is_success() {
                    info!("Feishu message sent successfully");
                    Ok(())
                } else {
                    warn!("Feishu API error: {} - {}", status, &text[..200.min(text.len())]);
                    Err(VolError::Notification(format!("Feishu API error: {}", status)))
                }
            }
            Err(e) => {
                warn!("Failed to send Feishu message: {:?}", e);
                Err(VolError::Notification(format!("Network error: {}", e)))
            }
        }
    }
}

#[async_trait::async_trait]
impl NotificationHandler for FeishuNotification {
    fn name(&self) -> &str {
        "feishu"
    }

    async fn send(&self, alert: &Alert) -> Result<()> {
        // Try to send as interactive card first, fall back to text
        let card_content = self.format_interactive_card(alert);
        let text_content = self.format_message(alert);

        // Send as interactive card
        if let Err(e) = self.send_message("interactive_text", &card_content).await {
            warn!("Interactive card failed, falling back to text: {:?}", e);
            // Fall back to plain text
            self.send_message("text", &serde_json::json!({ "text": text_content }).to_string()).await?;
        }

        Ok(())
    }

    fn clone_box(&self) -> Box<dyn NotificationHandler> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vol_core::{AlertType, OptionType, Tenor};

    #[test]
    fn test_format_message_all_fields() {
        // Create a mock config
        let config = FeishuConfig {
            app_id: Some("test_app_id".to_string()),
            app_secret: Some("test_app_secret".to_string()),
            receive_id: Some("test_receive_id".to_string()),
            message_template: "{tenor} {alert_type} {symbol} IV={value} Index={index_price} DTE={dte} Type={option_type} Moneyness={moneyness} CoinPrice={mark_price_coin} UsdPrice={mark_price_usd}".to_string(),
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
            mark_price_coin: 0.0183,
        };

        // Test the template replacement by calling format_message
        let formatted = handler.format_message(&alert);

        // Verify all new fields are replaced correctly
        assert!(formatted.contains("85.0%"), "IV value should be formatted as percentage");
        assert!(formatted.contains("68500.50"), "index_price should be formatted with 2 decimals");
        assert!(formatted.contains("28"), "dte should be present");
        assert!(formatted.contains("C"), "option_type should be present");
        assert!(formatted.contains("ITM +2.0%"), "moneyness should be formatted as ITM/OTM with percentage");
        assert!(formatted.contains("0.0183"), "mark_price_coin should be present");
        assert!(formatted.contains("1253.56"), "mark_price_usd should be calculated (0.0183 * 68500.50)");

        // Verify original fields are also replaced
        assert!(formatted.contains("short"), "tenor should be present");
        assert!(formatted.contains("absolute_iv"), "alert_type should be present");
        assert!(formatted.contains("BTC-29MAR24-70000-C"), "symbol should be present");
    }
}
