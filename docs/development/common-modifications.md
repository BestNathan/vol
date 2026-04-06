# Common Modifications

This guide covers common modifications to the volatility monitoring system.

## Adding a New Alert Type

### Steps

1. **Create new handler struct** in `crates/vol-alert/src/your_alert.rs`:

```rust
use vol_core::{AlertHandler, Alert, VolatilityData};

pub struct YourAlertHandler {
    // Your configuration
    threshold: f64,
}

impl YourAlertHandler {
    pub fn new(threshold: f64) -> Self {
        Self { threshold }
    }
}

#[async_trait::async_trait]
impl AlertHandler for YourAlertHandler {
    async fn evaluate(&self, data: &VolatilityData) -> Vec<Alert> {
        // Your alert logic here
        vec![]
    }
}
```

2. **Implement `AlertHandler` trait** from `vol-core`

3. **Register in `crates/vol-monitor/src/main.rs`**:

```rust
use vol_alert::YourAlertHandler;

// In main():
let rule = YourAlertHandler::new(config.threshold);
builder = builder.with_rule(Box::new(rule));
```

## Adding a New Data Source

### Steps

1. **Implement `DataSource` trait** from `vol-core`:

```rust
use vol_core::{DataSource, MonitoringEvent, Result};
use tokio::sync::mpsc;

pub struct YourDataSource {
    // Your data source state
}

#[async_trait::async_trait]
impl DataSource for YourDataSource {
    fn id(&self) -> &str { "your-datasource" }
    fn event_type(&self) -> EventType { EventType::YourType }
    fn name(&self) -> &str { "your-datasource" }
    async fn connect(&mut self) -> Result<()> { Ok(()) }
    async fn run(&self, tx: mpsc::Sender<MonitoringEvent>) -> Result<()> {
        // Your data streaming logic
        Ok(())
    }
    async fn health_check(&self) -> HealthStatus { HealthStatus::Healthy }
    fn clone_box(&self) -> Box<dyn DataSource> { Box::new(self.clone()) }
}
```

2. **Add to `crates/vol-datasource/src/registry.rs`** (if using registry pattern)

3. **Register in `crates/vol-monitor/src/main.rs`**:

```rust
use vol_datasource::YourDataSource;

let ds = YourDataSource::new(config);
builder = builder.with_datasource(Box::new(ds));
```

## Adding a Notification Channel

### Steps

1. **Implement `NotificationHandler` trait** from `vol-core`:

```rust
use vol_core::{NotificationHandler, Alert, Result};

pub struct YourNotification {
    // Your configuration
}

impl YourNotification {
    pub fn new(config: YourConfig) -> Result<Self> {
        Ok(Self { /* ... */ })
    }
}

#[async_trait::async_trait]
impl NotificationHandler for YourNotification {
    fn name(&self) -> &str { "your-notification" }
    
    async fn send(&self, alert: &Alert) -> Result<()> {
        // Your notification logic
        Ok(())
    }
    
    fn clone_box(&self) -> Box<dyn NotificationHandler> {
        Box::new(self.clone())
    }
}
```

2. **Register in `crates/vol-monitor/src/main.rs`**:

```rust
use vol_notification::YourNotification;

let notif = YourNotification::new(config)?;
builder = builder.with_notification(Box::new(notif));
```

## Modifying Alert Thresholds

### Via Configuration File

Edit `config.toml` or use environment-specific configs:

```toml
[[rules]]
id = "absolute-iv-btc"
type = "absolute-iv"
symbol = "BTC"
short_threshold = 0.55      # Short tenor IV threshold
medium_threshold = 0.53     # Medium tenor IV threshold
long_threshold = 0.51       # Long tenor IV threshold
```

### Via Environment Variables (for sensitive values)

Some thresholds can be overridden via environment variables (if implemented).

## Changing Tenor Classification

Edit `config.toml`:

```toml
[tenors]
short_max_dte = 7       # Short: 0-7 days
medium_min_dte = 20     # Medium: 20-40 days
medium_max_dte = 40
long_min_dte = 80       # Long: 80-200 days
long_max_dte = 200
```

**Note:** Gaps between ranges (8-19, 41-79) are intentional - options in gap regions don't trigger tenor-based alerts.

## Modifying Cooldown Periods

### Global Cooldown

```toml
[engine]
alert_cooldown_secs = 300  # 5 minutes
```

### Tenor-Specific Cooldowns

```toml
[engine.tenor_cooldowns]
short_secs = 600      # 10 minutes
medium_secs = 3600    # 1 hour
long_secs = 14400     # 4 hours
```

## Enabling/Disabling Rules

In `config.toml`:

```toml
[[rules]]
id = "absolute-iv-btc"
type = "absolute-iv"
enabled = true  # Set to false to disable
```

## Enabling/Disabling Notifications

In `config.toml`:

```toml
[[notifications]]
id = "feishu-alerts"
type = "feishu"
enabled = true  # Set to false to disable
```
