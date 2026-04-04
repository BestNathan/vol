//! Alert manager with cooldown logic.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use vol_core::Alert;
use vol_config::EngineConfigFile;

/// Manages alert cooldowns to prevent alert spam
pub struct AlertManager {
    config: EngineConfigFile,
    last_alert_time: Arc<Mutex<HashMap<String, u64>>>,
}

impl AlertManager {
    pub fn new(config: EngineConfigFile) -> Self {
        Self {
            config,
            last_alert_time: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Check if an alert is in cooldown
    /// Returns true if the alert can be sent (not in cooldown)
    pub fn can_send(&self, alert: &Alert) -> bool {
        let key = format!("{}:{}:{}", alert.alert_type, alert.tenor, alert.symbol);

        let mut last_times = self.last_alert_time.lock().unwrap();
        let now = alert.timestamp;
        let cooldown_secs = self.config.get_cooldown_for_tenor(alert.tenor);
        let cooldown_ms = cooldown_secs * 1000;

        let last_time = last_times.entry(key).or_insert(0);

        // First alert (last_time == 0) always passes
        if *last_time == 0 || now - *last_time >= cooldown_ms {
            *last_time = now;
            true
        } else {
            false
        }
    }

    /// Load last alert times from state (called during startup)
    pub fn load_state(&self, state: HashMap<String, u64>) {
        let mut last_times = self.last_alert_time.lock().unwrap();
        *last_times = state;
    }

    /// Get current state for persistence
    pub fn get_state(&self) -> HashMap<String, u64> {
        self.last_alert_time.lock().unwrap().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vol_core::{Alert, AlertType, OptionType, Tenor};
    use vol_config::{EngineConfigFile, TenorCooldownsConfig};

    fn create_test_alert(tenor: Tenor, timestamp: u64) -> Alert {
        Alert::new(
            AlertType::AbsoluteIv { threshold: 0.5 },
            tenor,
            "BTC-29MAR24-70000-C".to_string(),
            0.6,
            "Test alert".to_string(),
            timestamp,
            "test".to_string(),
            50000.0,
            30,
            OptionType::Call,
            0.1,
            0.02,
        )
    }

    #[test]
    fn test_tenor_based_cooldown() {
        let config = EngineConfigFile {
            alert_cooldown_secs: 300,
            tenor_cooldowns: TenorCooldownsConfig {
                short_secs: Some(600),    // 10 min
                medium_secs: Some(3600),  // 1 hour
                long_secs: Some(14400),   // 4 hours
            },
            ..Default::default()
        };

        let manager = AlertManager::new(config);

        // Short tenor alert - should pass first time
        let short_alert = create_test_alert(Tenor::Short, 1000);
        assert!(manager.can_send(&short_alert));

        // Same short alert within cooldown - should be blocked
        let short_alert2 = create_test_alert(Tenor::Short, 1000 + 500 * 1000);
        assert!(!manager.can_send(&short_alert2));

        // Medium tenor alert - should pass (different key)
        let medium_alert = create_test_alert(Tenor::Medium, 1000);
        assert!(manager.can_send(&medium_alert));
    }
}
