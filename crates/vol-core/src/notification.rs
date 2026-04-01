use crate::event::Alert;
use crate::error::Result;
use async_trait::async_trait;

/// NotificationChannel trait - delivers alerts to users.
///
/// All notification plugins (feishu, stdout, slack, etc.) must implement this trait.
#[async_trait]
pub trait NotificationChannel: Send + Sync {
    /// Returns the name of this notification handler (e.g., "feishu", "stdout")
    fn name(&self) -> &str;

    /// Send an alert notification. Returns Ok(()) if sent successfully.
    async fn send(&self, alert: &Alert) -> Result<()>;

    /// Check if channel is enabled
    fn is_enabled(&self) -> bool {
        true
    }

    /// Clone as trait object
    fn clone_box(&self) -> Box<dyn NotificationChannel>;
}

#[async_trait]
impl NotificationChannel for Box<dyn NotificationChannel> {
    fn name(&self) -> &str {
        (**self).name()
    }

    async fn send(&self, alert: &Alert) -> Result<()> {
        (**self).send(alert).await
    }

    fn is_enabled(&self) -> bool {
        (**self).is_enabled()
    }

    fn clone_box(&self) -> Box<dyn NotificationChannel> {
        (**self).clone_box()
    }
}

impl Clone for Box<dyn NotificationChannel> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}
