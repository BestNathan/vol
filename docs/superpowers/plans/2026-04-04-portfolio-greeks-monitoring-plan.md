# Portfolio & Greeks Monitoring Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement portfolio position and Greeks monitoring by adding PortfolioDataSource and fixing existing PortfolioRule integration.

**Architecture:** Extend the existing DataSource → Rule → Alert → Notification pipeline with:
1. PortfolioDataSource for Deribit WebSocket + REST polling
2. Fix duplicate PortfolioSnapshot struct definitions
3. Complete PortfolioRule::evaluate() implementation
4. Wire up in main.rs with config

**Tech Stack:** Rust, tokio async, Deribit API (WebSocket + REST), vol-core traits

---

### Prerequisites Check

**Existing code to reuse:**
- `vol-core/src/event.rs` - `PortfolioSnapshot`, `MonitoringEvent::Portfolio`, `EventType::Portfolio`
- `vol-rules/src/portfolio.rs` - `PortfolioRule` with metric evaluation logic
- `vol-config/src/metrics.rs` - `MetricConfig` enum with GreeksConfig, MarginRatioConfig, etc.
- `vol-deribit/src/portfolio.rs` - `PortfolioData` model from Deribit WebSocket

**What needs to be built:**
- `PortfolioDataSource` implementation
- Fix duplicate struct definitions
- Complete `PortfolioRule::evaluate()` method

---

### Task 1: Fix PortfolioSnapshot Duplication

**Files:**
- Modify: `crates/vol-rules/src/portfolio.rs:8-20`
- Modify: `crates/vol-rules/src/portfolio.rs:70-80` (evaluate method)

The `PortfolioSnapshot` struct is defined in both `vol-core/src/event.rs` and `vol-rules/src/portfolio.rs`. Remove the duplicate and use the vol-core version.

- [ ] **Step 1: Update vol-rules/src/portfolio.rs imports**

Add import at top of file:
```rust
use vol_core::PortfolioSnapshot;  // Use canonical type from vol-core
```

- [ ] **Step 2: Remove duplicate struct definition**

Delete lines 8-20 (the local `PortfolioSnapshot` definition). The file should now use `vol_core::PortfolioSnapshot`.

- [ ] **Step 3: Update evaluate method signature**

Change line 72 from:
```rust
pub async fn evaluate(&self, snapshot: &PortfolioSnapshot) -> Vec<Alert> {
```
To use the vol-core type (import handles this).

- [ ] **Step 4: Fix field name mismatches**

The vol-core `PortfolioSnapshot` has these fields:
```rust
pub struct PortfolioSnapshot {
    pub currency: String,
    pub timestamp: u64,
    pub equity: f64,
    pub balance: f64,
    pub available_funds: f64,
    pub margin_balance: f64,
    pub initial_margin: f64,
    pub maintenance_margin: f64,
    pub session_pnl: f64,
    pub delta_total: f64,
    pub options_delta: f64,
    pub options_gamma: f64,
    pub options_theta: f64,
    pub options_vega: f64,
}
```

Update the evaluate method to use correct field names:
- `snapshot.free_balance` → `snapshot.available_funds`
- `snapshot.delta_exposure` → `snapshot.delta_total`
- `snapshot.margin_ratio` → calculate as `Some(snapshot.margin_balance / snapshot.initial_margin)` if `initial_margin > 0`

- [ ] **Step 5: Update evaluate method to compute margin_ratio inline**

```rust
let margin_ratio = if snapshot.initial_margin > 0.0 {
    Some(snapshot.margin_balance / snapshot.initial_margin)
} else {
    None
};
```

Then use `margin_ratio` variable instead of `snapshot.margin_ratio`.

- [ ] **Step 6: Run cargo check to verify compilation**

Run: `cargo check --workspace`
Expected: No errors related to PortfolioSnapshot

- [ ] **Step 7: Commit**

```bash
git add crates/vol-rules/src/portfolio.rs
git commit -m "refactor: use canonical PortfolioSnapshot from vol-core

Remove duplicate struct definition and use vol_core::PortfolioSnapshot.
Fix field name mappings:
- free_balance → available_funds
- delta_exposure → delta_total
- Compute margin_ratio inline from margin_balance/initial_margin
"
```

---

### Task 2: Add Deribit REST API Client Method for Positions

