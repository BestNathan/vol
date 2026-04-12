use crate::models::VolatilityData;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Market data event types
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum MarketDataType {
    Ticker,
    MarkPrice,
    Trade,
    OrderBook,
}

/// Portfolio metric event types
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum PortfolioMetricType {
    MarginRatio,
    FreeBalance,
    DeltaExposure,
    SessionPnL,
    Greeks,
}

/// Event type identifier for rule filtering
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum EventType {
    MarketData(MarketDataType),
    Portfolio(PortfolioMetricType),
    Volatility,
    Custom(String),
}

/// Portfolio snapshot for account monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioSnapshot {
    pub currency: String,
    pub timestamp: u64,
    pub equity: f64,
    pub balance: f64,
    pub available_funds: f64,
    pub margin_balance: f64,
    pub initial_margin: f64,
    pub maintenance_margin: f64,
    pub session_pnl: f64,
    pub delta_total: f64,
    pub options_delta: f64,
    pub options_gamma: f64,
    pub options_theta: f64,
    pub options_vega: f64,
}

/// Generic market tick
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketTick {
    pub symbol: String,
    pub timestamp: u64,
    pub source: String,
    pub price: f64,
    pub volume: f64,
    pub extra: HashMap<String, serde_json::Value>,
}

/// Unified monitoring event - all datasources produce these
#[derive(Debug, Clone)]
pub enum MonitoringEvent {
    /// Option volatility data (IV, mark price, etc.)
    Volatility(VolatilityData),
    /// Portfolio snapshot (balance, margin, Greeks, etc.)
    Portfolio(PortfolioSnapshot),
    /// Generic market tick data
    Market(MarketTick),
    /// Custom extensible event
    Custom {
        source: String,
        kind: String,
        timestamp: u64,
        data: HashMap<String, serde_json::Value>,
    },
}

impl MonitoringEvent {
    /// Get the event type for routing/filtering
    pub fn event_type(&self) -> EventType {
        match self {
            Self::Volatility(_) => EventType::Volatility,
            Self::Portfolio(_) => EventType::Portfolio(PortfolioMetricType::Greeks),
            Self::Market(_) => EventType::MarketData(MarketDataType::Ticker),
            Self::Custom { kind, .. } => EventType::Custom(kind.clone()),
        }
    }

    /// Get event timestamp
    pub fn timestamp(&self) -> u64 {
        match self {
            Self::Volatility(v) => v.timestamp,
            Self::Portfolio(p) => p.timestamp,
            Self::Market(m) => m.timestamp,
            Self::Custom { timestamp, .. } => *timestamp,
        }
    }

    /// Get event source name
    pub fn source(&self) -> &str {
        match self {
            Self::Volatility(v) => &v.source,
            Self::Portfolio(_) => "portfolio",
            Self::Market(m) => &m.source,
            Self::Custom { source, .. } => source,
        }
    }
}

// Re-export Alert from alert module for backward compatibility
pub use crate::alert::Alert;
