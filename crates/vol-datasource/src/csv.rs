//! CSV file data source for testing.

use std::path::Path;
use tokio::sync::mpsc;
use vol_core::{DataSource, EventType, HealthStatus, MonitoringEvent, Result, VolError};
use vol_tracing::TracedEvent;

/// CSV file data source - reads volatility data from a CSV file
#[derive(Clone)]
pub struct CsvDataSource {
    file_path: String,
    id: String,
}

impl CsvDataSource {
    pub fn new(file_path: String, id: String) -> Self {
        Self { file_path, id }
    }
}

#[async_trait::async_trait]
impl DataSource for CsvDataSource {
    fn id(&self) -> &str {
        &self.id
    }

    fn event_type(&self) -> EventType {
        EventType::Volatility
    }

    fn name(&self) -> &str {
        "csv"
    }

    async fn connect(&mut self) -> Result<()> {
        if !Path::new(&self.file_path).exists() {
            return Err(VolError::Connection(format!(
                "CSV file not found: {}",
                self.file_path
            )));
        }
        Ok(())
    }

    async fn run(&self, tx: mpsc::Sender<TracedEvent<MonitoringEvent>>) -> Result<()> {
        // TODO: Read CSV file and stream data
        // For now, just close the channel
        drop(tx);
        Ok(())
    }

    async fn health_check(&self) -> HealthStatus {
        if Path::new(&self.file_path).exists() {
            HealthStatus::Healthy
        } else {
            HealthStatus::Unhealthy
        }
    }

    fn clone_box(&self) -> Box<dyn DataSource> {
        Box::new(self.clone())
    }
}
