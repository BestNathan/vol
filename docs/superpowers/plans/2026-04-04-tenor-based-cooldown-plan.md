# Tenor-Based Alert Cooldown Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement tenor-based configurable alert cooldowns where short/medium/long tenor alerts have separate cooldown periods.

**Architecture:** Add `TenorCooldownsConfig` to vol-config, extend `EngineConfigFile` with `get_cooldown_for_tenor()` method, integrate `AlertManager` into the monitoring engine with tenor-specific cooldown logic.

**Tech Stack:** Rust, Tokio async, vol-config, vol-alert, vol-engine crates.

---

### Task 1: Add TenorCooldownsConfig to vol-config

**Files:**
- Modify: `crates/vol-config/src/lib.rs`

**Current code - EngineConfigFile struct (lines 20-34):**
```rust
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct EngineConfigFile {
    #[serde(default)]
    pub hot_reload: bool,
    #[serde(default = "default_30")]
    pub hot_reload_interval_secs: u64,
    #[serde(default = "default_1000")]
    pub channel_buffer_size: usize,
    #[serde(default = "default_300")]
    pub alert_cooldown_secs: u64,
}

fn default_30() -> u64 { 30 }
fn default_1000() -> usize { 1000 }
fn default_300() -> u64 { 300 }
```

**Replace with:**
```rust
/// Tenor-specific cooldown configuration
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct TenorCooldownsConfig {
    #[serde(default)]
    pub short_secs: Option<u64>,
    #[serde(default)]
    pub medium_secs: Option<u64>,
    #[serde(default)]
    pub long_secs: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct EngineConfigFile {
    #[serde(default)]
    pub hot_reload: bool,
    #[serde(default = "default_30")]
    pub hot_reload_interval_secs: u64,
    #[serde(default = "default_1000")]
    pub channel_buffer_size: usize,
    #[serde(default = "default_300")]
    pub alert_cooldown_secs: u64,
    #[serde(default)]
    pub tenor_cooldowns: TenorCooldownsConfig,
}

fn default_30() -> u64 { 30 }
fn default_1000() -> usize { 1000 }
fn default_300() -> u64 { 300 }

impl EngineConfigFile {
    /// Get cooldown period for a specific tenor.
    /// Returns tenor-specific value if configured, otherwise falls back to global alert_cooldown_secs.
    pub fn get_cooldown_for_tenor(&self, tenor: Tenor) -> u64 {
        match tenor {
            Tenor::Short => self
                .tenor_cooldowns
                .short_secs
                .unwrap_or(self.alert_cooldown_secs),
            Tenor::Medium => self
                .tenor_cooldowns
                .medium_secs
                .unwrap_or(self.alert_cooldown_secs),
            Tenor::Long => self
                .tenor_cooldowns
                .long_secs
                .unwrap_or(self.alert_cooldown_secs),
        }
    }
}
```

- [ ] **Step 1: Read current lib.rs**

Run:
```bash
cat crates/vol-config/src/lib.rs
```

- [ ] **Step 2: Add TenorCooldownsConfig struct**

Add the new struct before `EngineConfigFile`.

- [ ] **Step 3: Update EngineConfigFile**

Add `tenor_cooldowns: TenorCooldownsConfig` field with `#[serde(default)]`.

- [ ] **Step 4: Add get_cooldown_for_tenor() method**

Add the `impl EngineConfigFile` block with the method.

- [ ] **Step 5: Add unit tests**

