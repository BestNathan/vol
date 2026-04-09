//! Retry plugin with exponential backoff.

use crate::react::plugin::*;
use crate::react::run_context::RunContext;
use crate::{AgentError, AgentResponse};
use std::sync::atomic::{AtomicU32, Ordering};

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
    config: RetryConfig,
    attempt: AtomicU32,
}

impl RetryPlugin {
    pub fn new(config: RetryConfig) -> Self {
        Self {
            config,
            attempt: AtomicU32::new(0),
        }
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

    async fn on_start(&self, ctx: &RunContext) -> PluginAction<()> {
        self.attempt.store(0, Ordering::SeqCst);
        let _ = ctx.set("retry.attempt", 0u32).await;
        PluginAction::Continue(())
    }

    async fn intercept(
        &self,
        event: crate::react::plugin::StreamEvent,
        _ctx: &RunContext,
    ) -> PluginAction<Option<crate::react::plugin::StreamEvent>> {
        PluginAction::Continue(Some(event))
    }

    async fn on_error(
        &self,
        ctx: &RunContext,
        _error: &AgentError,
    ) -> PluginAction<()> {
        let attempt = self.attempt.fetch_add(1, Ordering::SeqCst);

        if attempt < self.config.max_retries {
            let delay = (self.config.initial_delay_ms as f64
                * self.config.multiplier.powf(attempt as f64)) as u64;
            let delay = delay.min(self.config.max_delay_ms);

            tracing::warn!(
                run_id = %ctx.run_id,
                attempt = attempt + 1,
                delay_ms = delay,
                "Retrying agent run"
            );

            tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
        }

        PluginAction::Continue(())
    }

    async fn on_complete(
        &self,
        _ctx: &RunContext,
        _response: &AgentResponse,
    ) -> PluginAction<()> {
        PluginAction::Continue(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::session::{Session, InMemorySessionStore, InMemoryMessageStore};
    use crate::react::AgentConfig;

    fn create_test_run_context() -> RunContext {
        RunContext::new(
            "test-run".to_string(),
            "test input".to_string(),
            "session-1".to_string(),
            Arc::new(Session::new(
                "session-1".to_string(),
                Arc::new(InMemorySessionStore::new()),
                Arc::new(InMemoryMessageStore::new()),
            )),
            Arc::new(vol_llm_tool::ToolRegistry::new()),
            AgentConfig::default(),
        )
    }

    #[tokio::test]
    async fn test_retry_plugin_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.initial_delay_ms, 100);
        assert_eq!(config.max_delay_ms, 5000);
        assert_eq!(config.multiplier, 2.0);
    }

    #[tokio::test]
    async fn test_retry_plugin_on_start() {
        let plugin = RetryPlugin::new(RetryConfig::default());

        let ctx = create_test_run_context();

        match plugin.on_start(&ctx).await {
            PluginAction::Continue(()) => {
                assert_eq!(ctx.get::<u32>("retry.attempt").await, Some(0));
            }
            _ => panic!("Expected Continue"),
        }
    }
}