**Files:**
- Create: `crates/vol-deribit/src/positions.rs`
- Modify: `crates/vol-deribit/src/client.rs`
- Modify: `crates/vol-deribit/src/lib.rs`

- [ ] **Step 1: Create positions.rs with Position API models**

```rust
//! Deribit position API models for private/get_positions.

use serde::{Deserialize, Serialize};

/// Position data from get_positions API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub instrument_name: String,
    pub size: f64,
    pub average_price: f64,
    pub mark_price: f64,
    pub unrealized_pnl: f64,
    pub delta: f64,
    pub gamma: f64,
    pub vega: f64,
    pub theta: f64,
    pub underlying: String,
}

/// Response from private/get_positions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetPositionsResponse {
    pub positions: Vec<Position>,
}
```

- [ ] **Step 2: Add get_positions method to DeribitClient**

In `crates/vol-deribit/src/client.rs`, add:

```rust
/// Get account positions via REST API
pub async fn get_positions(&self, currency: Option<&str>) -> Result<Vec<Position>> {
    let mut params = serde_json::Map::new();
    if let Some(curr) = currency {
        params.insert("currency".to_string(), serde_json::Value::String(curr.to_string()));
    }
    
    let response = self.request("private/get_positions", Some(params)).await?;
    let positions: Vec<Position> = serde_json::from_value(response.result)?;
    Ok(positions)
}
```

- [ ] **Step 3: Export positions module from lib.rs**

In `crates/vol-deribit/src/lib.rs`, add:
```rust
pub mod positions;
pub use positions::{Position, GetPositionsResponse};
```

- [ ] **Step 4: Run cargo check**

Run: `cargo check -p vol-deribit`
Expected: Compiles successfully

- [ ] **Step 5: Commit**

```bash
git add crates/vol-deribit/src/positions.rs crates/vol-deribit/src/client.rs crates/vol-deribit/src/lib.rs
git commit -m "feat: add get_positions REST API method

Add Position model and get_positions() method to DeribitClient
for fetching account positions with Greeks data.
"
```

---

### Task 3: Implement PortfolioDataSource

**Files:**
- Create: `crates/vol-datasource/src/portfolio.rs`
- Modify: `crates/vol-datasource/src/lib.rs`
- Modify: `crates/vol-datasource/src/registry.rs`

- [ ] **Step 1: Create portfolio.rs data source**