Add test module:
```rust
#[cfg(test)]
mod tenor_cooldown_tests {
    use super::*;
    use vol_core::Tenor;

    #[test]
    fn test_get_cooldown_for_tenor_uses_specific_value() {
        let config = EngineConfigFile {
            alert_cooldown_secs: 300,
            tenor_cooldowns: TenorCooldownsConfig {
                short_secs: Some(600),
                medium_secs: Some(3600),
                long_secs: Some(14400),
            },
            ..Default::default()
        };

        assert_eq!(config.get_cooldown_for_tenor(Tenor::Short), 600);
        assert_eq!(config.get_cooldown_for_tenor(Tenor::Medium), 3600);
        assert_eq!(config.get_cooldown_for_tenor(Tenor::Long), 14400);
    }

    #[test]
    fn test_get_cooldown_for_tenor_fallback_to_global() {
        let config = EngineConfigFile {
            alert_cooldown_secs: 300,
            tenor_cooldowns: TenorCooldownsConfig {
                short_secs: Some(600),
                medium_secs: None,  // Should fallback
                long_secs: None,    // Should fallback
            },
            ..Default::default()
        };

        assert_eq!(config.get_cooldown_for_tenor(Tenor::Short), 600);
        assert_eq!(config.get_cooldown_for_tenor(Tenor::Medium), 300);
        assert_eq!(config.get_cooldown_for_tenor(Tenor::Long), 300);
    }

    #[test]
    fn test_get_cooldown_for_tenor_all_default() {
        let config = EngineConfigFile {
            alert_cooldown_secs: 300,
            tenor_cooldowns: TenorCooldownsConfig::default(),
            ..Default::default()
        };

        // All should fallback to global
        assert_eq!(config.get_cooldown_for_tenor(Tenor::Short), 300);
        assert_eq!(config.get_cooldown_for_tenor(Tenor::Medium), 300);
        assert_eq!(config.get_cooldown_for_tenor(Tenor::Long), 300);
    }
}
```

- [ ] **Step 6: Run tests**

Run:
```bash
cargo test -p vol-config tenor_cooldown_tests
```
Expected: All 3 tests pass

- [ ] **Step 7: Commit**

Run:
```bash
git add crates/vol-config/src/lib.rs
git commit -m "feat: add tenor-specific cooldown configuration"
```

### Task 2: Update AlertManager to use EngineConfigFile

**Files:**
- Modify: `crates/vol-alert/src/manager.rs`

**Current code:**
```rust
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
    // ... rest of impl
}
```

**Replace with:**
```rust
//! Alert manager with cooldown logic.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use vol_core::{Alert, Tenor};
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

#[cfg(test)]
mod tests {
    use super::*;
    use vol_core::{Alert, AlertType, OptionType};

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

        // Same short alert immediately - should be blocked (only 600s cooldown)
        let short_alert2 = create_test_alert(Tenor::Short, 1000 + 500 * 1000);
        assert!(!manager.can_send(&short_alert2));

        // Medium tenor alert - should pass (different tenor, different cooldown key)
        let medium_alert = create_test_alert(Tenor::Medium, 1000);
        assert!(manager.can_send(&medium_alert));

        // Long tenor after 10 min - should pass (14400s cooldown, only 600s elapsed)
        let long_alert = create_test_alert(Tenor::Long, 1000 + 600 * 1000);
        assert!(manager.can_send(&long_alert));
    }
}
```

- [ ] **Step 1: Read current manager.rs**

Run:
```bash
cat crates/vol-alert/src/manager.rs
```

- [ ] **Step 2: Update imports**

Add `use vol_config::EngineConfigFile;` and `use vol_core::Tenor;`.

- [ ] **Step 3: Update AlertManager struct**

Change `cooldown_secs: u64` to `config: EngineConfigFile`.

- [ ] **Step 4: Update constructor**

Change `new(cooldown_secs: u64)` to `new(config: EngineConfigFile)`.

- [ ] **Step 5: Update can_send method**

Replace `let cooldown_ms = self.cooldown_secs * 1000;` with:
```rust
let cooldown_secs = self.config.get_cooldown_for_tenor(alert.tenor);
let cooldown_ms = cooldown_secs * 1000;
```

- [ ] **Step 6: Add unit tests**

Add the test module shown above.

- [ ] **Step 7: Run tests**

Run:
```bash
cargo test -p vol-alert
```
Expected: All tests pass including new `test_tenor_based_cooldown`

- [ ] **Step 8: Commit**

