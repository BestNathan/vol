use serde::{Deserialize, Serialize};

/// Metric configuration - enum-based for type safety
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum MetricConfig {
    #[serde(rename = "free_balance")]
    FreeBalance(ThresholdConfig),

    #[serde(rename = "margin_ratio")]
    MarginRatio(MarginRatioConfig),

    #[serde(rename = "delta_exposure")]
    DeltaExposure(ThresholdConfig),

    #[serde(rename = "session_pnl")]
    SessionPnl(ThresholdConfig),

    #[serde(rename = "total_greeks")]
    TotalGreeks(GreeksConfig),
}

impl MetricConfig {
    pub fn enabled(&self) -> bool {
        match self {
            MetricConfig::FreeBalance(c) => c.enabled,
            MetricConfig::MarginRatio(c) => c.enabled,
            MetricConfig::DeltaExposure(c) => c.enabled,
            MetricConfig::SessionPnl(c) => c.enabled,
            MetricConfig::TotalGreeks(c) => c.enabled,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            MetricConfig::FreeBalance(_) => "free_balance",
            MetricConfig::MarginRatio(_) => "margin_ratio",
            MetricConfig::DeltaExposure(_) => "delta_exposure",
            MetricConfig::SessionPnl(_) => "session_pnl",
            MetricConfig::TotalGreeks(_) => "total_greeks",
        }
    }
}

/// Simple threshold configuration for most metrics
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ThresholdConfig {
    pub enabled: bool,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub min_threshold: Option<f64>,
    #[serde(default)]
    pub max_threshold: Option<f64>,
}

/// Margin ratio specific configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MarginRatioConfig {
    pub enabled: bool,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub min_threshold: Option<f64>,
}

/// Greeks configuration with multiple thresholds
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GreeksConfig {
    pub enabled: bool,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub gamma_threshold: Option<f64>,
    #[serde(default)]
    pub vega_threshold: Option<f64>,
    #[serde(default)]
    pub theta_threshold: Option<f64>,
    #[serde(default)]
    pub delta_threshold: Option<f64>,
}