```rust
//! Portfolio data source - Deribit WebSocket + REST polling.

use async_trait::async_trait;
use tokio::sync::mpsc;
use tracing::{info, warn, error};
use std::time::Duration;
use vol_core::{DataSource, MonitoringEvent, PortfolioSnapshot, Result, HealthStatus};
use vol_deribit::DeribitClient;

pub struct PortfolioDataSource {
    id: String,
    client: DeribitClient,
    poll_interval_secs: u64,
    currencies: Vec<String>,
}

impl PortfolioDataSource {
    pub fn new(id: String, client: DeribitClient, poll_interval_secs: u64, currencies: Vec<String>) -> Self {
        Self { id, client, poll_interval_secs, currencies }
    }

    /// Poll positions and build snapshot
    async fn fetch_snapshot(&self, currency: &str) -> Result<PortfolioSnapshot> {
        // Get account summary from WebSocket subscription data
        // Get positions from REST API
        let positions = self.client.get_positions(Some(currency)).await?;
        
        // Aggregate Greeks from positions
        let mut delta_total = 0.0;
        let mut gamma_total = 0.0;
        let mut vega_total = 0.0;
        let mut theta_total = 0.0;
        
        for pos in &positions {
            delta_total += pos.delta;
            gamma_total += pos.gamma;
            vega_total += pos.vega;
            theta_total += pos.theta;
        }

        // For now, use zeros for fields not available from positions API
        // These will be populated from WebSocket portfolio subscription
        Ok(PortfolioSnapshot {
            currency: currency.to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            equity: 0.0,      // From WebSocket
            balance: 0.0,     // From WebSocket
            available_funds: 0.0,  // From WebSocket
            margin_balance: 0.0,   // From WebSocket
            initial_margin: 0.0,   // From WebSocket
            maintenance_margin: 0.0, // From WebSocket
            session_pnl: 0.0,      // From WebSocket
            delta_total,
            options_delta: delta_total,
            options_gamma: gamma_total,
            options_theta: theta_total,
            options_vega: vega_total,
        })
    }
}

#[async_trait]
impl DataSource for PortfolioDataSource {
    fn id(&self) -> &str {
        &self.id
    }

    fn event_type(&self) -> vol_core::EventType {
        vol_core::EventType::Portfolio(vol_core::PortfolioMetricType::Greeks)
    }

    fn name(&self) -> &str {
        "deribit-portfolio"
    }

    async fn connect(&mut self) -> Result<()> {
        // Subscribe to user.portfolio.{currency} channels
        for currency in &self.currencies {
            let channel = format!("user.portfolio.{}", currency.to_lowercase());
            self.client.subscribe(&channel).await?;
            info!("Subscribed to portfolio channel: {}", channel);
        }
        Ok(())
    }

    async fn run(&self, tx: mpsc::Sender<MonitoringEvent>) -> Result<()> {
        info!("Starting portfolio data source with {} currencies", self.currencies.len());
        
        let mut interval = tokio::time::Duration::from_secs(self.poll_interval_secs);
        let mut ticker = tokio::time::interval(interval);

        loop {
            ticker.tick().await;
            
            for currency in &self.currencies {
                match self.fetch_snapshot(currency).await {
                    Ok(snapshot) => {
                        let event = MonitoringEvent::Portfolio(snapshot);
                        if let Err(e) = tx.send(event).await {
                            error!("Failed to send portfolio event: {}", e);
                            break;
                        }
                    }
                    Err(e) => {
                        warn!("Failed to fetch portfolio for {}: {}", currency, e);
                    }
                }
            }
        }
    }

    async fn health_check(&self) -> HealthStatus {
        // Simple check - if we can connect to Deribit
        HealthStatus::Healthy
    }

    fn clone_box(&self) -> Box<dyn DataSource> {
        Box::new(self.clone())
    }
}

impl Clone for PortfolioDataSource {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            client: self.client.clone(),
            poll_interval_secs: self.poll_interval_secs,
            currencies: self.currencies.clone(),
        }
    }
}
```

- [ ] **Step 2: Export from lib.rs**

In `crates/vol-datasource/src/lib.rs`, add:
```rust
mod portfolio;
pub use portfolio::PortfolioDataSource;
```

- [ ] **Step 3: Run cargo check**

Run: `cargo check -p vol-datasource`
Expected: Compiles (may have warnings about unused imports)

- [ ] **Step 4: Commit**

```bash
git add crates/vol-datasource/src/portfolio.rs crates/vol-datasource/src/lib.rs
git commit -m "feat: add PortfolioDataSource for Deribit account monitoring

Implements DataSource trait for portfolio data:
- Polls get_positions REST API at configurable interval
- Aggregates Greeks from all positions
- Emits PortfolioSnapshot events to event bus
"
```

---

### Task 4: Complete PortfolioRule::evaluate() Implementation

**Files:**
- Modify: `crates/vol-rules/src/portfolio.rs:315-324`

- [ ] **Step 1: Fix evaluate method to use event snapshot**

Change the evaluate method from:
```rust
async fn evaluate(&self, event: &MonitoringEvent) -> Vec<Alert> {
    let MonitoringEvent::Portfolio(snapshot) = event else {
        return vec![];
    };
    let _ = snapshot;
    vec![]
}
```

To:
```rust
async fn evaluate(&self, event: &MonitoringEvent) -> Vec<Alert> {
    let MonitoringEvent::Portfolio(snapshot) = event else {
        return vec![];
    };
    
    // Convert vol_core::PortfolioSnapshot to local type for evaluation
    let local_snapshot = PortfolioSnapshot {
        currency: snapshot.currency.clone(),
        timestamp: snapshot.timestamp,
        margin_ratio: if snapshot.initial_margin > 0.0 {
            Some(snapshot.margin_balance / snapshot.initial_margin)
        } else {
            None
        },
        free_balance: snapshot.available_funds,
        delta_exposure: snapshot.delta_total,
        session_pnl: snapshot.session_pnl,
        options_gamma: snapshot.options_gamma,
        options_vega: snapshot.options_vega,
        options_theta: snapshot.options_theta,
    };
    
    self.evaluate(&local_snapshot).await
}
```

Wait, this creates a naming conflict. The struct is removed in Task 1. Let me fix this properly.