Run:
```bash
git add crates/vol-alert/src/manager.rs
git commit -m "feat: update AlertManager to use tenor-specific cooldowns"
```

### Task 3: Integrate AlertManager into Monitoring Engine

**Files:**
- Modify: `crates/vol-engine/src/engine.rs`
- Modify: `crates/vol-engine/src/builder.rs`
- Modify: `crates/vol-engine/src/lib.rs`

**Changes to engine.rs:**

The engine needs to use AlertManager for cooldown checking before sending alerts to notifications.

**Current spawn_notifications (lines 152-180):**
```rust
fn spawn_notifications(
    &self,
    mut alert_rx: mpsc::Receiver<Alert>,
) -> Vec<JoinHandle<Result<()>>> {
    // For notifications, we use a fan-out pattern where each notification channel
    // runs in the same task to avoid needing mpsc resubscribe
    let notifications: Vec<Box<dyn NotificationHandler>> = self.notifications
        .iter()
        .filter(|n| n.is_enabled())
        .map(|n| n.clone_box())
        .collect();

    if notifications.is_empty() {
        return vec![];
    }

    let num_notifications = notifications.len();
    vec![tokio::spawn(async move {
        info!("Starting {} notification channels", num_notifications);
        while let Some(alert) = alert_rx.recv().await {
            for notif in &notifications {
                if let Err(e) = notif.send(&alert).await {
                    error!("Notification {} failed: {}", notif.name(), e);
                }
            }
        }
        Ok(())
    })]
}
```

**Replace with:**
```rust
fn spawn_notifications(
    &self,
    mut alert_rx: mpsc::Receiver<Alert>,
    alert_manager: AlertManager,
) -> Vec<JoinHandle<Result<()>>> {
    // For notifications, we use a fan-out pattern where each notification channel
    // runs in the same task to avoid needing mpsc resubscribe
    let notifications: Vec<Box<dyn NotificationHandler>> = self.notifications
        .iter()
        .filter(|n| n.is_enabled())
        .map(|n| n.clone_box())
        .collect();

    if notifications.is_empty() {
        return vec![];
    }

    let num_notifications = notifications.len();
    let alert_manager = Arc::new(alert_manager);
    vec![tokio::spawn(async move {
        info!("Starting {} notification channels", num_notifications);
        while let Some(alert) = alert_rx.recv().await {
            // Check cooldown before sending
            if !alert_manager.can_send(&alert) {
                tracing::debug!("Alert in cooldown, skipping: {}:{}:{}", 
                    alert.alert_type, alert.tenor, alert.symbol);
                continue;
            }
            for notif in &notifications {
                if let Err(e) = notif.send(&alert).await {
                    error!("Notification {} failed: {}", notif.name(), e);
                }
            }
        }
        Ok(())
    })]
}
```

**Update build() method signature to accept AlertManager:**

- [ ] **Step 1: Read current engine.rs**

Run:
```bash
cat crates/vol-engine/src/engine.rs
```

- [ ] **Step 2: Add AlertManager import**

Add to top of file:
```rust
use vol_alert::AlertManager;
```

- [ ] **Step 3: Update spawn_notifications signature**

Add `alert_manager: AlertManager` parameter.

- [ ] **Step 4: Add cooldown check in notification loop**

Add the `alert_manager.can_send(&alert)` check before sending.

- [ ] **Step 5: Update MonitoringEngineBuilder**

The builder needs to accept and pass AlertManager.

Modify `crates/vol-engine/src/builder.rs`:

**Add import:**
```rust
use vol_alert::AlertManager;
```

**Add field to struct:**
```rust
pub struct MonitoringEngineBuilder {
    config: EngineConfig,
    datasources: Vec<Box<dyn DataSource>>,
    rules: Vec<Box<dyn RuleProcessor>>,
    notifications: Vec<Box<dyn NotificationHandler>>,
    alert_manager: Option<AlertManager>,  // Add this
}
```

**Update new():**
```rust
pub fn new() -> Self {
    Self {
        config: EngineConfig::default(),
        datasources: Vec::new(),
        rules: Vec::new(),
        notifications: Vec::new(),
        alert_manager: None,  // Add this
    }
}
```

