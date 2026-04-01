//! Portfolio rule with configurable metrics.

use vol_core::{Alert, Tenor, OptionType, AlertType, RuleProcessor, MonitoringEvent, EventType, RuleAction, Result};
use vol_config::metrics::MetricConfig;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Portfolio data snapshot for alert evaluation
#[derive(Debug, Clone)]
pub struct PortfolioSnapshot {
    pub currency: String,
    pub timestamp: u64,
    pub margin_ratio: Option<f64>,
    pub free_balance: f64,
    pub delta_exposure: f64,
    pub session_pnl: f64,
    pub options_gamma: f64,
    pub options_vega: f64,
    pub options_theta: f64,
}

/// Portfolio rule with configurable metrics
pub struct PortfolioRule {
    metrics: Arc<RwLock<Vec<MetricConfig>>>,
    cooldown_secs: u64,
    last_alert: Arc<RwLock<std::collections::HashMap<String, u64>>>,
}

impl PortfolioRule {
    pub fn new(metrics: Vec<MetricConfig>, cooldown_secs: u64) -> Self {
        Self {
            metrics: Arc::new(RwLock::new(metrics)),
            cooldown_secs,
            last_alert: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// Update metrics configuration
    pub async fn update_metrics(&self, metrics: Vec<MetricConfig>) {
        let mut self_metrics = self.metrics.write().await;
        *self_metrics = metrics;
    }

    /// Check if alert is in cooldown
    async fn in_cooldown(&self, key: &str) -> bool {
        let last = self.last_alert.read().await;
        if let Some(&timestamp) = last.get(key) {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            return now - timestamp < self.cooldown_secs;
        }
        false
    }

    /// Record alert timestamp
    async fn record_alert(&self, key: String) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let mut last = self.last_alert.write().await;
        last.insert(key, now);
    }

    /// Evaluate snapshot against configured metrics
    pub async fn evaluate(&self, snapshot: &PortfolioSnapshot) -> Vec<Alert> {
        let mut alerts = Vec::new();
        let metrics = self.metrics.read().await;

        for metric in metrics.iter() {
            if !metric.enabled() {
                continue;
            }

            match metric {
                MetricConfig::MarginRatio(cfg) => {
                    if let Some(ratio) = snapshot.margin_ratio {
                        if let Some(min) = cfg.min_threshold {
                            if ratio < min {
                                let key = format!("margin_ratio_{}", snapshot.currency);
                                if !self.in_cooldown(&key).await {
                                    alerts.push(Alert {
                                        alert_type: AlertType::PortfolioMargin {
                                            current: ratio,
                                            threshold: min
                                        },
                                        tenor: Tenor::Medium,
                                        symbol: format!("PORTFOLIO_{}", snapshot.currency),
                                        iv: 0.0,
                                        message: format!("Margin ratio {:.2} below threshold {:.2}", ratio, min),
                                        timestamp: snapshot.timestamp,
                                        source: "deribit".to_string(),
                                        index_price: 0.0,
                                        dte: 0,
                                        option_type: OptionType::Call,
                                        moneyness: 0.0,
                                        mark_price_coin: snapshot.free_balance,
                                    });
                                    self.record_alert(key).await;
                                }
                            }
                        }
                    }
                }
                MetricConfig::FreeBalance(cfg) => {
                    if let Some(min) = cfg.min_threshold {
                        if snapshot.free_balance < min {
                            let key = format!("free_balance_{}", snapshot.currency);
                            if !self.in_cooldown(&key).await {
                                alerts.push(Alert {
                                    alert_type: AlertType::PortfolioBalance {
                                        current: snapshot.free_balance,
                                        threshold: min
                                    },
                                    tenor: Tenor::Medium,
                                    symbol: format!("PORTFOLIO_{}", snapshot.currency),
                                    iv: 0.0,
                                    message: format!("Free balance {:.2} below threshold {:.2}", snapshot.free_balance, min),
                                    timestamp: snapshot.timestamp,
                                    source: "deribit".to_string(),
                                    index_price: 0.0,
                                    dte: 0,
                                    option_type: OptionType::Call,
                                    moneyness: 0.0,
                                    mark_price_coin: snapshot.free_balance,
                                });
                                self.record_alert(key).await;
                            }
                        }
                    }
                }
                MetricConfig::DeltaExposure(cfg) => {
                    let delta = snapshot.delta_exposure;
                    let triggered = cfg.min_threshold.map(|min| delta < min).unwrap_or(false)
                        || cfg.max_threshold.map(|max| delta > max).unwrap_or(false);

                    if triggered {
                        let key = format!("delta_exposure_{}", snapshot.currency);
                        if !self.in_cooldown(&key).await {
                            alerts.push(Alert {
                                alert_type: AlertType::PortfolioDelta {
                                    current: delta
                                },
                                tenor: Tenor::Medium,
                                symbol: format!("PORTFOLIO_{}", snapshot.currency),
                                iv: 0.0,
                                message: format!("Delta exposure {:.2} outside thresholds", delta),
                                timestamp: snapshot.timestamp,
                                source: "deribit".to_string(),
                                index_price: 0.0,
                                dte: 0,
                                option_type: OptionType::Call,
                                moneyness: 0.0,
                                mark_price_coin: 0.0,
                            });
                            self.record_alert(key).await;
                        }
                    }
                }
                MetricConfig::SessionPnl(cfg) => {
                    if let Some(max) = cfg.max_threshold {
                        if snapshot.session_pnl < max {
                            let key = format!("session_pnl_{}", snapshot.currency);
                            if !self.in_cooldown(&key).await {
                                alerts.push(Alert {
                                    alert_type: AlertType::PortfolioPnL {
                                        current: snapshot.session_pnl,
                                        threshold: max
                                    },
                                    tenor: Tenor::Medium,
                                    symbol: format!("PORTFOLIO_{}", snapshot.currency),
                                    iv: 0.0,
                                    message: format!("Session PnL {:.2} below threshold {:.2}", snapshot.session_pnl, max),
                                    timestamp: snapshot.timestamp,
                                    source: "deribit".to_string(),
                                    index_price: 0.0,
                                    dte: 0,
                                    option_type: OptionType::Call,
                                    moneyness: 0.0,
                                    mark_price_coin: 0.0,
                                });
                                self.record_alert(key).await;
                            }
                        }
                    }
                }
                MetricConfig::TotalGreeks(cfg) => {
                    // Check gamma
                    if let Some(threshold) = cfg.gamma_threshold {
                        if snapshot.options_gamma.abs() > threshold {
                            let key = format!("gamma_{}", snapshot.currency);
                            if !self.in_cooldown(&key).await {
                                alerts.push(Alert {
                                    alert_type: AlertType::PortfolioGreek {
                                        greek: "gamma".to_string(),
                                        current: snapshot.options_gamma,
                                        threshold
                                    },
                                    tenor: Tenor::Medium,
                                    symbol: format!("PORTFOLIO_{}", snapshot.currency),
                                    iv: 0.0,
                                    message: format!("Gamma {:.6} exceeds threshold {:.6}", snapshot.options_gamma, threshold),
                                    timestamp: snapshot.timestamp,
                                    source: "deribit".to_string(),
                                    index_price: 0.0,
                                    dte: 0,
                                    option_type: OptionType::Call,
                                    moneyness: 0.0,
                                    mark_price_coin: 0.0,
                                });
                                self.record_alert(key).await;
                            }
                        }
                    }
                    // Check vega
                    if let Some(threshold) = cfg.vega_threshold {
                        if snapshot.options_vega.abs() > threshold {
                            let key = format!("vega_{}", snapshot.currency);
                            if !self.in_cooldown(&key).await {
                                alerts.push(Alert {
                                    alert_type: AlertType::PortfolioGreek {
                                        greek: "vega".to_string(),
                                        current: snapshot.options_vega,
                                        threshold
                                    },
                                    tenor: Tenor::Medium,
                                    symbol: format!("PORTFOLIO_{}", snapshot.currency),
                                    iv: 0.0,
                                    message: format!("Vega {:.6} exceeds threshold {:.6}", snapshot.options_vega, threshold),
                                    timestamp: snapshot.timestamp,
                                    source: "deribit".to_string(),
                                    index_price: 0.0,
                                    dte: 0,
                                    option_type: OptionType::Call,
                                    moneyness: 0.0,
                                    mark_price_coin: 0.0,
                                });
                                self.record_alert(key).await;
                            }
                        }
                    }
                    // Check theta
                    if let Some(threshold) = cfg.theta_threshold {
                        if snapshot.options_theta < threshold {
                            let key = format!("theta_{}", snapshot.currency);
                            if !self.in_cooldown(&key).await {
                                alerts.push(Alert {
                                    alert_type: AlertType::PortfolioGreek {
                                        greek: "theta".to_string(),
                                        current: snapshot.options_theta,
                                        threshold
                                    },
                                    tenor: Tenor::Medium,
                                    symbol: format!("PORTFOLIO_{}", snapshot.currency),
                                    iv: 0.0,
                                    message: format!("Theta {:.6} below threshold {:.6}", snapshot.options_theta, threshold),
                                    timestamp: snapshot.timestamp,
                                    source: "deribit".to_string(),
                                    index_price: 0.0,
                                    dte: 0,
                                    option_type: OptionType::Call,
                                    moneyness: 0.0,
                                    mark_price_coin: 0.0,
                                });
                                self.record_alert(key).await;
                            }
                        }
                    }
                }
            }
        }

        alerts
    }
}

impl Clone for PortfolioRule {
    fn clone(&self) -> Self {
        Self {
            metrics: self.metrics.clone(),
            cooldown_secs: self.cooldown_secs,
            last_alert: self.last_alert.clone(),
        }
    }
}

#[async_trait::async_trait]
impl RuleProcessor for PortfolioRule {
    fn name(&self) -> &str {
        "portfolio"
    }

    fn interests(&self) -> Vec<EventType> {
        vec![EventType::Portfolio]
    }

    fn evaluate(&self, _event: &MonitoringEvent) -> Option<Alert> {
        // Portfolio rule evaluates multiple alerts at once via evaluate() method
        // This is called synchronously, so we just return None here
        // The async evaluate() method should be used instead
        None
    }

    async fn on_alert(&self, _alert: &Alert) -> Result<RuleAction> {
        Ok(RuleAction::Continue)
    }

    fn clone_box_rule(&self) -> Box<dyn RuleProcessor> {
        Box::new(self.clone())
    }
}
