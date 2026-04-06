use crate::error::Result;
use crate::event::MonitoringEvent;
use async_trait::async_trait;
use tokio::sync::mpsc;
use vol_tracing::TracedEvent;

/// Health status for a data source
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

/// DataSource trait - produces monitoring events.
///
/// All datasource plugins (Deribit, Binance, CSV, etc.) must implement this trait.
#[async_trait]
pub trait DataSource: Send + Sync {
    /// Unique datasource ID for configuration reference
    fn id(&self) -> &str;

    /// Event type this datasource produces
    fn event_type(&self) -> crate::event::EventType;

    /// Datasource name for logging
    fn name(&self) -> &str;

    /// Connect to the data source.
    async fn connect(&mut self) -> Result<()>;

    /// Run the datasource, sending events to the provided channel.
    /// Returns when the connection is closed or an error occurs.
    ///
    /// Events are wrapped in TracedEvent for span propagation across channel boundaries.
    async fn run(&self, tx: mpsc::Sender<TracedEvent<MonitoringEvent>>) -> Result<()>;

    /// Health check
    async fn health_check(&self) -> HealthStatus;

    /// Clone as trait object (for spawning in tokio tasks)
    fn clone_box(&self) -> Box<dyn DataSource>;
}

// Implement for Box<dyn DataSource>
#[async_trait]
impl DataSource for Box<dyn DataSource> {
    fn id(&self) -> &str {
        (**self).id()
    }

    fn event_type(&self) -> crate::event::EventType {
        (**self).event_type()
    }

    fn name(&self) -> &str {
        (**self).name()
    }

    async fn connect(&mut self) -> Result<()> {
        (**self).connect().await
    }

    async fn run(&self, tx: mpsc::Sender<TracedEvent<MonitoringEvent>>) -> Result<()> {
        (**self).run(tx).await
    }

    async fn health_check(&self) -> HealthStatus {
        (**self).health_check().await
    }

    fn clone_box(&self) -> Box<dyn DataSource> {
        (**self).clone_box()
    }
}

impl Clone for Box<dyn DataSource> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}
