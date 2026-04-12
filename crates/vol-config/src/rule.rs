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
    /// ATM moneyness threshold per tenor - only alert on options within this moneyness range
    /// e.g., 0.10 means |moneyness| <= 10% (within 10% of index price)
    #[serde(default = "default_short_atm")]
    pub short_atm_threshold: f64,
    #[serde(default = "default_medium_atm")]
    pub medium_atm_threshold: f64,
    #[serde(default = "default_long_atm")]
    pub long_atm_threshold: f64,
    /// Per-DTE ATM moneyness thresholds - overrides tenor-based thresholds for specific DTE values
    /// Key is DTE in days as string (TOML limitation), value is the ATM threshold
    /// e.g., {"1": 0.01, "2": 0.02, "3": 0.03}
    #[serde(default)]
    pub dte_atm_thresholds: std::collections::HashMap<String, f64>,
    #[serde(default)]
    pub notifications: Vec<String>,
}

fn default_short_atm() -> f64 {
    0.10
} // 10% for short-term options
fn default_medium_atm() -> f64 {
    0.08
} // 8% for medium-term options
fn default_long_atm() -> f64 {
    0.05
} // 5% for long-term options

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

/// Term structure rule configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TermStructureRuleConfig {
    pub id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub datasources: Vec<String>,
    pub short_long_spread_threshold: f64,
    #[serde(default)]
    pub notifications: Vec<String>,
}

/// Skew rule configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SkewRuleConfig {
    pub id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub datasources: Vec<String>,
    pub symbol: String,
    pub threshold: f64,
    #[serde(default)]
    pub notifications: Vec<String>,
}

/// Portfolio rule configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PortfolioRuleConfig {
    pub id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub datasources: Vec<String>,
    #[serde(default)]
    pub metrics: Vec<super::MetricConfig>,
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
    TermStructure(TermStructureRuleConfig),
    Skew(SkewRuleConfig),
    Portfolio(PortfolioRuleConfig),
    MarginRatio(MarginRatioRuleConfig),
}

impl RuleConfig {
    pub fn id(&self) -> &str {
        match self {
            RuleConfig::AbsoluteIv(c) => &c.id,
            RuleConfig::RateChange(c) => &c.id,
            RuleConfig::TermStructure(c) => &c.id,
            RuleConfig::Skew(c) => &c.id,
            RuleConfig::Portfolio(c) => &c.id,
            RuleConfig::MarginRatio(c) => &c.id,
        }
    }

    pub fn enabled(&self) -> bool {
        match self {
            RuleConfig::AbsoluteIv(c) => c.enabled,
            RuleConfig::RateChange(c) => c.enabled,
            RuleConfig::TermStructure(c) => c.enabled,
            RuleConfig::Skew(c) => c.enabled,
            RuleConfig::Portfolio(c) => c.enabled,
            RuleConfig::MarginRatio(c) => c.enabled,
        }
    }

    pub fn notifications(&self) -> &[String] {
        match self {
            RuleConfig::AbsoluteIv(c) => &c.notifications,
            RuleConfig::RateChange(c) => &c.notifications,
            RuleConfig::TermStructure(c) => &c.notifications,
            RuleConfig::Skew(c) => &c.notifications,
            RuleConfig::Portfolio(c) => &c.notifications,
            RuleConfig::MarginRatio(c) => &c.notifications,
        }
    }
}

fn default_true() -> bool {
    true
}
