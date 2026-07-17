//! Data source registry for managing multiple data sources.

use std::collections::HashMap;
use vol_core::DataSource;

/// Registry for managing multiple data source instances
pub struct DataSourceRegistry {
    sources: HashMap<String, Box<dyn DataSource>>,
}

impl DataSourceRegistry {
    pub fn new() -> Self {
        Self {
            sources: HashMap::new(),
        }
    }

    /// Register a data source
    pub fn register(&mut self, source: Box<dyn DataSource>) {
        let name = source.name().to_string();
        self.sources.insert(name, source);
    }

    /// Get a data source by name
    pub fn get(&self, name: &str) -> Option<&dyn DataSource> {
        self.sources.get(name).map(std::convert::AsRef::as_ref)
    }

    /// Get all registered data source names
    pub fn names(&self) -> Vec<&str> {
        self.sources
            .keys()
            .map(std::string::String::as_str)
            .collect()
    }

    /// Check if a data source is registered
    pub fn contains(&self, name: &str) -> bool {
        self.sources.contains_key(name)
    }
}

impl Default for DataSourceRegistry {
    fn default() -> Self {
        Self::new()
    }
}
