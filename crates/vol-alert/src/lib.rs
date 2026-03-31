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
mod rate_change;
mod term_structure;
mod skew;
mod portfolio;
mod manager;

pub use absolute_iv::AbsoluteIvHandler;
pub use rate_change::RateChangeHandler;
pub use term_structure::TermStructureHandler;
pub use skew::SkewHandler;
pub use portfolio::{PortfolioAlertHandler, PortfolioSnapshot};
pub use manager::AlertManager;