**Add method:**
```rust
pub fn with_alert_manager(mut self, alert_manager: AlertManager) -> Self {
    self.alert_manager = Some(alert_manager);
    self
}
```

**Update build():**

Need to pass alert_manager to spawn_notifications. This requires updating the MonitoringEngine to store and use it.

Actually, simpler approach: pass the EngineConfigFile to the engine and create AlertManager there.

Let me revise: The engine should receive EngineConfigFile from config and create AlertManager internally.

- [ ] **Step 6: Add engine_config_file field to EngineConfig**

Modify `crates/vol-engine/src/config.rs`:

Add:
```rust
use vol_config::EngineConfigFile;

#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub event_buffer_size: usize,
    pub alert_buffer_size: usize,
    pub enable_backpressure: bool,
    pub config_file: EngineConfigFile,  // Add this
}
```

Update Default:
```rust
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
```

- [ ] **Step 7: Update engine.rs to create AlertManager**

In `MonitoringEngine::new()`:
```rust
pub fn new(config: EngineConfig) -> Self {
    Self {
        datasources: Vec::new(),
        rules: Vec::new(),
        notifications: Vec::new(),
        config,
    }
}
```

Then in `run()`, create AlertManager and pass to spawn_notifications:
```rust
let alert_manager = AlertManager::new(self.config.config_file.clone());
let notif_handles = self.spawn_notifications(alert_rx, alert_manager);
```

- [ ] **Step 8: Run build to check for errors**

Run:
```bash
cargo build -p vol-engine
```
Expected: Compiles successfully

- [ ] **Step 9: Commit**

Run:
```bash
git add crates/vol-engine/
git commit -m "feat: integrate AlertManager with tenor-based cooldowns into engine"
```

### Task 4: Update vol-monitor main.rs

**Files:**
- Modify: `crates/vol-monitor/src/main.rs`

**Current code (lines 171-177):**
```rust
fn create_default_config() -> Config {
    Config {
        engine: vol_config::EngineConfigFile {
            hot_reload: false,
            hot_reload_interval_secs: 30,
            channel_buffer_size: 1000,
            alert_cooldown_secs: 300,
        },
        // ...
```

**Update to include tenor_cooldowns:**

```rust
fn create_default_config() -> Config {
    Config {
        engine: vol_config::EngineConfigFile {
            hot_reload: false,
            hot_reload_interval_secs: 30,
            channel_buffer_size: 1000,
            alert_cooldown_secs: 300,
            tenor_cooldowns: vol_config::TenorCooldownsConfig {
                short_secs: Some(600),    // 10 minutes
                medium_secs: Some(3600),  // 1 hour
                long_secs: Some(14400),   // 4 hours
            },
        },
        // ...
```

- [ ] **Step 1: Update EngineConfig in main.rs**

The engine config needs to include the config_file. Find where EngineConfig is created:

```rust
let engine_config = EngineConfig::default();
```

Replace with:
```rust
let engine_config = EngineConfig {
    event_buffer_size: config.engine.channel_buffer_size,
    alert_buffer_size: 100,
    enable_backpressure: true,
    config_file: config.engine.clone(),
};
```

- [ ] **Step 2: Update create_default_config**

Add tenor_cooldowns as shown above.

- [ ] **Step 3: Build and test**

Run:
```bash
cargo build --release
```
Expected: Compiles successfully

- [ ] **Step 4: Commit**

Run:
```bash
git add crates/vol-monitor/src/main.rs
git commit -m "feat: configure tenor cooldowns in main.rs"
```

### Task 5: Update config.toml example

**Files:**
- Modify: `config.toml`

**Current [engine] section (lines 3-7):**
```toml
[engine]
hot_reload = true
hot_reload_interval_secs = 30
channel_buffer_size = 1000
alert_cooldown_secs = 300
```

