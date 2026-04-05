//! Feishu/Lark notification handler using openlark SDK.
//!
//! Reference: https://github.com/foxzool/openlark

use vol_core::{NotificationHandler, Alert, Result, VolError, Tenor, OptionType};
use vol_config::FeishuConfig;
use open_lark::Client;
use open_lark::communication::im::im::v1::message::create::{CreateMessageRequest, CreateMessageBody};
use open_lark::communication::im::im::v1::message::models::ReceiveIdType;
use serde_json::json;
use tracing::{info, warn, info_span};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use opentelemetry::trace::TraceContextExt;
use opentelemetry::Context;

/// Feishu/Lark notification handler
#[derive(Clone)]
pub struct FeishuNotification {
    client: Client,
    receive_id: String,
    receive_id_type: ReceiveIdType,
    message_template: String,
}

impl FeishuNotification {
    pub fn new(config: FeishuConfig) -> Result<Self> {
        let app_id = config.app_id.ok_or_else(|| {
            VolError::Notification("Feishu app_id is required".to_string())
        })?;

        let app_secret = config.app_secret.ok_or_else(|| {
            VolError::Notification("Feishu app_secret is required".to_string())
        })?;

        let receive_id = config.receive_id.ok_or_else(|| {
            VolError::Notification("Feishu receive_id is required".to_string())
        })?;

        // Determine receive_id_type based on prefix
        // oc_ -> chat_id, ou_ -> open_id, og_ -> chat_id (group chat)
        let receive_id_type = if receive_id.starts_with("oc_") {
            ReceiveIdType::ChatId
        } else if receive_id.starts_with("ou_") {
            ReceiveIdType::OpenId
        } else if receive_id.starts_with("og_") {
            ReceiveIdType::ChatId
        } else {
            // Default to chat_id for backwards compatibility
            ReceiveIdType::ChatId
        };

        // Create openlark client with app credentials
        let client = Client::builder()
            .app_id(&app_id)
            .app_secret(&app_secret)
            .build()
            .map_err(|e| VolError::Notification(format!("Failed to create openlark client: {}", e)))?;

        Ok(Self {
            client,
            receive_id,
            receive_id_type,
            message_template: config.message_template,
        })
    }

    /// Format message using template
    fn format_message(&self, alert: &Alert, trace_id_prefix: &str) -> String {
        let formatted = self.message_template
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
            .replace("{strike}", &alert.message);

        // Prepend trace_id prefix
        format!("{} {}", trace_id_prefix, formatted)
    }

