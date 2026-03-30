use crate::error::Result;
use crate::models::VolatilityData;
use tokio::sync::mpsc::Receiver;
use async_trait::async_trait;

/// Health status for a data source
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

/// DataSource trait - abstracts where volatility data comes from.
///
/// All data source plugins (Deribit, Binance, CSV, etc.) must implement this trait.
/// The trait is designed to be:
/// - Source-agnostic: No knowledge of specific API details
/// - Uniform: All sources emit the same VolatilityData struct
/// - Testable: Can be mocked for unit tests
#[async_trait]
pub trait DataSource: Send + Sync {
    /// Returns the name of this data source (e.g., "deribit", "binance", "csv")
    fn name(&self) -> &str;

    /// Connect to the data source. Should be called before subscribe().
    async fn connect(&mut self) -> Result<()>;

    /// Subscribe to volatility data for the given symbols.
    /// Returns a receiver that will stream VolatilityData events.
    fn subscribe(&self, symbols: Vec<String>) -> Result<Receiver<VolatilityData>>;

    /// Check the health status of this data source
    async fn health_check(&self) -> HealthStatus;
}
