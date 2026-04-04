//! Engine configuration.

use vol_config::EngineConfigFile;

/// Monitoring engine configuration
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Event channel capacity (max events in flight)
    pub event_buffer_size: usize,
    /// Alert channel capacity (max alerts in flight)
    pub alert_buffer_size: usize,
    /// Enable backpressure - block datasource when channel is full
    pub enable_backpressure: bool,
    /// Engine config file reference for cooldown settings
    pub config_file: EngineConfigFile,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            event_buffer_size: 1000,
            alert_buffer_size: 100,
            enable_backpressure: true,
            config_file: EngineConfigFile::default(),
        }
    }
}

impl EngineConfig {
    /// Create a new config with custom buffer sizes
    pub fn new(event_buffer_size: usize, alert_buffer_size: usize, config_file: EngineConfigFile) -> Self {
        Self {
            event_buffer_size,
            alert_buffer_size,
            enable_backpressure: true,
            config_file,
        }
    }

    /// Set backpressure behavior
    pub fn with_backpressure(mut self, enable: bool) -> Self {
        self.enable_backpressure = enable;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = EngineConfig::default();
        assert_eq!(config.event_buffer_size, 1000);
        assert_eq!(config.alert_buffer_size, 100);
        assert!(config.enable_backpressure);
    }
}
