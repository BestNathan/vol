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
    /// Rule ID for configuration reference
    fn id(&self) -> &str;

    /// Rule type for configuration parsing
    fn rule_type(&self) -> &str;

    /// Declare which event types this rule is interested in.
    /// Rules only receive events matching their interests.
    fn interests(&self) -> Vec<EventType>;

    /// Evaluate an event and return alerts (can produce multiple).
    async fn evaluate(&self, event: &MonitoringEvent) -> Vec<Alert>;

    /// Get configured notification channel IDs
    fn notification_ids(&self) -> Vec<String>;

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
    fn id(&self) -> &str {
        (**self).id()
    }

    fn rule_type(&self) -> &str {
        (**self).rule_type()
    }

    fn interests(&self) -> Vec<EventType> {
        (**self).interests()
    }

    async fn evaluate(&self, event: &MonitoringEvent) -> Vec<Alert> {
        (**self).evaluate(event).await
    }

    fn notification_ids(&self) -> Vec<String> {
        (**self).notification_ids()
    }

    async fn on_alert(&self, alert: &Alert) -> Result<RuleAction> {
        (**self).on_alert(alert).await
    }

    fn clone_box_rule(&self) -> Box<dyn RuleProcessor> {
        (**self).clone_box_rule()
    }
}
