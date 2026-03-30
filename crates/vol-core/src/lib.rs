//! vol-core: Core traits, data models, and event types for the volatility monitoring system.
//!
//! This crate defines the abstraction layer - all plugins implement traits from this crate.

mod datasource;
mod alert;
mod notification;
mod models;
mod event;
mod error;

pub use datasource::*;
pub use alert::*;
pub use notification::*;
pub use models::*;
pub use event::*;
pub use error::*;
