//! Chain builder for MonitoringEngine.

use crate::{MonitoringEngine, EngineConfig};
use vol_core::{DataSource, RuleProcessor, NotificationChannel};

/// Builder for constructing MonitoringEngine with fluent API
pub struct MonitoringEngineBuilder {
    config: EngineConfig,
    datasources: Vec<Box<dyn DataSource>>,
    rules: Vec<Box<dyn RuleProcessor>>,
    notifications: Vec<Box<dyn NotificationChannel>>,
}

impl MonitoringEngineBuilder {
    /// Create a new builder with default config
    pub fn new() -> Self {
        Self {
            config: EngineConfig::default(),
            datasources: Vec::new(),
            rules: Vec::new(),
            notifications: Vec::new(),
        }
    }

    /// Set custom engine configuration
    pub fn with_config(mut self, config: EngineConfig) -> Self {
        self.config = config;
        self
    }

    /// Add a datasource
    pub fn with_datasource(mut self, ds: Box<dyn DataSource>) -> Self {
        self.datasources.push(ds);
        self
    }

    /// Add a rule processor
    pub fn with_rule(mut self, rule: Box<dyn RuleProcessor>) -> Self {
        self.rules.push(rule);
        self
    }

    /// Add a notification channel
    pub fn with_notification(mut self, notif: Box<dyn NotificationChannel>) -> Self {
        self.notifications.push(notif);
        self
    }

    /// Build the engine
    pub fn build(self) -> MonitoringEngine {
        let mut engine = MonitoringEngine::new(self.config);
        for ds in self.datasources {
            engine.add_datasource(ds);
        }
        for rule in self.rules {
            engine.add_rule(rule);
        }
        for notif in self.notifications {
            engine.add_notification(notif);
        }
        engine
    }
}

impl Default for MonitoringEngineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_fluent_api() {
        let builder = MonitoringEngineBuilder::new()
            .with_config(EngineConfig::default());

        // Verify builder compiles and returns engine
        let _engine = builder.build();
    }
}
