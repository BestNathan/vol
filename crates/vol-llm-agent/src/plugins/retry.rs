//! Retry plugin with exponential backoff.

use crate::react::plugin::*;
use crate::AgentStreamEvent;

/// Retry configuration
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
    pub multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay_ms: 100,
            max_delay_ms: 5000,
            multiplier: 2.0,
        }
    }
}

/// Retry plugin
pub struct RetryPlugin {
    #[allow(dead_code)]
    config: RetryConfig,
}

impl RetryPlugin {
    pub fn new(config: RetryConfig) -> Self {
        Self { config }
    }
}

#[async_trait::async_trait]
impl AgentPlugin for RetryPlugin {
    fn id(&self) -> PluginId {
        "retry".to_string()
    }

    fn priority(&self) -> u32 {
        30
    }

    /// Interceptor hook - no-op for retry (retry logic handled externally)
    async fn intercept(&self, _event: &AgentStreamEvent, _ctx: &RunContext) -> PluginDecision {
        PluginDecision::Continue
    }

    /// Listener hook - logs retry events
    async fn listen(&self, event: &AgentStreamEvent, ctx: &RunContext) {
        match event {
            AgentStreamEvent::AgentAborted { reason, .. } => {
                tracing::warn!(
                    run_id = %ctx.run_id,
                    reason = %reason,
                    "Retry: agent aborted"
                );
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::react::{AgentConfig, RunContext};
    use vol_session::{InMemoryEntryStore, Session};
    use std::sync::Arc;

    fn create_test_run_context() -> RunContext {
        let (ctx, _rx) = RunContext::new(
            "test-run".to_string(),
            "test input".to_string(),
            AgentConfig::default(),
        );
        ctx
    }

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.initial_delay_ms, 100);
        assert_eq!(config.max_delay_ms, 5000);
        assert_eq!(config.multiplier, 2.0);
    }

    #[test]
    fn test_retry_plugin_id() {
        let plugin = RetryPlugin::new(RetryConfig::default());
        assert_eq!(plugin.id(), "retry");
    }

    #[tokio::test]
    async fn test_retry_plugin_intercept() {
        let plugin = RetryPlugin::new(RetryConfig::default());
        let ctx = create_test_run_context();

        let event = AgentStreamEvent::agent_start("test".to_string());
        match plugin.intercept(&event, &ctx).await {
            PluginDecision::Continue => {}
            _ => panic!("Expected Continue"),
        }
    }
}
