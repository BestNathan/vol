//! vol-datasource: Data source implementations for volatility monitoring.
//!
//! Includes:
//! - Deribit WebSocket client
//! - CSV file reader (for testing)
//! - Portfolio polling via REST API

mod csv;
mod portfolio;
mod registry;
mod volatility;

pub use csv::CsvDataSource;
pub use portfolio::PortfolioDataSource;
pub use registry::DataSourceRegistry;
pub use vol_config::PortfolioConfig;
#[allow(deprecated)]
pub use vol_tracing::WithSpan;
pub use volatility::VolatilityDataSource;
