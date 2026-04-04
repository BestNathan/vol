//! vol-datasource: Data source implementations for volatility monitoring.
//!
//! Includes:
//! - Deribit WebSocket client
//! - CSV file reader (for testing)
//! - Portfolio polling via REST API

mod deribit;
mod csv;
mod registry;
mod portfolio;

pub use deribit::DeribitDataSource;
pub use csv::CsvDataSource;
pub use registry::DataSourceRegistry;
pub use portfolio::{PortfolioDataSource, PortfolioDataSourceConfig};