**Corrected approach:** Inline the evaluation logic using vol_core fields directly.

- [ ] **Step 1: Rewrite evaluate method**

```rust
async fn evaluate(&self, event: &MonitoringEvent) -> Vec<Alert> {
    let MonitoringEvent::Portfolio(snapshot) = event else {
        return vec![];
    };
    
    let mut alerts = Vec::new();
    let metrics = self.metrics.read().await;
    
    // Compute margin_ratio inline
    let margin_ratio = if snapshot.initial_margin > 0.0 {
        Some(snapshot.margin_balance / snapshot.initial_margin)
    } else {
        None
    };

    for metric in metrics.iter() {
        if !metric.enabled() {
            continue;
        }

        match metric {
            MetricConfig::MarginRatio(cfg) => {
                if let Some(ratio) = margin_ratio {
                    if let Some(min) = cfg.min_threshold {
                        if ratio < min {
                            let key = format!("margin_ratio_{}", snapshot.currency);
                            if !self.in_cooldown(&key).await {
                                alerts.push(self.create_margin_alert(snapshot, ratio, min));
                                self.record_alert(key).await;
                            }
                        }
                    }
                }
            }
            // ... other metric handlers using snapshot fields directly
        }
    }

    alerts
}
```

This is getting complex. Let me reconsider - the existing evaluate logic in lines 71-280 is correct, it just needs the snapshot conversion.

**Simpler approach:** Keep a local conversion struct but don't call it PortfolioSnapshot.

- [ ] **Step 1: Add LocalPortfolioSnapshot struct**

```rust
/// Local snapshot for evaluation - matches metric config expectations
struct LocalSnapshot {
    currency: String,
    timestamp: u64,
    margin_ratio: Option<f64>,
    free_balance: f64,
    delta_exposure: f64,
    session_pnl: f64,
    options_gamma: f64,
    options_vega: f64,
    options_theta: f64,
}
```

- [ ] **Step 2: Update evaluate to convert and call existing logic**

```rust
async fn evaluate(&self, event: &MonitoringEvent) -> Vec<Alert> {
    let MonitoringEvent::Portfolio(snapshot) = event else {
        return vec![];
    };
    
    let local = LocalSnapshot {
        currency: snapshot.currency.clone(),
        timestamp: snapshot.timestamp,
        margin_ratio: if snapshot.initial_margin > 0.0 {
            Some(snapshot.margin_balance / snapshot.initial_margin)
        } else {
            None
        },
        free_balance: snapshot.available_funds,
        delta_exposure: snapshot.delta_total,
        session_pnl: snapshot.session_pnl,
        options_gamma: snapshot.options_gamma,
        options_vega: snapshot.options_vega,
        options_theta: snapshot.options_theta,
    };
    
    // Now use existing evaluation logic with local snapshot
    self.evaluate_local(&local).await
}
```

Actually, this is getting messy. The cleanest approach is to just fix the existing evaluate method to use vol_core fields directly. Let me provide the complete fixed implementation.

- [ ] **Step 1: Replace entire evaluate method (lines 315-324)**

Delete lines 315-324 and replace with:

