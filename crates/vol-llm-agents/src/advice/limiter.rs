//! Frequency limiter for alert analysis.
//!
//! Prevents over-analysis by limiting:
//! - Per (symbol, alert_type) cooldown (default 5 min)
//! - Global hourly limit (default 20/hour)

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use vol_core::Alert;

#[cfg(test)]
use vol_core::{AlertType, OptionType, Tenor};

/// Creates a unique key for rate limiting
pub fn limiter_key(alert: &Alert) -> String {
    format!("{}:{}", alert.symbol, alert.alert_type)
}

/// Frequency limiter
#[derive(Clone)]
pub struct FrequencyLimiter {
    cooldown_secs: u64,
    max_per_hour: u32,
    last_analysis: Arc<Mutex<HashMap<String, u64>>>,
    hourly_count: Arc<AtomicU32>,
    hour_start: Arc<Mutex<u64>>,
}

impl FrequencyLimiter {
    /// Create a new frequency limiter
    pub fn new(cooldown_secs: u64, max_per_hour: u32) -> Self {
        Self {
            cooldown_secs,
            max_per_hour,
            last_analysis: Arc::new(Mutex::new(HashMap::new())),
            hourly_count: Arc::new(AtomicU32::new(0)),
            hour_start: Arc::new(Mutex::new(0)),
        }
    }

    /// Check if analysis is allowed for this alert
    pub fn can_analyze(&self, alert: &Alert) -> bool {
        let key = limiter_key(alert);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Check cooldown
        {
            let last = self.last_analysis.lock().unwrap();
            if let Some(&last_time) = last.get(&key) {
                if now - last_time < self.cooldown_secs {
                    return false;
                }
            }
        }

        // Check hourly limit
        let mut hour_start = self.hour_start.lock().unwrap();
        let current_hour = now / 3600;

        if *hour_start != current_hour {
            // New hour, reset counter
            *hour_start = current_hour;
            self.hourly_count.store(0, Ordering::SeqCst);
        }

        self.hourly_count.load(Ordering::SeqCst) < self.max_per_hour
    }

    /// Record an analysis
    pub fn record_analysis(&self, alert: &Alert) {
        let key = limiter_key(alert);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Update last analysis time
        {
            let mut last = self.last_analysis.lock().unwrap();
            last.insert(key, now);
        }

        // Increment hourly count
        let mut hour_start = self.hour_start.lock().unwrap();
        let current_hour = now / 3600;

        if *hour_start != current_hour {
            *hour_start = current_hour;
            self.hourly_count.store(0, Ordering::SeqCst);
        }

        self.hourly_count.fetch_add(1, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_alert(symbol: &str, alert_type: AlertType) -> Alert {
        Alert {
            alert_type,
            tenor: Tenor::Short,
            symbol: symbol.to_string(),
            iv: 0.5,
            message: "test".to_string(),
            timestamp: 0,
            source: "test".to_string(),
            index_price: 50000.0,
            dte: 30,
            option_type: OptionType::Call,
            moneyness: 1.0,
            mark_price_coin: 0.05,
            trace_id: "test-trace".to_string(),
        }
    }

    #[test]
    fn test_first_analysis_allowed() {
        let limiter = FrequencyLimiter::new(300, 20);
        let alert = create_test_alert("BTC", AlertType::AbsoluteIv { threshold: 0.4 });
        assert!(limiter.can_analyze(&alert));
    }

    #[test]
    fn test_cooldown_blocks_analysis() {
        let limiter = FrequencyLimiter::new(300, 20);
        let alert = create_test_alert("BTC", AlertType::AbsoluteIv { threshold: 0.4 });

        // First analysis allowed
        assert!(limiter.can_analyze(&alert));
        limiter.record_analysis(&alert);

        // Second analysis blocked by cooldown
        assert!(!limiter.can_analyze(&alert));
    }

    #[test]
    fn test_different_symbols_independent() {
        let limiter = FrequencyLimiter::new(300, 20);
        let btc_alert = create_test_alert("BTC", AlertType::AbsoluteIv { threshold: 0.4 });
        let eth_alert = create_test_alert("ETH", AlertType::AbsoluteIv { threshold: 0.4 });

        // Both should be allowed (different symbols)
        assert!(limiter.can_analyze(&btc_alert));
        assert!(limiter.can_analyze(&eth_alert));
    }

    #[test]
    fn test_hourly_limit() {
        let limiter = FrequencyLimiter::new(0, 2); // No cooldown, max 2/hour

        let alert1 = create_test_alert("BTC", AlertType::AbsoluteIv { threshold: 0.4 });
        let alert2 = create_test_alert("ETH", AlertType::AbsoluteIv { threshold: 0.4 });
        let alert3 = create_test_alert("SOL", AlertType::AbsoluteIv { threshold: 0.4 });

        // First two should be allowed
        assert!(limiter.can_analyze(&alert1));
        limiter.record_analysis(&alert1);
        assert!(limiter.can_analyze(&alert2));
        limiter.record_analysis(&alert2);

        // Third should be blocked by hourly limit
        assert!(!limiter.can_analyze(&alert3));
    }
}
