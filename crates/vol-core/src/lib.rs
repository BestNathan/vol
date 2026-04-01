//! vol-core: Core traits, data models, and event types for the volatility monitoring system.
//!
//! This crate defines the abstraction layer - all plugins implement traits from this crate.

pub mod datasource;
pub mod alert;
pub mod notification;
pub mod models;
pub mod event;
pub mod error;
pub mod rule;

pub use datasource::*;
pub use alert::*;
pub use notification::*;
pub use models::*;
pub use event::*;
pub use error::*;
pub use rule::*;