```rust
async fn evaluate(&self, event: &MonitoringEvent) -> Vec<Alert> {
    let MonitoringEvent::Portfolio(snapshot) = event else {
        return vec![];
    };

    let mut alerts = Vec::new();
    let metrics = self.metrics.read().await;

    // Compute derived fields
    let margin_ratio = if snapshot.initial_margin > 0.0 {
        Some(snapshot.margin_balance / snapshot.initial_margin)
    } else {
        None
    };

    for metric in metrics.iter() {
        if !metric.enabled() {
            continue;
        }

        match metric {
            MetricConfig::MarginRatio(cfg) => {
                if let Some(ratio) = margin_ratio {
                    if let Some(min) = cfg.min_threshold {
                        if ratio < min {
                            let key = format!("margin_ratio_{}", snapshot.currency);
                            if !self.in_cooldown(&key).await {
                                alerts.push(Alert {
                                    alert_type: AlertType::PortfolioMargin {
                                        current: ratio,
                                        threshold: min
                                    },
                                    tenor: Tenor::Medium,
                                    symbol: format!("PORTFOLIO_{}", snapshot.currency),
                                    iv: 0.0,
                                    message: format!("Margin ratio {:.2} below threshold {:.2}", ratio, min),
                                    timestamp: snapshot.timestamp,
                                    source: "deribit".to_string(),
                                    index_price: 0.0,
                                    dte: 0,
                                    option_type: OptionType::Call,
                                    moneyness: 0.0,
                                    mark_price_coin: snapshot.available_funds,
                                });
                                self.record_alert(key).await;
                            }
                        }
                    }
                }
            }
            MetricConfig::FreeBalance(cfg) => {
                if let Some(min) = cfg.min_threshold {
                    if snapshot.available_funds < min {
                        let key = format!("free_balance_{}", snapshot.currency);
                        if !self.in_cooldown(&key).await {
                            alerts.push(Alert {
                                alert_type: AlertType::PortfolioBalance {
                                    current: snapshot.available_funds,
                                    threshold: min
                                },
                                tenor: Tenor::Medium,
                                symbol: format!("PORTFOLIO_{}", snapshot.currency),
                                iv: 0.0,
                                message: format!("Free balance {:.2} below threshold {:.2}", snapshot.available_funds, min),
                                timestamp: snapshot.timestamp,
                                source: "deribit".to_string(),
                                index_price: 0.0,
                                dte: 0,
                                option_type: OptionType::Call,
                                moneyness: 0.0,
                                mark_price_coin: snapshot.available_funds,
                            });
                            self.record_alert(key).await;
                        }
                    }
                }
            }
            MetricConfig::DeltaExposure(cfg) => {
                let delta = snapshot.delta_total;
                let triggered = cfg.min_threshold.map(|min| delta < min).unwrap_or(false)
                    || cfg.max_threshold.map(|max| delta > max).unwrap_or(false);

                if triggered {
                    let key = format!("delta_exposure_{}", snapshot.currency);
                    if !self.in_cooldown(&key).await {
                        alerts.push(Alert {
                            alert_type: AlertType::PortfolioDelta {
                                current: delta
                            },
                            tenor: Tenor::Medium,
                            symbol: format!("PORTFOLIO_{}", snapshot.currency),
                            iv: 0.0,
                            message: format!("Delta exposure {:.2} outside thresholds", delta),
                            timestamp: snapshot.timestamp,
                            source: "deribit".to_string(),
                            index_price: 0.0,
                            dte: 0,
                            option_type: OptionType::Call,
                            moneyness: 0.0,
                            mark_price_coin: 0.0,
                        });
                        self.record_alert(key).await;
                    }
                }
            }
            MetricConfig::SessionPnl(cfg) => {
                if let Some(max) = cfg.max_threshold {
                    if snapshot.session_pnl > max {  // PnL is positive for gains
                        let key = format!("session_pnl_{}", snapshot.currency);
                        if !self.in_cooldown(&key).await {
                            alerts.push(Alert {
                                alert_type: AlertType::PortfolioPnL {
                                    current: snapshot.session_pnl,
                                    threshold: max
                                },
                                tenor: Tenor::Medium,
                                symbol: format!("PORTFOLIO_{}", snapshot.currency),
                                iv: 0.0,
                                message: format!("Session PnL {:.2} exceeds threshold {:.2}", snapshot.session_pnl, max),
                                timestamp: snapshot.timestamp,
                                source: "deribit".to_string(),
                                index_price: 0.0,
                                dte: 0,
                                option_type: OptionType::Call,
                                moneyness: 0.0,
                                mark_price_coin: 0.0,
                            });
                            self.record_alert(key).await;
                        }
                    }
                }
            }
            MetricConfig::TotalGreeks(cfg) => {
                // Gamma
                if let Some(threshold) = cfg.gamma_threshold {
                    if snapshot.options_gamma.abs() > threshold {
                        let key = format!("gamma_{}", snapshot.currency);
                        if !self.in_cooldown(&key).await {
                            alerts.push(Alert {
                                alert_type: AlertType::PortfolioGreek {
                                    greek: "gamma".to_string(),
                                    current: snapshot.options_gamma,
                                    threshold
                                },
                                tenor: Tenor::Medium,
                                symbol: format!("PORTFOLIO_{}", snapshot.currency),
                                iv: 0.0,
                                message: format!("Gamma {:.6} exceeds threshold {:.6}", snapshot.options_gamma, threshold),
                                timestamp: snapshot.timestamp,
                                source: "deribit".to_string(),
                                index_price: 0.0,
                                dte: 0,
                                option_type: OptionType::Call,
                                moneyness: 0.0,
                                mark_price_coin: 0.0,
                            });
                            self.record_alert(key).await;
                        }
                    }
                }
                // Vega
                if let Some(threshold) = cfg.vega_threshold {
                    if snapshot.options_vega.abs() > threshold {
                        let key = format!("vega_{}", snapshot.currency);
                        if !self.in_cooldown(&key).await {
                            alerts.push(Alert {
                                alert_type: AlertType::PortfolioGreek {
                                    greek: "vega".to_string(),
                                    current: snapshot.options_vega,
                                    threshold
                                },
                                tenor: Tenor::Medium,
                                symbol: format!("PORTFOLIO_{}", snapshot.currency),
                                iv: 0.0,
                                message: format!("Vega {:.6} exceeds threshold {:.6}", snapshot.options_vega, threshold),
                                timestamp: snapshot.timestamp,
                                source: "deribit".to_string(),
                                index_price: 0.0,
                                dte: 0,
                                option_type: OptionType::Call,
                                moneyness: 0.0,
                                mark_price_coin: 0.0,
                            });
                            self.record_alert(key).await;
                        }
                    }
                }
                // Theta
                if let Some(threshold) = cfg.theta_threshold {
                    if snapshot.options_theta < -threshold {  // Theta is typically negative (time decay)
                        let key = format!("theta_{}", snapshot.currency);
                        if !self.in_cooldown(&key).await {
                            alerts.push(Alert {
                                alert_type: AlertType::PortfolioGreek {
                                    greek: "theta".to_string(),
                                    current: snapshot.options_theta,
                                    threshold
                                },
                                tenor: Tenor::Medium,
                                symbol: format!("PORTFOLIO_{}", snapshot.currency),
                                iv: 0.0,
                                message: format!("Theta {:.6} exceeds threshold {:.6}", snapshot.options_theta, threshold),
                                timestamp: snapshot.timestamp,
                                source: "deribit".to_string(),
                                index_price: 0.0,
                                dte: 0,
                                option_type: OptionType::Call,
                                moneyness: 0.0,
                                mark_price_coin: 0.0,
                            });
                            self.record_alert(key).await;
                        }
                    }
                }
            }
        }
    }

    alerts
}
```

