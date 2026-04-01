//! Rule configuration types.

use serde::{Deserialize, Serialize};

/// Absolute IV threshold rule configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AbsoluteIvRuleConfig {
    pub id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub datasources: Vec<String>,
    pub symbol: String,
    pub short_threshold: f64,
    pub medium_threshold: f64,
    pub long_threshold: f64,
    #[serde(default)]
    pub notifications: Vec<String>,
}

/// Rate of change rule configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RateChangeRuleConfig {
    pub id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub datasources: Vec<String>,
    pub symbol: String,
    pub window_1h_threshold: f64,
    pub window_4h_threshold: f64,
    pub window_24h_threshold: f64,
    #[serde(default)]
    pub notifications: Vec<String>,
}

/// Margin ratio rule configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MarginRatioRuleConfig {
    pub id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub datasources: Vec<String>,
    pub min_threshold: f64,
    #[serde(default)]
    pub notifications: Vec<String>,
}

/// Rule configuration enum
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum RuleConfig {
    AbsoluteIv(AbsoluteIvRuleConfig),
    RateChange(RateChangeRuleConfig),
    MarginRatio(MarginRatioRuleConfig),
}

impl RuleConfig {
    pub fn id(&self) -> &str {
        match self {
            RuleConfig::AbsoluteIv(c) => &c.id,
            RuleConfig::RateChange(c) => &c.id,
            RuleConfig::MarginRatio(c) => &c.id,
        }
    }

    pub fn enabled(&self) -> bool {
        match self {
            RuleConfig::AbsoluteIv(c) => c.enabled,
            RuleConfig::RateChange(c) => c.enabled,
            RuleConfig::MarginRatio(c) => c.enabled,
        }
    }

    pub fn notifications(&self) -> &[String] {
        match self {
            RuleConfig::AbsoluteIv(c) => &c.notifications,
            RuleConfig::RateChange(c) => &c.notifications,
            RuleConfig::MarginRatio(c) => &c.notifications,
        }
    }
}

fn default_true() -> bool { true }
