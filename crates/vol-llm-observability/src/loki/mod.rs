//! Loki integration for agent observability.
//!
//! Provides `LokiPlugin` which implements `AgentPlugin` to send agent events
//! to a Loki instance via HTTP. Runs alongside `LoggerPlugin` for dual-write
//! (local JSONL + Loki).

pub mod client;
pub mod config;
pub mod labels;
pub mod plugin;

pub use config::LokiConfig;
pub use plugin::LokiPlugin;