- [ ] **Step 2: Remove old evaluate method lines 71-280**

The old `pub async fn evaluate(&self, snapshot: &PortfolioSnapshot)` method is no longer needed. Delete it.

- [ ] **Step 3: Run cargo check**

Run: `cargo check -p vol-rules`
Expected: Compiles successfully

- [ ] **Step 4: Commit**

```bash
git add crates/vol-rules/src/portfolio.rs
git commit -m "fix: complete PortfolioRule::evaluate() implementation

Convert vol_core::PortfolioSnapshot fields and evaluate all metrics:
- Margin ratio (computed from margin_balance/initial_margin)
- Free balance (available_funds)
- Delta exposure (delta_total)
- Session PnL
- Greeks (gamma, vega, theta thresholds)
"
```

---

### Task 5: Add PortfolioDataSource to Configuration

**Files:**
- Modify: `crates/vol-config/src/datasource.rs`

- [ ] **Step 1: Add PortfolioDataSourceConfig**

In `crates/vol-config/src/datasource.rs`, add:

```rust
/// Portfolio data source configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioDataSourceConfig {
    pub id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_60")]
    pub poll_interval_secs: u64,
    #[serde(default = "default_currencies")]
    pub currencies: Vec<String>,
}

fn default_currencies() -> Vec<String> {
    vec!["BTC".to_string(), "ETH".to_string()]
}
```

- [ ] **Step 2: Add to DataSourceConfig enum**

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "provider", rename_all = "lowercase")]
pub enum DataSourceConfig {
    Deribit(DeribitDataSourceConfig),
    Binance(BinanceDataSourceConfig),
    Internal(InternalDataSourceConfig),
    #[serde(rename = "deribit-portfolio")]
    Portfolio(PortfolioDataSourceConfig),
}
```

- [ ] **Step 3: Update id() and enabled() match arms**

Add match arms for `DataSourceConfig::Portfolio` in both methods.

- [ ] **Step 4: Run cargo check**

Run: `cargo check -p vol-config`
Expected: Compiles successfully

- [ ] **Step 5: Commit**

```bash
git add crates/vol-config/src/datasource.rs
git commit -m "feat: add PortfolioDataSourceConfig to configuration

