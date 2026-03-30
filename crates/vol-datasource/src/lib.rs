//! vol-datasource: Data source implementations for volatility monitoring.
//!
//! Includes:
//! - Deribit WebSocket client
//! - CSV file reader (for testing)

mod deribit;
mod csv;
mod registry;

pub use deribit::DeribitDataSource;
pub use csv::CsvDataSource;
pub use registry::DataSourceRegistry;
