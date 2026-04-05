//! vol-datasource: Data source implementations for volatility monitoring.
//!
//! Includes:
//! - Deribit WebSocket client
//! - CSV file reader (for testing)
//! - Portfolio polling via REST API

mod volatility;
mod csv;
mod registry;
mod portfolio;

pub use volatility::VolatilityDataSource;
pub use csv::CsvDataSource;
pub use registry::DataSourceRegistry;
pub use portfolio::PortfolioDataSource;
pub use vol_config::PortfolioConfig;
#[allow(deprecated)]
pub use vol_tracing::WithSpan;
