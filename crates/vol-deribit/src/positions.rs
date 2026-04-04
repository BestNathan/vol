//! Deribit position API models for private/get_positions.

use serde::{Deserialize, Serialize};

/// Position data from get_positions API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub instrument_name: String,
    pub size: f64,
    pub average_price: f64,
    pub mark_price: f64,
    pub unrealized_pnl: f64,
    pub delta: f64,
    pub gamma: f64,
    pub vega: f64,
    pub theta: f64,
    pub underlying: String,
}
