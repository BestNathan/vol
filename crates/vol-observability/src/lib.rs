//! vol-observability: Independent observability service for ReAct Agent events.
//!
//! Receives agent events via HTTP, routes to Loki (structured logs)
//! and TDengine (time-series metrics).

pub mod config;
pub mod event;
pub mod ingest;
pub mod loki_writer;
pub mod tdengine_writer;
