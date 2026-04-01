//! Rule processor trait - evaluates events and produces alerts.

use crate::event::{MonitoringEvent, Alert, EventType};
use crate::error::Result;
use async_trait::async_trait;

/// Rule action after processing an alert
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleAction {
    /// Continue monitoring
    Continue,
    /// Pause this rule temporarily
    Pause,
    /// Stop this rule permanently
    Stop,
}

/// RuleProcessor trait - evaluates monitoring events and emits alerts.
///
/// All rule implementations (IV threshold, portfolio margin, etc.) must implement this trait.
#[async_trait]
pub trait RuleProcessor: Send + Sync {
    /// Rule name for logging and identification
    fn name(&self) -> &str;

    /// Declare which event types this rule is interested in.
    /// Rules only receive events matching their interests.
    fn interests(&self) -> Vec<EventType>;

    /// Evaluate an event and optionally return an alert.
    /// This is called synchronously - keep it fast!
    fn evaluate(&self, event: &MonitoringEvent) -> Option<Alert>;

    /// Optional callback after an alert is sent.
    /// Can be used for cooldown logic, state updates, etc.
    async fn on_alert(&self, _alert: &Alert) -> Result<RuleAction> {
        Ok(RuleAction::Continue)
    }

    /// Clone as trait object
    fn clone_box_rule(&self) -> Box<dyn RuleProcessor>;
}

// Implement RuleProcessor for Box<dyn RuleProcessor>
#[async_trait]
impl RuleProcessor for Box<dyn RuleProcessor> {
    fn name(&self) -> &str {
        (**self).name()
    }

    fn interests(&self) -> Vec<EventType> {
        (**self).interests()
    }

    fn evaluate(&self, event: &MonitoringEvent) -> Option<Alert> {
        (**self).evaluate(event)
    }

    async fn on_alert(&self, alert: &Alert) -> Result<RuleAction> {
        (**self).on_alert(alert).await
    }

    fn clone_box_rule(&self) -> Box<dyn RuleProcessor> {
        (**self).clone_box_rule()
    }
}
