//! ObservabilityPlugin — sends agent events to the observability service.

use tokio::sync::mpsc;
use vol_llm_core::AgentStreamEvent;

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

    /// Whether this plugin is enabled.
    pub fn is_enabled(config: &ObservabilityAgentConfig) -> bool {
        config.enabled
    }
}

#[async_trait::async_trait]
impl vol_llm_agent::react::AgentPlugin for ObservabilityPlugin {
    fn id(&self) -> String {
        "observability".to_string()
    }

    fn priority(&self) -> u32 {
        5 // Lower than logger (10) so logger runs first
    }

    async fn intercept(
        &self,
        _event: &AgentStreamEvent,
        _ctx: &vol_llm_agent::react::RunContext,
    ) -> vol_llm_agent::react::PluginDecision {
        vol_llm_agent::react::PluginDecision::Continue
    }

    async fn listen(
        &self,
        event: &AgentStreamEvent,
        _ctx: &vol_llm_agent::react::RunContext,
    ) {
        let _ = self.tx.send(BatchCommand::Event(event.clone())).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_agent::react::AgentPlugin;

    #[tokio::test]
    async fn test_plugin_id() {
        let config = ObservabilityAgentConfig::default();
        let plugin = ObservabilityPlugin::new(
            &config,
            "run-1".to_string(),
            "session-1".to_string(),
            "agent-1".to_string(),
            "CodingAgent".to_string(),
        );
        assert_eq!(plugin.id(), "observability");
    }

    #[tokio::test]
    async fn test_plugin_priority() {
        let config = ObservabilityAgentConfig::default();
        let plugin = ObservabilityPlugin::new(
            &config,
            "run-1".to_string(),
            "session-1".to_string(),
            "agent-1".to_string(),
            "CodingAgent".to_string(),
        );
        assert_eq!(plugin.priority(), 5);
    }

    #[test]
    fn test_is_enabled() {
        let enabled_config = ObservabilityAgentConfig { enabled: true, ..Default::default() };
        let disabled_config = ObservabilityAgentConfig { enabled: false, ..Default::default() };
        assert!(ObservabilityPlugin::is_enabled(&enabled_config));
        assert!(!ObservabilityPlugin::is_enabled(&disabled_config));
    }
}
