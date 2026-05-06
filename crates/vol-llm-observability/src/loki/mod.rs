//! Loki integration for agent observability.
//!
//! Provides `LokiPlugin` which implements `AgentPlugin` to send agent events
//! via `tracing::info!` structured logging. The tracing-subscriber stack,
//! extended with opentelemetry-appender-tracing, routes logs to the OTel Collector.

pub mod plugin;

pub use plugin::LokiPlugin;
