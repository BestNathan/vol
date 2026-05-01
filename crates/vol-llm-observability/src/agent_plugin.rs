//! ObservabilityPlugin — sends agent events to the observability service.

use tokio::sync::mpsc;

use crate::agent_client::{BatchCommand, spawn_batch_sender};
use crate::agent_config::ObservabilityAgentConfig;

/// Plugin that forwards agent events to the observability service via HTTP.
pub struct ObservabilityPlugin {
    tx: mpsc::Sender<BatchCommand>,
}

impl ObservabilityPlugin {
    /// Create a new ObservabilityPlugin and spawn the background batch sender.
    pub fn new(
        config: &ObservabilityAgentConfig,
        run_id: String,
        session_id: String,
        agent_id: String,
        agent_type: String,
    ) -> Self {
        let tx = spawn_batch_sender(
            config.ingest_url.clone(),
            config.channel_capacity,
            config.batch_size,
            config.flush_interval_ms,
            run_id,
            session_id,
            agent_id,
            agent_type,
        );

        Self { tx }
    }

    /// Send an event to the batch sender.
    pub async fn send_event(&self, event: vol_llm_core::AgentStreamEvent) {
        let _ = self.tx.send(BatchCommand::Event(event)).await;
    }

    /// Whether this plugin is enabled.
    pub fn is_enabled(config: &ObservabilityAgentConfig) -> bool {
        config.enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_enabled() {
        let enabled_config = ObservabilityAgentConfig { enabled: true, ..Default::default() };
        let disabled_config = ObservabilityAgentConfig { enabled: false, ..Default::default() };
        assert!(ObservabilityPlugin::is_enabled(&enabled_config));
        assert!(!ObservabilityPlugin::is_enabled(&disabled_config));
    }
}