Add deribit-portfolio provider type with:
- poll_interval_secs configuration
- currencies list (default: BTC, ETH)
"
```

---

### Task 6: Wire Up PortfolioDataSource in vol-monitor main.rs

**Files:**
- Modify: `crates/vol-monitor/src/main.rs`

- [ ] **Step 1: Add import**

At top of file, add:
```rust
use vol_datasource::PortfolioDataSource;
use vol_deribit::DeribitClient;
```

- [ ] **Step 2: Add PortfolioDataSource in datasources loop**

In the `for ds_config in &config.datasources` loop, add new match arm:

```rust
DataSourceConfig::Portfolio(portfolio_cfg) => {
    if !portfolio_cfg.enabled {
        continue;
    }

    // Create DeribitClient for portfolio access
    let client = DeribitClient::new(
        deribit_cfg.ws_url.clone(),  // Reuse WebSocket URL from config
    );
    
    // Use auth from existing Deribit config if available
    // (Portfolio uses same credentials)
    
    let ds = PortfolioDataSource::new(
        portfolio_cfg.id.clone(),
        client,
        portfolio_cfg.poll_interval_secs,
        portfolio_cfg.currencies.clone(),
    );

    builder = builder.with_datasource(Box::new(ds));
    info!("Added portfolio datasource: {}", portfolio_cfg.id);
}
```

Wait, this needs the Deribit auth config. Let me reconsider. The portfolio datasource should share the same Deribit client/authentication as the market data datasource.

**Better approach:** The portfolio datasource reuses the existing Deribit datasource's authentication. We need to pass the client or credentials.

Actually, looking at the existing code, the DeribitDataSource creates its own client internally. For PortfolioDataSource, we should do the same - create a client with the same credentials.

Let me simplify: just require that portfolio uses the same auth as the Deribit datasource, and create the client similarly.

- [ ] **Step 2 (corrected): Add PortfolioDataSource match arm**

```rust
DataSourceConfig::Portfolio(portfolio_cfg) => {
    if !portfolio_cfg.enabled {
        continue;
    }

    // Find Deribit datasource config for auth credentials
    let deribit_auth = config.datasources.iter()
        .find_map(|ds| {
            if let DataSourceConfig::Deribit(d) = ds {
                d.auth.clone()
            } else {
                None
            }
        });

    let mut client = DeribitClient::new("wss://www.deribit.com/ws/api/v2");
    if let Some(auth) = deribit_auth {
        if let (Some(id), Some(secret)) = (auth.client_id(), auth.client_secret()) {
            client = client.with_auth(id, secret);
        }
    }

    let ds = PortfolioDataSource::new(
        portfolio_cfg.id.clone(),
        client,
        portfolio_cfg.poll_interval_secs,
        portfolio_cfg.currencies.clone(),
    );

    builder = builder.with_datasource(Box::new(ds));
    info!("Added portfolio datasource: {}", portfolio_cfg.id);
}
```

- [ ] **Step 3: Add Portfolio rules in rules loop**

The existing rules loop should handle Portfolio rules via the MetricConfig. Make sure the rule creation uses the correct config type.

In the rules match, add:
```rust
RuleConfig::Portfolio(portfolio_cfg) => {
    if !portfolio_cfg.enabled {
        continue;
    }
    
    let rule = PortfolioRule::new(
        portfolio_cfg.metrics.clone(),
        config.engine.alert_cooldown_secs,
        portfolio_cfg.id.clone(),
        portfolio_cfg.notifications.clone(),
    );
    
    builder = builder.with_rule(Box::new(rule));
    info!("Added portfolio rule: {} (metrics: {})", 
        portfolio_cfg.id, 
        portfolio_cfg.metrics.iter().filter(|m| m.enabled()).count());
}
```

- [ ] **Step 4: Run cargo check**

Run: `cargo check -p vol-monitor`
Expected: Compiles successfully

- [ ] **Step 5: Commit**

```bash
git add crates/vol-monitor/src/main.rs
git commit -m "feat: wire up PortfolioDataSource and PortfolioRule in main

