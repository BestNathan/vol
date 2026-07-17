//! Portfolio data JSONL output handler.

use std::path::PathBuf;
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::sync::mpsc;
use tracing::info;
use vol_core::{Result, VolError};
use vol_deribit::portfolio::PortfolioData;

/// Portfolio data JSONL output handler
pub struct PortfolioOutput {
    output_dir: PathBuf,
    file_format: String,
    rotate_interval: String,
}

impl PortfolioOutput {
    pub fn new(output_dir: PathBuf, file_format: String, rotate_interval: String) -> Self {
        Self {
            output_dir,
            file_format,
            rotate_interval,
        }
    }

    /// Run the output loop, writing portfolio data to JSONL files
    pub async fn run(self, mut rx: mpsc::Receiver<PortfolioData>) -> Result<()> {
        // Create output directory if needed
        tokio::fs::create_dir_all(&self.output_dir)
            .await
            .map_err(|e| VolError::Internal(format!("Failed to create output directory: {e}")))?;

        let mut current_file = self.current_file_path();
        let mut writer = BufWriter::new(
            tokio::fs::File::create(&current_file)
                .await
                .map_err(|e| VolError::Internal(format!("Failed to create output file: {e}")))?,
        );

        while let Some(data) = rx.recv().await {
            // Check if rotation needed
            let new_file = self.current_file_path();
            if new_file != current_file {
                writer
                    .flush()
                    .await
                    .map_err(|e| VolError::Internal(format!("Failed to flush writer: {e}")))?;
                current_file = new_file;
                writer =
                    BufWriter::new(tokio::fs::File::create(&current_file).await.map_err(|e| {
                        VolError::Internal(format!("Failed to create output file: {e}"))
                    })?);
            }

            // Write as JSONL
            let json = serde_json::to_string(&data).map_err(|e| {
                VolError::Internal(format!("Failed to serialize portfolio data: {e}"))
            })?;
            writer
                .write_all(json.as_bytes())
                .await
                .map_err(|e| VolError::Internal(format!("Failed to write portfolio data: {e}")))?;
            writer
                .write_all(b"\n")
                .await
                .map_err(|e| VolError::Internal(format!("Failed to write newline: {e}")))?;

            if self.file_format == "jsonl" {
                info!("Portfolio data written: {}", data.currency);
            }
        }

        writer
            .flush()
            .await
            .map_err(|e| VolError::Internal(format!("Failed to flush writer: {e}")))?;
        Ok(())
    }

    fn current_file_path(&self) -> PathBuf {
        let now = chrono::Utc::now();
        let filename = match self.rotate_interval.as_str() {
            "hourly" => format!("portfolio_{}.jsonl", now.format("%Y%m%d_%H")),
            "daily" => format!("portfolio_{}.jsonl", now.format("%Y%m%d")),
            "weekly" => format!("portfolio_{}.jsonl", now.format("%Y%m%d_week%V")),
            _ => format!("portfolio_{}.jsonl", now.format("%Y%m%d")),
        };
        self.output_dir.join(filename)
    }
}
