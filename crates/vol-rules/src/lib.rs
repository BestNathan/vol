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
mod rate_change;
mod term_structure;
mod skew;
mod portfolio;
mod manager;
mod registry;

pub use absolute_iv::AbsoluteIvRule;
pub use rate_change::RateChangeRule;
pub use term_structure::TermStructureRule;
pub use skew::SkewRule;
pub use portfolio::PortfolioRule;
pub use manager::AlertManager;
pub use registry::RuleRegistry;
pub use vol_core::PortfolioSnapshot;
