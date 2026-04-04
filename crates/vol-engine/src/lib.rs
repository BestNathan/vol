//! vol-engine: Channel-based monitoring engine.
//!
//! Provides the core event loop that orchestrates datasources, rules, and notifications
//! using tokio mpsc channels for efficient, decoupled communication.
//!
//! ## Architecture
//!
//! ```text
//! DataSource → Event Channel → Rules → Alert Channel → Notifications
//! ```
//!
//! ## Example
//!
//! ```rust
//! let engine = MonitoringEngineBuilder::new()
//!     .with_datasource(Box::new(DeribitDataSource::new(config)))
//!     .with_rule(Box::new(IvThresholdRule::new(thresholds)))
//!     .with_notification(Box::new(FeishuNotification::new(feishu_config)))
//!     .build();
//!
//! engine.run().await?;
//! ```

mod engine;
mod builder;
mod config;
mod registry;

pub use engine::MonitoringEngine;
pub use builder::MonitoringEngineBuilder;
pub use config::EngineConfig;
pub use registry::RuleRegistry;
pub use vol_alert::AlertManager;
