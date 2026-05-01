//! Wrappers that implement AgentPlugin for observability plugins.

use std::path::PathBuf;
use vol_llm_core::AgentStreamEvent;
use vol_llm_observability::LoggerPlugin;
use vol_llm_observability::ObservabilityAgentConfig;
use vol_llm_observability::ObservabilityPlugin as InnerObservabilityPlugin;

use super::{AgentPlugin, PluginDecision, RunContext};

// --- ObservabilityAgentPlugin ---

/// Agent-side wrapper that implements AgentPlugin for the observability plugin.
pub struct ObservabilityAgentPlugin {
    inner: InnerObservabilityPlugin,
}

impl ObservabilityAgentPlugin {
    /// Create a new ObservabilityAgentPlugin if observability is configured and enabled.
    pub fn new(
        config: &ObservabilityAgentConfig,
        run_id: String,
        session_id: String,
        agent_id: String,
        agent_type: String,
    ) -> Self {
        Self {
            inner: InnerObservabilityPlugin::new(
                config,
                run_id,
                session_id,
                agent_id,
                agent_type,
            ),
        }
    }

    /// Whether observability is enabled for the given config.
    pub fn is_enabled(config: &ObservabilityAgentConfig) -> bool {
        InnerObservabilityPlugin::is_enabled(config)
    }
}

#[async_trait::async_trait]
impl AgentPlugin for ObservabilityAgentPlugin {
    fn id(&self) -> String {
        "observability".to_string()
    }

    fn priority(&self) -> u32 {
        5 // Lower than logger (10) so logger runs first
    }

    async fn intercept(
        &self,
        _event: &AgentStreamEvent,
        _ctx: &RunContext,
    ) -> PluginDecision {
        PluginDecision::Continue
    }

    async fn listen(
        &self,
        event: &AgentStreamEvent,
        _ctx: &RunContext,
    ) {
        let _ = self.inner.send_event(event.clone()).await;
    }
}

// --- LoggerAgentPlugin ---

/// Agent-side wrapper that implements AgentPlugin for the logger plugin.
pub struct LoggerAgentPlugin {
    inner: LoggerPlugin,
}

impl LoggerAgentPlugin {
    /// Create a new LoggerAgentPlugin with the given working directory and run_id.
    pub fn new(working_dir: PathBuf) -> Self {
        Self {
            inner: LoggerPlugin::new(working_dir),
        }
    }
}

#[async_trait::async_trait]
impl AgentPlugin for LoggerAgentPlugin {
    fn id(&self) -> String {
        "logger".to_string()
    }

    fn priority(&self) -> u32 {
        10 // Higher number = lower priority, runs after observability (5)
    }

    async fn intercept(
        &self,
        _event: &AgentStreamEvent,
        _ctx: &RunContext,
    ) -> PluginDecision {
        PluginDecision::Continue
    }

    async fn listen(
        &self,
        event: &AgentStreamEvent,
        ctx: &RunContext,
    ) {
        if !LoggerPlugin::should_log(event) {
            return;
        }
        let entry = LoggerPlugin::create_log_entry(event, &ctx.run_id);
        let path = self.inner.log_path(event, &ctx.run_id);
        let line = entry.to_json_line();
        if let Err(e) = vol_llm_observability::append_log(&path, &line).await {
            tracing::warn!(path = %path.display(), error = %e, "Failed to write log entry");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_observability_plugin_is_enabled() {
        let enabled_config = ObservabilityAgentConfig { enabled: true, ..Default::default() };
        let disabled_config = ObservabilityAgentConfig { enabled: false, ..Default::default() };
        assert!(ObservabilityAgentPlugin::is_enabled(&enabled_config));
        assert!(!ObservabilityAgentPlugin::is_enabled(&disabled_config));
    }

    #[test]
    fn test_logger_plugin_id() {
        let plugin = LoggerAgentPlugin::new(PathBuf::from("/tmp"));
        assert_eq!(plugin.id(), "logger");
    }

    #[test]
    fn test_logger_plugin_priority() {
        let plugin = LoggerAgentPlugin::new(PathBuf::from("/tmp"));
        assert_eq!(plugin.priority(), 10);
    }

    #[tokio::test]
    async fn test_observability_plugin_id() {
        let config = ObservabilityAgentConfig::default();
        let plugin = ObservabilityAgentPlugin::new(
            &config,
            "run-1".to_string(),
            "session-1".to_string(),
            "agent-1".to_string(),
            "ReActAgent".to_string(),
        );
        assert_eq!(plugin.id(), "observability");
    }

    #[tokio::test]
    async fn test_observability_plugin_priority() {
        let config = ObservabilityAgentConfig::default();
        let plugin = ObservabilityAgentPlugin::new(
            &config,
            "run-1".to_string(),
            "session-1".to_string(),
            "agent-1".to_string(),
            "ReActAgent".to_string(),
        );
        assert_eq!(plugin.priority(), 5);
    }
}