    /// Format alert as an interactive card (rich message)
    fn format_interactive_card(&self, alert: &Alert, trace_id_prefix: &str) -> String {
        let (title, content) = match &alert.alert_type {
            vol_core::AlertType::AbsoluteIv { .. } => (format!("{} 🚨 IV 阈值告警", trace_id_prefix), format!(
                "**合约**: {}\n**期限**: {}\n**类型**: {}\n**IV**: {:.1}%\n**指数价格**: {:.2} USD\n**DTE**: {} 天\n**合约价格**: {:.4} {} ({:.2} USD)\n**实虚值**: {}",
                alert.symbol, self.tenor_cn(alert.tenor), self.option_type_cn(alert.option_type),
                alert.iv * 100.0, alert.index_price, alert.dte,
                alert.mark_price_coin, alert.symbol.split('-').next().unwrap_or("BTC").to_uppercase(),
                alert.mark_price_usd(), self.moneyness_str(alert.moneyness)
            )),
            vol_core::AlertType::RateChange { .. } => (format!("{} 📈 IV 快速变化告警", trace_id_prefix), format!(
                "**合约**: {}\n**期限**: {}\n**类型**: {}\n**IV**: {:.1}%\n**指数价格**: {:.2} USD\n**DTE**: {} 天\n**合约价格**: {:.4} {} ({:.2} USD)\n**实虚值**: {}",
                alert.symbol, self.tenor_cn(alert.tenor), self.option_type_cn(alert.option_type),
                alert.iv * 100.0, alert.index_price, alert.dte,
                alert.mark_price_coin, alert.symbol.split('-').next().unwrap_or("BTC").to_uppercase(),
                alert.mark_price_usd(), self.moneyness_str(alert.moneyness)
            )),
            vol_core::AlertType::TermStructure { .. } => (format!("{} 📊 期限结构异常告警", trace_id_prefix), format!(
                "**策略**: {}\n**期限**: {}\n**类型**: {}\n**IV Spread**: {:.1}%\n**指数价格**: {:.2} USD",
                alert.symbol, self.tenor_cn(alert.tenor), self.option_type_cn(alert.option_type),
                alert.iv * 100.0, alert.index_price
            )),
            vol_core::AlertType::Skew { .. } => (format!("{} ⚖️ Skew 偏离告警", trace_id_prefix), format!(
                "**标的**: {}\n**期限**: {}\n**Skew**: {:.1}%\n**指数价格**: {:.2} USD",
                alert.symbol, self.tenor_cn(alert.tenor), alert.iv * 100.0, alert.index_price
            )),
            vol_core::AlertType::PortfolioMargin { current, threshold } => (format!("{} 💰 保证金比率告警", trace_id_prefix), format!(
                "**账户**: PORTFOLIO_{}\n**保证金比率**: {:.2}\n**阈值**: {:.2}\n**可用资金**: {:.4}\n**初始保证金**: {:.4}\n**维持保证金**: {:.4}",
                alert.symbol.replace("PORTFOLIO_", ""), current, threshold,
                alert.mark_price_coin, alert.index_price, alert.moneyness
            )),
            vol_core::AlertType::PortfolioBalance { current, threshold } => (format!("{} 💵 余额告警", trace_id_prefix), format!(
                "**账户**: PORTFOLIO_{}\n**可用余额**: {:.4}\n**阈值**: {:.4}\n**总权益**: {:.4}",
                alert.symbol.replace("PORTFOLIO_", ""), current, threshold, alert.index_price
            )),
            vol_core::AlertType::PortfolioDelta { current } => (format!("{} 📉 Delta 敞口告警", trace_id_prefix), format!(
                "**账户**: PORTFOLIO_{}\n**总 Delta**: {:.2}\n**指数价格**: {:.2} USD",
                alert.symbol.replace("PORTFOLIO_", ""), current, alert.index_price
            )),
            vol_core::AlertType::PortfolioPnL { current, threshold } => (format!("{} 📊 P&L 告警", trace_id_prefix), format!(
                "**账户**: PORTFOLIO_{}\n**Session PnL**: {:.4}\n**阈值**: {:.4}",
                alert.symbol.replace("PORTFOLIO_", ""), current, threshold
            )),
            vol_core::AlertType::PortfolioGreek { greek, current, threshold } => {
                let greek_name = match greek.as_str() {
                    "gamma" => "Gamma",
                    "theta" => "Theta",
                    "vega" => "Vega",
                    _ => "Greek",
                };
                (format!("{} 📈 Greek 告警", trace_id_prefix), format!(
                    "**账户**: PORTFOLIO_{}\n**{}**: {:.6}\n**阈值**: {:.6}",
                    alert.symbol.replace("PORTFOLIO_", ""), greek_name, current, threshold
                ))
            }
        };

        serde_json::to_string(&json!({
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
                        "content": content
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

    /// Helper function for tenor Chinese name
    fn tenor_cn(&self, tenor: Tenor) -> &'static str {
        match tenor {
            Tenor::Short => "短期",
            Tenor::Medium => "中期",
            Tenor::Long => "长期",
        }
    }

    /// Helper function for option type Chinese name
    fn option_type_cn(&self, option_type: OptionType) -> &'static str {
        match option_type {
            OptionType::Call => "Call",
            OptionType::Put => "Put",
        }
    }

    /// Helper function for moneyness string
    fn moneyness_str(&self, moneyness: f64) -> String {
        if moneyness > 0.0 {
            format!("ITM +{:.1}%", moneyness * 100.0)
        } else {
            format!("OTM {:.1}%", moneyness * 100.0)
        }
    }

    /// Send message to Feishu API using openlark SDK
    async fn send_message(&self, msg_type: &str, content: &str) -> Result<()> {
        // Build the message request body
        let body = CreateMessageBody {
            receive_id: self.receive_id.clone(),
            msg_type: msg_type.to_string(),
            content: content.to_string(),
            uuid: None,
        };

        // Create the request with the appropriate receive_id_type
        let request = CreateMessageRequest::new(self.client.core_config().clone())
            .receive_id_type(self.receive_id_type);

        // Log request details for debugging
        info!("Sending Feishu message: receive_id={}, receive_id_type={:?}, msg_type={}",
              self.receive_id, self.receive_id_type, msg_type);

        // Send the message
        let result: open_lark::SDKResult<serde_json::Value> = request.execute(body).await;

        match result {
            Ok(response) => {
                info!("Feishu API raw response: {:?}", response);
                // Check if the response contains a message_id (success indicator)
                // openlark SDK returns Feishu API response with fields at root level:
                // {"body": {...}, "message_id": "...", "chat_id": "...", ...}
                let response: &serde_json::Value = &response;

                // Try to find message_id at root level (openlark SDK format)
                let message_id = response
                    .get("message_id")
                    .and_then(|v| v.as_str());

                if let Some(message_id) = message_id {
                    info!("Feishu message sent successfully: {}", message_id);
                    return Ok(());
                }

                // Legacy format check (Feishu API v2 with data wrapper)
                if let Some(data) = response.get("data") {
                    if let Some(message_id) = data.get("message_id").and_then(|v| v.as_str()) {
                        info!("Feishu message sent successfully: {}", message_id);
                        return Ok(());
                    }
                }

                // Fallback: check code field (older API format)
                let code = response.get("code").and_then(|v| v.as_i64()).unwrap_or(-1);
                if code == 0 {
                    info!("Feishu message sent successfully");
                    Ok(())
                } else {
                    let msg = response.get("msg").and_then(|v| v.as_str()).unwrap_or("Unknown error");
                    warn!("Feishu API error: code={}, msg={}", code, msg);
                    Err(VolError::Notification(format!("Feishu API error: {} - {}", code, msg)))
                }
            }
            Err(e) => {
                warn!("Failed to send Feishu message: {:?}", e);
                Err(VolError::Notification(format!("Failed to send message: {}", e)))
            }
        }
    }
}

/// Extract trace_id from current span context and return short prefix like [tr_abc1234]
fn get_trace_id_prefix() -> String {
    // Get the current OpenTelemetry context
    let context = Context::current();

    // Extract span from context
    let span = context.span();
    let span_context = span.span_context();

    // Get trace_id
    let trace_id = span_context.trace_id().to_string();

    // Shorten to 8 chars like tr_abc1234
    if trace_id.len() >= 8 {
        format!("[tr_{}]", &trace_id[..8])
    } else {
        format!("[tr_{}]", trace_id)
    }
}

    #[async_trait::async_trait]
impl NotificationHandler for FeishuNotification {
    fn name(&self) -> &str {
        "feishu"
    }

    async fn send(&self, alert: &Alert) -> Result<()> {
        // Create span for notification with business attributes
        let span = info_span!(
            "notification_send",
            channel = "feishu",
            alert_type = %alert.alert_type,
            tenor = ?alert.tenor,
            symbol = %alert.symbol,
            iv = %alert.iv
        );

        // Record additional alert attributes
        span.record("alert.dte", &alert.dte);
        span.record("alert.index_price", &alert.index_price);

        let _guard = span.enter();

        // Extract trace_id from current span context for reverse tracing
        let trace_id_prefix = get_trace_id_prefix();

        // Try to send as interactive card first, fall back to text
        let card_content = self.format_interactive_card(alert, &trace_id_prefix);
        let text_content = self.format_message(alert, &trace_id_prefix);

        // Send as interactive card (msg_type must be "interactive" per Feishu API docs)
        if let Err(e) = self.send_message("interactive", &card_content).await {
            warn!("Interactive card failed, falling back to text: {:?}", e);
            // Fall back to plain text
            self.send_message("text", &json!({ "text": text_content }).to_string()).await?;
        }

        // Extract current trace_id for logging
        let trace_id = tracing::Span::current()
            .context()
            .span()
            .span_context()
            .trace_id();

        tracing::info!(
            trace_id = %trace_id,
            recipient = %self.receive_id,
            "notification sent"
        );

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

        let handler = FeishuNotification::new(config).unwrap();

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
        let formatted = handler.format_message(&alert, "[tr_test]");

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

    #[test]
    fn test_receive_id_type_mapping() {
        // Test oc_ prefix -> ChatId
        let config_oc = FeishuConfig {
            app_id: Some("test".to_string()),
            app_secret: Some("test".to_string()),
            receive_id: Some("oc_c29208d94757e2aefd97bfa5f57e0b26".to_string()),
            message_template: "test".to_string(),
        };
        let handler_oc = FeishuNotification::new(config_oc).unwrap();
        assert!(matches!(handler_oc.receive_id_type, ReceiveIdType::ChatId));

        // Test ou_ prefix -> OpenId
        let config_ou = FeishuConfig {
            app_id: Some("test".to_string()),
            app_secret: Some("test".to_string()),
            receive_id: Some("ou_xxxxxxxxxxxx".to_string()),
            message_template: "test".to_string(),
        };
        let handler_ou = FeishuNotification::new(config_ou).unwrap();
        assert!(matches!(handler_ou.receive_id_type, ReceiveIdType::OpenId));
    }
}
