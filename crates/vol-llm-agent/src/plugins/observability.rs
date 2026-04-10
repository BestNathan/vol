//! Observability plugin for tracing, metrics, and audit logging.

use crate::react::plugin::*;
use crate::react::run_context::RunContext;
use crate::AgentStreamEvent;

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
    audit_tx: Option<tokio::sync::mpsc::Sender<AuditEvent>>,
}

impl ObservabilityPlugin {
    pub fn new(
        audit_tx: Option<tokio::sync::mpsc::Sender<AuditEvent>>,
    ) -> Self {
        Self {
            audit_tx,
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
            AgentStreamEvent::AgentAborted { .. } => "AgentAborted",
            AgentStreamEvent::PluginEvent { .. } => "PluginEvent",
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

    /// Interceptor hook - no-op for observability (doesn't block flow)
    async fn intercept(&self, _event: &AgentStreamEvent, _ctx: &RunContext) -> PluginDecision {
        PluginDecision::Continue
    }

    /// Listener hook - logs events for observability and audit
    async fn listen(&self, event: &AgentStreamEvent, ctx: &RunContext) {
        let event_type = Self::get_event_type(event);

        tracing::debug!(
            run_id = %ctx.run_id,
            event_type = event_type,
            "Agent event"
        );

        // Send audit log
        if let Some(ref audit_tx) = self.audit_tx {
            let audit_event = AuditEvent {
                run_id: ctx.run_id.clone(),
                timestamp: chrono::Utc::now(),
                event_type: event_type.to_string(),
                data: serde_json::json!({ "event": event_type }),
            };
            if let Err(e) = audit_tx.send(audit_event).await {
                tracing::warn!(run_id = %ctx.run_id, error = %e, "Failed to send audit event");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::session::{Session, InMemorySessionStore, InMemoryMessageStore};
    use crate::react::{AgentConfig, RunContext};

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

    #[tokio::test]
    async fn test_observability_plugin_logs_events() {
        let (audit_tx, mut audit_rx) = tokio::sync::mpsc::channel(100);
        let plugin = ObservabilityPlugin::new(Some(audit_tx));

        let ctx = create_test_run_context();

        // listen should send audit event
        let event = AgentStreamEvent::AgentStart {
            input: "test".to_string(),
        };

        plugin.listen(&event, &ctx).await;

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

    #[test]
    fn test_observability_plugin_id() {
        let plugin = ObservabilityPlugin::new(None);
        assert_eq!(plugin.id(), "observability");
    }

    #[test]
    fn test_observability_plugin_priority() {
        let plugin = ObservabilityPlugin::new(None);
        assert_eq!(plugin.priority(), 10);
    }
}
