//! vol-rules: Rule processor implementations.
//!
//! Contains:
//! - Absolute IV threshold rule
//! - Rate of change rule
//! - Term structure rule
//! - Skew rule
//! - Portfolio alert rule
//! - Alert manager with cooldown logic

mod absolute_iv;
mod manager;
mod portfolio;
mod rate_change;
mod registry;
mod skew;
mod term_structure;

pub use absolute_iv::AbsoluteIvRule;
pub use manager::AlertManager;
pub use portfolio::PortfolioRule;
pub use rate_change::RateChangeRule;
pub use registry::RuleRegistry;
pub use skew::SkewRule;
pub use term_structure::TermStructureRule;
pub use vol_core::PortfolioSnapshot;
