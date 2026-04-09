//! Observability plugin for tracing, metrics, and audit logging.

use crate::react::plugin::*;
use crate::react::run_context::RunContext;
use crate::{AgentStreamEvent, AgentResponse, AgentError};
use std::time::Instant;

/// Audit event for logging
#[derive(Debug, Clone, serde::Serialize)]
pub struct AuditEvent {
    pub run_id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub event_type: String,
    pub data: serde_json::Value,
}

/// Observability plugin
pub struct ObservabilityPlugin {
    run_start: Instant,
    audit_tx: Option<tokio::sync::mpsc::Sender<AuditEvent>>,
}

impl ObservabilityPlugin {
    pub fn new(
        audit_tx: Option<tokio::sync::mpsc::Sender<AuditEvent>>,
    ) -> Self {
        Self {
            audit_tx,
            run_start: Instant::now(),
        }
    }

    fn get_event_type(event: &AgentStreamEvent) -> &'static str {
        match event {
            AgentStreamEvent::AgentStart { .. } => "AgentStart",
            AgentStreamEvent::ThinkingComplete { .. } => "ThinkingComplete",
            AgentStreamEvent::ToolCallBegin { .. } => "ToolCallBegin",
            AgentStreamEvent::ToolCallComplete { .. } => "ToolCallComplete",
            AgentStreamEvent::IterationComplete { .. } => "IterationComplete",
            AgentStreamEvent::AgentComplete { .. } => "AgentComplete",
        }
    }
}

#[async_trait::async_trait]
impl AgentPlugin for ObservabilityPlugin {
    fn id(&self) -> PluginId {
        "observability".to_string()
    }

    fn priority(&self) -> u32 {
        10
    }

    async fn on_start(&self, ctx: &RunContext) -> PluginAction<()> {
        tracing::info!(
            run_id = %ctx.run_id,
            session_id = %ctx.session_id,
            input = %ctx.user_input,
            "Agent run started"
        );

        PluginAction::Continue(())
    }

    async fn intercept(
        &self,
        event: crate::react::plugin::StreamEvent,
        ctx: &RunContext,
    ) -> PluginAction<Option<crate::react::plugin::StreamEvent>> {
        match &event {
            Ok(agent_event) => {
                tracing::debug!(
                    run_id = %ctx.run_id,
                    event_type = Self::get_event_type(agent_event),
                    "Agent event"
                );

                // Send audit log
                if let Some(ref audit_tx) = self.audit_tx {
                    let audit_event = AuditEvent {
                        run_id: ctx.run_id.clone(),
                        timestamp: chrono::Utc::now(),
                        event_type: Self::get_event_type(agent_event).to_string(),
                        data: serde_json::json!({ "event": "logged" }),
                    };
                    let _ = audit_tx.send(audit_event).await;
                }
            }
            Err(e) => {
                tracing::error!(run_id = %ctx.run_id, error = %e, "Agent error");
            }
        }

        PluginAction::Continue(Some(event))
    }

    async fn on_complete(
        &self,
        ctx: &RunContext,
        response: &AgentResponse,
    ) -> PluginAction<()> {
        let elapsed = self.run_start.elapsed();

        tracing::info!(
            run_id = %ctx.run_id,
            duration_ms = elapsed.as_millis(),
            iterations = response.iterations,
            "Agent run completed"
        );

        PluginAction::Continue(())
    }

    async fn on_error(
        &self,
        ctx: &RunContext,
        error: &AgentError,
    ) -> PluginAction<()> {
        let elapsed = self.run_start.elapsed();

        tracing::error!(
            run_id = %ctx.run_id,
            error = %error,
            duration_ms = elapsed.as_millis(),
            "Agent run failed"
        );

        PluginAction::Continue(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::AgentStreamEvent;
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
    async fn test_observability_plugin_logs_events() {
        let (audit_tx, mut audit_rx) = tokio::sync::mpsc::channel(100);
        let plugin = ObservabilityPlugin::new(Some(audit_tx));

        let ctx = create_test_run_context();

        // on_start should log
        match plugin.on_start(&ctx).await {
            PluginAction::Continue(()) => {}
            _ => panic!("Expected Continue"),
        }

        // intercept should send audit event
        let event = Ok(AgentStreamEvent::AgentStart {
            input: "test".to_string(),
        });

        match plugin.intercept(event, &ctx).await {
            PluginAction::Continue(Some(_)) => {}
            _ => panic!("Expected Continue"),
        }

        // Should have received audit event
        let audit_event = tokio::time::timeout(
            std::time::Duration::from_millis(100),
            audit_rx.recv(),
        )
        .await
        .expect("Timeout waiting for audit event")
        .expect("Channel closed");

        assert_eq!(audit_event.run_id, "test-run");
        assert_eq!(audit_event.event_type, "AgentStart");
    }

    #[tokio::test]
    async fn test_observability_plugin_on_complete() {
        let plugin = ObservabilityPlugin::new(None);

        let ctx = create_test_run_context();

        let response = AgentResponse {
            content: "test response".to_string(),
            reasoning: String::new(),
            iterations: 2,
            tool_calls: Vec::new(),
        };

        match plugin.on_complete(&ctx, &response).await {
            PluginAction::Continue(()) => {}
            _ => panic!("Expected Continue"),
        }
    }
}
