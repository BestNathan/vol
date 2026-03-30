use crate::event::Alert;
use crate::error::Result;
use async_trait::async_trait;

/// NotificationHandler trait - delivers alerts to users.
///
/// All notification plugins (feishu, stdout, slack, etc.) must implement this trait.
#[async_trait]
pub trait NotificationHandler: Send + Sync {
    /// Returns the name of this notification handler (e.g., "feishu", "stdout")
    fn name(&self) -> &str;

    /// Send an alert notification. Returns Ok(()) if sent successfully.
    async fn send(&self, alert: &Alert) -> Result<()>;
}
