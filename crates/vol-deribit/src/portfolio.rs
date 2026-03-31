//! Deribit portfolio data models for user.portfolio subscription.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Portfolio notification data from user.portfolio.(currency) channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioData {
    /// Currency code (BTC, ETH, USDC, etc.)
    pub currency: String,
    /// Account equity
    pub equity: f64,
    /// Account balance
    pub balance: f64,
    /// Available funds for trading
    pub available_funds: f64,
    /// Available withdrawal funds
    #[serde(default)]
    pub available_withdrawal_funds: Option<f64>,
    /// Margin balance
    pub margin_balance: f64,
    /// Initial margin requirement
    pub initial_margin: f64,
    /// Maintenance margin requirement
    pub maintenance_margin: f64,
    /// Session unrealized P&L
    #[serde(default)]
    pub session_upl: f64,
    /// Session realized P&L
    #[serde(default)]
    pub session_rpl: f64,
    /// Total P&L
    #[serde(default)]
    pub total_pl: f64,
    /// Total delta (options + futures)
    #[serde(default)]
    pub delta_total: f64,
    /// Options delta
    #[serde(default)]
    pub options_delta: f64,
    /// Options gamma
    #[serde(default)]
    pub options_gamma: f64,
    /// Options theta
    #[serde(default)]
    pub options_theta: f64,
    /// Options vega
    #[serde(default)]
    pub options_vega: f64,
    /// Options value
    #[serde(default)]
    pub options_value: f64,
    /// Options P&L
    #[serde(default)]
    pub options_pl: f64,
    /// Futures P&L
    #[serde(default)]
    pub futures_pl: f64,
    /// Delta total map per index
    #[serde(default)]
    pub delta_total_map: HashMap<String, f64>,
    /// Gamma map per index
    #[serde(default)]
    pub options_gamma_map: HashMap<String, f64>,
    /// Theta map per index
    #[serde(default)]
    pub options_theta_map: HashMap<String, f64>,
    /// Vega map per index
    #[serde(default)]
    pub options_vega_map: HashMap<String, f64>,
    /// Projected delta total
    #[serde(default)]
    pub projected_delta_total: Option<f64>,
    /// Portfolio margining enabled
    #[serde(default)]
    pub portfolio_margining_enabled: bool,
    /// Cross collateral enabled
    #[serde(default)]
    pub cross_collateral_enabled: bool,
    /// Margin model
    #[serde(default)]
    pub margin_model: String,
    /// Fee balance
    #[serde(default)]
    pub fee_balance: Option<f64>,
    /// Projected initial margin
    #[serde(default)]
    pub projected_initial_margin: Option<f64>,
    /// Projected maintenance margin
    #[serde(default)]
    pub projected_maintenance_margin: Option<f64>,
}

impl PortfolioData {
    /// Calculate margin ratio (margin_balance / initial_margin)
    pub fn margin_ratio(&self) -> Option<f64> {
        if self.initial_margin > 0.0 {
            Some(self.margin_balance / self.initial_margin)
        } else {
            None
        }
    }

    /// Get free balance (available_funds)
    pub fn free_balance(&self) -> f64 {
        self.available_funds
    }

    /// Get delta exposure (delta_total)
    pub fn delta_exposure(&self) -> f64 {
        self.delta_total
    }

    /// Get session P&L in USD (approximate for BTC/ETH)
    pub fn session_pnl_usd(&self, index_price: f64) -> f64 {
        (self.session_upl + self.session_rpl) * index_price
    }
}