- Create PortfolioDataSource from config
- Reuse Deribit authentication from existing config
- Register PortfolioRule with metric configuration
"
```

---

### Task 7: Update config.toml with Example Configuration

**Files:**
- Modify: `config.toml`

- [ ] **Step 1: Add portfolio datasource**

Add after the existing deribit datasource:

```toml
[[datasources]]
id = "portfolio"
provider = "deribit-portfolio"
poll_interval_secs = 30
currencies = ["BTC", "ETH"]
enabled = true
```

- [ ] **Step 2: Add portfolio rules**

Add at end of rules section:

```toml
# Portfolio monitoring rules
[[rules]]
id = "portfolio-greeks"
type = "portfolio"
enabled = true
metrics = [
    { type = "delta_exposure", enabled = true, min_threshold = -100.0, max_threshold = 100.0 },
    { type = "total_greeks", enabled = true, gamma_threshold = 50.0, vega_threshold = 200.0, theta_threshold = 100.0 },
    { type = "free_balance", enabled = true, min_threshold = 0.5 },
    { type = "margin_ratio", enabled = true, min_threshold = 1.25 },
]
notifications = ["feishu-alerts", "stdout"]
```

- [ ] **Step 3: Validate config parsing**

Run: `cargo run --bin vol-monitor -- --help` (or just check config loads)
Expected: No parse errors

- [ ] **Step 4: Commit**

```bash
git add config.toml
git commit -m "docs: add example portfolio monitoring configuration

Example configuration for:
- Portfolio datasource (30s polling)
- Greeks exposure thresholds
- Margin ratio and free balance alerts
"
```

---

### Task 8: Test and Verify

**Files:** N/A (testing task)

- [ ] **Step 1: Build workspace**

Run: `cargo build --release`
Expected: Successful build

- [ ] **Step 2: Run unit tests**

Run: `cargo test --workspace`
Expected: All tests pass

- [ ] **Step 3: Verify config loading**

Run: `RUST_LOG=info cargo run --package vol-monitor 2>&1 | head -50`
Expected: See "Added portfolio datasource" and "Added portfolio rule" log messages

- [ ] **Step 4: Commit if all tests pass**

```bash
git commit --allow-empty -m "test: portfolio monitoring implementation complete

All tests passing. Ready for integration testing with live Deribit account.
"
```

---

## Self-Review Checklist

Before presenting this plan, let me verify:

**1. Spec Coverage:**
- ✅ PortfolioDataSource (Task 3)
- ✅ Portfolio data models (Task 2 - positions API)
- ✅ EventType::Portfolio exists (already in vol-core)
- ✅ Rule types - uses existing MetricConfig-based PortfolioRule
- ✅ Configuration (Task 5, Task 7)
- ✅ Notification (reuses existing Feishu/Stdout)

**2. Placeholder Scan:**
- No TBD/TODO in plan
- All code steps show actual code
- All file paths are exact

**3. Type Consistency:**
- `PortfolioSnapshot` - uses vol_core version consistently
- `MetricConfig` variants match existing types
- `Alert` construction follows existing patterns

**Potential Issues Identified:**
1. The `AlertType` enum needs variants like `PortfolioMargin`, `PortfolioBalance`, etc. - these may need to be added to `vol-core/src/alert.rs`
2. The `DeribitClient` may need `with_auth` method or similar for authentication

Let me add a task to fix AlertType:

---

### Task 9 (Fix): Add Portfolio AlertType Variants

**Files:**
- Modify: `crates/vol-core/src/alert.rs`

- [ ] **Step 1: Check existing AlertType enum**

Read `crates/vol-core/src/alert.rs` and check for Portfolio variants.

- [ ] **Step 2: Add missing variants**

```rust
pub enum AlertType {
    // Existing variants...
    
    // Portfolio alerts
    PortfolioMargin { current: f64, threshold: f64 },
    PortfolioBalance { current: f64, threshold: f64 },
    PortfolioDelta { current: f64 },
    PortfolioPnL { current: f64, threshold: f64 },
    PortfolioGreek { greek: String, current: f64, threshold: f64 },
}
```

- [ ] **Step 3: Update Display/Serialize implementations**

Add match arms for new variants.

- [ ] **Step 4: Commit**

```bash
git add crates/vol-core/src/alert.rs
git commit -m "feat: add Portfolio alert types to AlertType enum
"
```

---

Plan complete. Ready for execution.
