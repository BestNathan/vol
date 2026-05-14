//! Observability configuration for the agent side.

use serde::Deserialize;

/// Observability plugin configuration for AgentConfig.
#[derive(Debug, Clone, Deserialize)]
pub struct ObservabilityAgentConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,

    #[serde(default = "default_ingest_url")]
    pub ingest_url: String,

    #[serde(default = "default_channel_capacity")]
    pub channel_capacity: usize,

    #[serde(default = "default_batch_size")]
    pub batch_size: usize,

    #[serde(default = "default_flush_ms")]
    pub flush_interval_ms: u64,
}

fn default_true() -> bool { true }
fn default_ingest_url() -> String { "http://localhost:3030/api/v1/events".to_string() }
fn default_channel_capacity() -> usize { 1000 }
fn default_batch_size() -> usize { 10 }
fn default_flush_ms() -> u64 { 500 }

impl Default for ObservabilityAgentConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            ingest_url: default_ingest_url(),
            channel_capacity: default_channel_capacity(),
            batch_size: default_batch_size(),
            flush_interval_ms: default_flush_ms(),
        }
    }
}
