//! vol-alert: Alert handler implementations for volatility monitoring.
//!
//! Includes:
//! - Absolute IV threshold handler
//! - Rate of change handler
//! - Term structure handler
//! - Skew handler
//! - Portfolio alert handler
//! - Alert manager with cooldown logic

mod absolute_iv;
mod manager;
mod portfolio;
mod rate_change;
mod skew;
mod term_structure;

pub use absolute_iv::AbsoluteIvHandler;
pub use manager::AlertManager;
pub use portfolio::{PortfolioAlertHandler, PortfolioSnapshot};
pub use rate_change::RateChangeHandler;
pub use skew::SkewHandler;
pub use term_structure::TermStructureHandler;