**Replace with:**
```toml
[engine]
hot_reload = true
hot_reload_interval_secs = 30
channel_buffer_size = 1000
alert_cooldown_secs = 300  # Global fallback

# Tenor-specific cooldowns (override global)
[engine.tenor_cooldowns]
short_secs = 600    # 10 minutes - short tenor alerts
medium_secs = 3600  # 1 hour - medium tenor alerts
long_secs = 14400   # 4 hours - long tenor alerts
```

- [ ] **Step 1: Read current config.toml**

Run:
```bash
cat config.toml
```

- [ ] **Step 2: Add tenor_cooldowns section**

Add the `[engine.tenor_cooldowns]` section after `alert_cooldown_secs`.

- [ ] **Step 3: Validate config parsing**

Run:
```bash
cargo run --release 2>&1 | head -30
```
Expected: Starts successfully with new config

- [ ] **Step 4: Commit**

Run:
```bash
git add config.toml
git commit -m "docs: add tenor cooldown configuration example"
```

### Task 6: Test tenor-based cooldown behavior

**Files:**
- No file changes - integration testing

- [ ] **Step 1: Create test configuration**

Create a test config with short cooldowns for quick testing:
```toml
[engine]
hot_reload = false
alert_cooldown_secs = 60

[engine.tenor_cooldowns]
short_secs = 10     # 10 seconds for testing
medium_secs = 30    # 30 seconds
long_secs = 60      # 60 seconds

[tenors]
short_max_dte = 7
medium_min_dte = 20
medium_max_dte = 40
long_min_dte = 80

[[datasources]]
id = "deribit"
provider = "deribit"
ws_url = "wss://www.deribit.com/ws/api/v2"
symbols = ["BTC"]
enabled = true

[datasources.auth]
client_id = "nhXng7Bj"
client_secret = "OxCGY10HlzgKfRoXPBRQqg5IBQcZguGPhE1tewP5U3Y"

[[rules]]
id = "absolute-iv-btc"
type = "absolute-iv"
symbol = "BTC"
short_threshold = 0.55
medium_threshold = 0.53
long_threshold = 0.51
short_atm_threshold = 0.05
medium_atm_threshold = 0.08
long_atm_threshold = 0.10
enabled = true
notifications = ["stdout"]

[[notifications]]
id = "stdout"
type = "stdout"
enabled = true
```

- [ ] **Step 2: Run monitor with test config**

Run:
```bash
HTTPS_PROXY=http://192.168.2.98:8890 RUST_LOG=info ./target/release/vol-monitor --config /tmp/test_cooldown.toml
```

- [ ] **Step 3: Verify cooldown behavior in logs**

Watch for:
- First short-tenor alert should be sent
- Subsequent short-tenor alerts within 10s should be suppressed
- Medium-tenor alerts should have 30s cooldown
- Long-tenor alerts should have 60s cooldown

- [ ] **Step 4: Verify log messages**

Look for debug messages about cooldown:
```bash
kubectl -n deribit logs deployment/vol-monitor --since=1h | grep -i cooldown
```

- [ ] **Step 5: Document test results**

Note whether cooldown behavior matches expected tenor-specific values.

---

## Testing Summary

After all tasks complete:

```bash
# Unit tests
cargo test -p vol-config
cargo test -p vol-alert

# Integration test
cargo build --release
./target/release/vol-monitor --config config.toml

# Verify in logs
kubectl -n deribit logs deployment/vol-monitor | grep -E "(ALERT|cooldown)"
```

## Configuration Examples

**Default (fallback to global):**
```toml
[engine]
alert_cooldown_secs = 300  # 5 minutes for all
```

**Tenor-specific:**
```toml
[engine]
alert_cooldown_secs = 300  # Fallback

[engine.tenor_cooldowns]
short_secs = 600    # 10 minutes
medium_secs = 3600  # 1 hour
long_secs = 14400   # 4 hours
```

**Partial override:**
```toml
[engine]
alert_cooldown_secs = 300  # Fallback

[engine.tenor_cooldowns]
short_secs = 600    # Only short is different, others use 300s
```
