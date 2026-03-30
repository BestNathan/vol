//! CSV file data source for testing.

use std::path::Path;
use tokio::sync::mpsc;
use vol_core::{DataSource, HealthStatus, VolatilityData, VolError, Result};

/// CSV file data source - reads volatility data from a CSV file
pub struct CsvDataSource {
    file_path: String,
}

impl CsvDataSource {
    pub fn new(file_path: String) -> Self {
        Self { file_path }
    }
}

#[async_trait::async_trait]
impl DataSource for CsvDataSource {
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

    fn subscribe(&self, _symbols: Vec<String>) -> Result<mpsc::Receiver<VolatilityData>> {
        let (tx, rx) = mpsc::channel(1024);

        tokio::spawn(async move {
            // TODO: Read CSV file and stream data
            // For now, just close the channel
            drop(tx);
        });

        Ok(rx)
    }

    async fn health_check(&self) -> HealthStatus {
        if Path::new(&self.file_path).exists() {
            HealthStatus::Healthy
        } else {
            HealthStatus::Unhealthy
        }
    }
}
