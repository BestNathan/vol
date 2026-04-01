//! Alert manager with cooldown logic.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use vol_core::Alert;

/// Manages alert cooldowns to prevent alert spam
pub struct AlertManager {
    cooldown_secs: u64,
    last_alert_time: Arc<Mutex<HashMap<String, u64>>>,
}

impl AlertManager {
    pub fn new(cooldown_secs: u64) -> Self {
        Self {
            cooldown_secs,
            last_alert_time: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Check if an alert is in cooldown
    /// Returns true if the alert can be sent (not in cooldown)
    pub fn can_send(&self, alert: &Alert) -> bool {
        let key = format!("{}:{}:{}", alert.alert_type, alert.tenor, alert.symbol);

        let mut last_times = self.last_alert_time.lock().unwrap();
        let now = alert.timestamp;
        let cooldown_ms = self.cooldown_secs * 1000;

        let last_time = last_times.entry(key).or_insert(0);

        if now - *last_time >= cooldown_ms {
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
