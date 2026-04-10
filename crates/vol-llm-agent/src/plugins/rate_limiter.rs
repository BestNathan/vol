//! Rate limiter plugin for concurrency control.

use crate::react::plugin::*;
use crate::react::run_context::RunContext;
use crate::AgentStreamEvent;
use tokio::sync::Semaphore;
use std::sync::Arc;

/// Rate limiter plugin
pub struct RateLimiterPlugin {
    semaphore: Arc<Semaphore>,
}

impl RateLimiterPlugin {
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
        }
    }
}

#[async_trait::async_trait]
impl AgentPlugin for RateLimiterPlugin {
    fn id(&self) -> PluginId {
        "rate_limiter".to_string()
    }

    fn priority(&self) -> u32 {
        5
    }

    /// Interceptor hook - no-op for rate limiter (flow control handled externally)
    async fn intercept(&self, _event: &AgentStreamEvent, _ctx: &RunContext) -> PluginDecision {
        PluginDecision::Continue
    }

    /// Listener hook - logs rate limiting events
    async fn listen(&self, event: &AgentStreamEvent, ctx: &RunContext) {
        match event {
            AgentStreamEvent::AgentStart { .. } => {
                tracing::debug!(run_id = %ctx.run_id, "Rate limiter: agent started");
            }
            AgentStreamEvent::AgentComplete { .. } => {
                tracing::debug!(run_id = %ctx.run_id, "Rate limiter: agent completed");
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::session::{Session, InMemorySessionStore, InMemoryMessageStore};
    use crate::react::AgentConfig;

    fn create_test_run_context() -> RunContext {
        let (ctx, _rx) = RunContext::new(
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
        );
        ctx
    }

    #[test]
    fn test_rate_limiter_id() {
        let plugin = RateLimiterPlugin::new(2);
        assert_eq!(plugin.id(), "rate_limiter");
    }

    #[test]
    fn test_rate_limiter_priority() {
        let plugin = RateLimiterPlugin::new(2);
        assert_eq!(plugin.priority(), 5);
    }

    #[tokio::test]
    async fn test_rate_limiter_allows_concurrent() {
        let plugin = RateLimiterPlugin::new(2);
        let ctx = create_test_run_context();

        // Plugin should always return Continue from intercept
        let event = AgentStreamEvent::AgentStart {
            input: "test".to_string(),
        };
        match plugin.intercept(&event, &ctx).await {
            PluginDecision::Continue => {}
            _ => panic!("Expected Continue"),
        }
    }
}
