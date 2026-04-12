//! vol-core: Core traits, data models, and event types for the volatility monitoring system.
//!
//! This crate defines the abstraction layer - all plugins implement traits from this crate.

pub mod alert;
pub mod datasource;
pub mod error;
pub mod event;
pub mod models;
pub mod notification;
pub mod rule;

pub use alert::*;
pub use datasource::*;
pub use error::*;
pub use event::*;
pub use models::*;
pub use notification::*;
pub use rule::*;
