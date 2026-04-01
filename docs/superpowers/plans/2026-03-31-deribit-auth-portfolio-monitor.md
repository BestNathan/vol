# Deribit Authentication + Portfolio Monitor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Deribit OAuth authentication support and portfolio monitoring with configurable alerts for margin ratio, balance, Greeks, and PnL.

**Architecture:**
1. Add `auth` config section to `DeribitConfig` with `client_id`/`client_secret`, support env var override
2. Extend `DeribitClient` to authenticate via `private/auth` method before subscribing to private channels
3. Add `user.portfolio.any` subscription channel type and parse portfolio notifications
4. Create `PortfolioAlertHandler` with configurable metrics (margin_ratio, free_balance, delta_exposure, session_pnl, total_greeks)
5. Add JSONL output handler for portfolio data logging

**Tech Stack:** Rust, tokio, serde, TOML config, Deribit API v2

---

## File Structure

### Files to Create
- `crates/vol-deribit/src/portfolio.rs` - Portfolio data models and parsing
- `crates/vol-alert/src/portfolio.rs` - Portfolio alert handler with configurable metrics
- `crates/vol-notification/src/portfolio_output.rs` - JSONL file output handler
- `crates/vol-config/src/metrics.rs` - Metric configuration types (enum-based)

### Files to Modify
- `crates/vol-config/src/lib.rs:24-29` - Add `DeribitAuthConfig` to `DeribitConfig`
- `crates/vol-config/src/lib.rs:54-62` - Add `metrics: Vec<MetricConfig>` to `AlertsConfig`
- `crates/vol-deribit/src/client.rs:46-55` - Add auth fields to `DeribitClient`
- `crates/vol-deribit/src/client.rs:58-68` - Update `new()` to accept auth params
- `crates/vol-deribit/src/client.rs:419-548` - Add authentication before subscription
- `crates/vol-deribit/src/subscription.rs` - Add `user_portfolio_any()` channel builder
- `crates/vol-deribit/src/message.rs` - Add `PortfolioNotification` type
- `crates/vol-deribit/src/lib.rs` - Export new modules
- `crates/vol-monitor/src/main.rs:100-150` - Wire up portfolio subscription and alert handler

---

### Task 1: Add Deribit Auth Configuration

**Files:**
- Create: `crates/vol-config/src/metrics.rs`
- Modify: `crates/vol-config/src/lib.rs:24-29`

- [ ] **Step 1: Write test for auth config parsing**

```rust
#[test]
fn test_deribit_auth_config_from_toml() {
    let toml_str = r#"
        [data_sources.deribit.auth]
        client_id = "test_client"
        client_secret = "test_secret"
    "#;

    let config: toml::Value = toml::from_str(toml_str).unwrap();
    let auth = config.get("data_sources").unwrap().get("deribit").unwrap().get("auth").unwrap();
    assert_eq!(auth.get("client_id").unwrap().as_str().unwrap(), "test_client");
    assert_eq!(auth.get("client_secret").unwrap().as_str().unwrap(), "test_secret");
}

#[test]
fn test_metric_config_parsing() {
    let toml_str = r#"
        [[alerts.metrics]]
        type = "free_balance"
        enabled = true
        min_threshold = 10000.0

        [[alerts.metrics]]
        type = "total_greeks"
        enabled = true
        gamma_threshold = 0.05
        vega_threshold = 100.0
    "#;

    let config: toml::Value = toml::from_str(toml_str).unwrap();
    let metrics = config.get("alerts").unwrap().get("metrics").unwrap().as_array().unwrap();
    assert_eq!(metrics.len(), 2);
    assert_eq!(metrics[0].get("type").unwrap().as_str().unwrap(), "free_balance");
    assert_eq!(metrics[1].get("type").unwrap().as_str().unwrap(), "total_greeks");
}
```

- [ ] **Step 2: Run test to verify it passes**

```bash
cargo test -p vol-config test_deribit_auth_config_from_toml -- --nocapture
cargo test -p vol-config test_metric_config_parsing -- --nocapture
```
Expected: PASS (basic TOML parsing)

- [ ] **Step 3: Add `DeribitAuthConfig` struct**

```rust
// In crates/vol-config/src/lib.rs, add before DeribitConfig

/// Deribit OAuth authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeribitAuthConfig {
    /// OAuth client ID - env var DERIBIT_CLIENT_ID takes precedence
    #[serde(default)]
    pub client_id: Option<String>,
    /// OAuth client secret - env var DERIBIT_CLIENT_SECRET takes precedence
    #[serde(default)]
    pub client_secret: Option<String>,
}

impl DeribitAuthConfig {
    /// Get client_id with env var override
    pub fn client_id(&self) -> Option<String> {
        std::env::var("DERIBIT_CLIENT_ID")
            .ok()
            .or_else(|| self.client_id.clone())
    }

    /// Get client_secret with env var override
    pub fn client_secret(&self) -> Option<String> {
        std::env::var("DERIBIT_CLIENT_SECRET")
            .ok()
            .or_else(|| self.client_secret.clone())
    }
}
```

- [ ] **Step 4: Add `auth` field to `DeribitConfig`**

```rust
// Modify DeribitConfig in crates/vol-config/src/lib.rs:24-29

/// Deribit-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeribitConfig {
    pub ws_url: String,
    pub symbols: Vec<String>,
    pub poll_interval_secs: u64,
    /// Optional OAuth authentication for private channels
    #[serde(default)]
    pub auth: Option<DeribitAuthConfig>,
}
```

- [ ] **Step 5: Add metric configuration types**

Create `crates/vol-config/src/metrics.rs`:

```rust
use serde::{Deserialize, Serialize};

/// Metric configuration - enum-based for type safety
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum MetricConfig {
    #[serde(rename = "free_balance")]
    FreeBalance(ThresholdConfig),

    #[serde(rename = "margin_ratio")]
    MarginRatio(MarginRatioConfig),

    #[serde(rename = "delta_exposure")]
    DeltaExposure(ThresholdConfig),

    #[serde(rename = "session_pnl")]
    SessionPnl(ThresholdConfig),

    #[serde(rename = "total_greeks")]
    TotalGreeks(GreeksConfig),
}

impl MetricConfig {
    pub fn enabled(&self) -> bool {
        match self {
            MetricConfig::FreeBalance(c) => c.enabled,
            MetricConfig::MarginRatio(c) => c.enabled,
            MetricConfig::DeltaExposure(c) => c.enabled,
            MetricConfig::SessionPnl(c) => c.enabled,
            MetricConfig::TotalGreeks(c) => c.enabled,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            MetricConfig::FreeBalance(_) => "free_balance",
            MetricConfig::MarginRatio(_) => "margin_ratio",
            MetricConfig::DeltaExposure(_) => "delta_exposure",
            MetricConfig::SessionPnl(_) => "session_pnl",
            MetricConfig::TotalGreeks(_) => "total_greeks",
        }
    }
}

/// Simple threshold configuration for most metrics
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ThresholdConfig {
    pub enabled: bool,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub min_threshold: Option<f64>,
    #[serde(default)]
    pub max_threshold: Option<f64>,
}

/// Margin ratio specific configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MarginRatioConfig {
    pub enabled: bool,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub min_threshold: Option<f64>,
}

/// Greeks configuration with multiple thresholds
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GreeksConfig {
    pub enabled: bool,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub gamma_threshold: Option<f64>,
    #[serde(default)]
    pub vega_threshold: Option<f64>,
    #[serde(default)]
    pub theta_threshold: Option<f64>,
    #[serde(default)]
    pub delta_threshold: Option<f64>,
}
```

- [ ] **Step 6: Add `metrics` field to `AlertsConfig`**

```rust
// In crates/vol-config/src/lib.rs, add use and field
use crate::metrics::MetricConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertsConfig {
    pub enabled: bool,
    pub cooldown_secs: u64,
    pub absolute_iv: AbsoluteIvConfig,
    pub rate_of_change: RateOfChangeConfig,
    pub term_structure: TermStructureConfig,
    pub skew: SkewConfig,
    /// Portfolio monitoring metrics
    #[serde(default)]
    pub metrics: Vec<MetricConfig>,
}
```

- [ ] **Step 7: Export metrics module**

```rust
// In crates/vol-config/src/lib.rs, add at module level
pub mod metrics;
```

- [ ] **Step 8: Run tests**

```bash
cargo test -p vol-config
```
Expected: All tests pass

- [ ] **Step 9: Commit**

```bash
git add crates/vol-config/src/lib.rs crates/vol-config/src/metrics.rs
git commit -m "feat: add Deribit auth config and metric configuration types"
```

---

### Task 2: Add Portfolio Data Models

**Files:**
- Create: `crates/vol-deribit/src/portfolio.rs`
- Modify: `crates/vol-deribit/src/lib.rs`

- [ ] **Step 1: Write test for portfolio notification parsing**

```rust
#[test]
fn test_portfolio_notification_parsing() {
    let json = r#"{
        "jsonrpc": "2.0",
        "method": "subscription",
        "params": {
            "channel": "user.portfolio.BTC",
            "data": {
                "currency": "BTC",
                "equity": 2.6437733,
                "balance": 3.4906363,
                "available_funds": 2.2638913,
                "margin_balance": 2.25,
                "initial_margin": 1.24639592,
                "maintenance_margin": 0.8854841,
                "session_upl": 0.05341555,
                "session_rpl": -0.03311399,
                "delta_total": 31.602298,
                "options_delta": -1.01958,
                "options_gamma": 0.00001,
                "options_theta": 16.13825,
                "options_vega": 0.07976
            }
        }
    }"#;

    let notification: DeribitNotification<PortfolioData> = serde_json::from_str(json).unwrap();
    assert_eq!(notification.method, "subscription");
    // Additional assertions on data fields
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p vol-deribit test_portfolio_notification_parsing
```
Expected: FAIL (types not defined)

- [ ] **Step 3: Create portfolio data model**

Create `crates/vol-deribit/src/portfolio.rs`:

```rust
//! Deribit portfolio data models for user.portfolio subscription.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Portfolio notification data from user.portfolio.(currency) channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioData {
    /// Currency code (BTC, ETH, USDC, etc.)
    pub currency: String,
    /// Account equity
    pub equity: f64,
    /// Account balance
    pub balance: f64,
    /// Available funds for trading
    pub available_funds: f64,
    /// Available withdrawal funds
    #[serde(default)]
    pub available_withdrawal_funds: Option<f64>,
    /// Margin balance
    pub margin_balance: f64,
    /// Initial margin requirement
    pub initial_margin: f64,
    /// Maintenance margin requirement
    pub maintenance_margin: f64,
    /// Session unrealized P&L
    #[serde(default)]
    pub session_upl: f64,
    /// Session realized P&L
    #[serde(default)]
    pub session_rpl: f64,
    /// Total P&L
    #[serde(default)]
    pub total_pl: f64,
    /// Total delta (options + futures)
    #[serde(default)]
    pub delta_total: f64,
    /// Options delta
    #[serde(default)]
    pub options_delta: f64,
    /// Options gamma
    #[serde(default)]
    pub options_gamma: f64,
    /// Options theta
    #[serde(default)]
    pub options_theta: f64,
    /// Options vega
    #[serde(default)]
    pub options_vega: f64,
    /// Options value
    #[serde(default)]
    pub options_value: f64,
    /// Options P&L
    #[serde(default)]
    pub options_pl: f64,
    /// Futures P&L
    #[serde(default)]
    pub futures_pl: f64,
    /// Delta total map per index
    #[serde(default)]
    pub delta_total_map: HashMap<String, f64>,
    /// Gamma map per index
    #[serde(default)]
    pub options_gamma_map: HashMap<String, f64>,
    /// Theta map per index
    #[serde(default)]
    pub options_theta_map: HashMap<String, f64>,
    /// Vega map per index
    #[serde(default)]
    pub options_vega_map: HashMap<String, f64>,
    /// Projected delta total
    #[serde(default)]
    pub projected_delta_total: Option<f64>,
    /// Portfolio margining enabled
    #[serde(default)]
    pub portfolio_margining_enabled: bool,
    /// Cross collateral enabled
    #[serde(default)]
    pub cross_collateral_enabled: bool,
    /// Margin model
    #[serde(default)]
    pub margin_model: String,
    /// Fee balance
    #[serde(default)]
    pub fee_balance: Option<f64>,
    /// Projected initial margin
    #[serde(default)]
    pub projected_initial_margin: Option<f64>,
    /// Projected maintenance margin
    #[serde(default)]
    pub projected_maintenance_margin: Option<f64>,
}

impl PortfolioData {
    /// Calculate margin ratio (margin_balance / initial_margin)
    pub fn margin_ratio(&self) -> Option<f64> {
        if self.initial_margin > 0.0 {
            Some(self.margin_balance / self.initial_margin)
        } else {
            None
        }
    }

    /// Get free balance (available_funds)
    pub fn free_balance(&self) -> f64 {
        self.available_funds
    }

    /// Get delta exposure (delta_total)
    pub fn delta_exposure(&self) -> f64 {
        self.delta_total
    }

    /// Get session P&L in USD (approximate for BTC/ETH)
    pub fn session_pnl_usd(&self, index_price: f64) -> f64 {
        (self.session_upl + self.session_rpl) * index_price
    }
}
```

- [ ] **Step 4: Add notification type for portfolio**

```rust
// In crates/vol-deribit/src/message.rs, add to DeribitNotification enum

use crate::portfolio::PortfolioData;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DeribitNotification<T = Vec<super::market_data::OptionMarkPrice>> {
    Markprice(super::message::Notification<T>),
    PriceIndex(super::message::Notification<super::market_data::PriceIndex>),
    Ticker(super::message::Notification<Vec<super::market_data::DeribitTicker>>),
    Trade(super::message::Notification<Vec<super::market_data::Trade>>),
    Portfolio(super::message::Notification<PortfolioData>),
}
```

- [ ] **Step 5: Export portfolio module**

```rust
// In crates/vol-deribit/src/lib.rs, add
pub mod portfolio;
pub use portfolio::PortfolioData;
```

- [ ] **Step 6: Run tests**

```bash
cargo test -p vol-deribit
```
Expected: All tests pass

- [ ] **Step 7: Commit**

```bash
git add crates/vol-deribit/src/portfolio.rs crates/vol-deribit/src/message.rs crates/vol-deribit/src/lib.rs
git commit -m "feat: add portfolio data models for user.portfolio subscription"
```

---

### Task 3: Implement Deribit OAuth Authentication

**Files:**
- Modify: `crates/vol-deribit/src/client.rs:46-68`, `crates/vol-deribit/src/client.rs:419-548`

- [ ] **Step 1: Write test for auth flow (unit test with mock)**

```rust
#[test]
fn test_client_with_auth() {
    let client = DeribitClient::new("wss://test.deribit.com/ws/api/v2")
        .with_auth("test_client", "test_secret");

    assert!(client.has_auth());
}
```

- [ ] **Step 2: Add auth fields to `DeribitClient`**

```rust
// Modify DeribitClient struct in client.rs:46-55

pub struct DeribitClient {
    ws_url: String,
    state: Arc<Mutex<ClientState>>,
    proxy_url: Option<String>,
    subscription_manager: Arc<SubscriptionManager>,
    subscribed_channels: Arc<Mutex<Vec<ChannelType>>>,
    ws_sender: Arc<Mutex<Option<Arc<Mutex<WsSplitSink>>>>>,
    /// OAuth client ID
    client_id: Option<String>,
    /// OAuth client secret
    client_secret: Option<String>,
}
```

- [ ] **Step 3: Add `with_auth()` method**

```rust
// Modify client.rs:58-68, add after with_proxy

/// Configure OAuth authentication
pub fn with_auth(mut self, client_id: impl Into<String>, client_secret: impl Into<String>) -> Self {
    self.client_id = Some(client_id.into());
    self.client_secret = Some(client_secret.into());
    self
}

/// Check if authentication is configured
pub fn has_auth(&self) -> bool {
    self.client_id.is_some() && self.client_secret.is_some()
}
```

- [ ] **Step 4: Add `get_access_token()` method**

```rust
// Add new method before start_connection

/// Get OAuth access token via HTTP POST
async fn get_access_token(&self) -> Result<String, vol_core::VolError> {
    let client_id = self.client_id.as_ref()
        .ok_or_else(|| vol_core::VolError::Auth("client_id not configured".into()))?;
    let client_secret = self.client_secret.as_ref()
        .ok_or_else(|| vol_core::VolError::VolError::Auth("client_secret not configured".into()))?;

    let client = reqwest::Client::new();
    let response = client
        .post("https://www.deribit.com/api/v2/public/auth")
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "public/auth",
            "params": {
                "grant_type": "client_credentials",
                "client_id": client_id,
                "client_secret": client_secret
            }
        }))
        .send()
        .await
        .map_err(|e| vol_core::VolError::Auth(format!("Token request failed: {}", e)))?;

    let result: serde_json::Value = response.json().await
        .map_err(|e| vol_core::VolError::Auth(format!("Token parse failed: {}", e)))?;

    result.get("result")
        .and_then(|r| r.get("access_token"))
        .and_then(|t| t.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| vol_core::VolError::Auth("No access token in response".into()))
}
```

- [ ] **Step 5: Modify `start_connection` to authenticate first**

```rust
// In start_connection, after connect and before subscription (around line 463)

// After TLS connection established, authenticate if credentials present
let access_token = if self.client_id.is_some() && self.client_secret.is_some() {
    match self.get_access_token().await {
        Ok(token) => {
            info!("OAuth authentication successful");
            Some(token)
        }
        Err(e) => {
            error!("OAuth authentication failed: {}", e);
            None
        }
    }
} else {
    None
};

// Send initial subscription
{
    let mut writer = write_arc.lock().await;

    // If we have auth, send auth message first
    if let Some(token) = &access_token {
        let auth_msg = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "private/auth",
            "params": {
                "access_token": token
            }
        });

        writer.send(Message::Text(auth_msg.to_string())).await?;

        // Wait for auth response
        if let Some(Ok(msg)) = read.next().await {
            if let Message::Text(text) = msg {
                if let Ok(resp) = serde_json::from_str::<serde_json::Value>(&text) {
                    if resp.get("error").is_some() {
                        error!("Auth failed: {}", text);
                    }
                }
            }
        }
    }

    // Then send subscription
    let subscribe_msg = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "public/subscribe",
        "params": {
            "channels": channel_names.iter().map(|s| s.as_str()).collect::<Vec<&str>>()
        }
    });

    writer.send(Message::Text(subscribe_msg.to_string())).await?;
}
```

- [ ] **Step 6: Add `Auth` variant to `VolError`**

```rust
// In crates/vol-core/src/error.rs, add variant

#[derive(Debug, thiserror::Error)]
pub enum VolError {
    // ... existing variants ...
    #[error("Authentication error: {0}")]
    Auth(String),
}
```

- [ ] **Step 7: Run tests**

```bash
cargo test -p vol-deribit
cargo build -p vol-deribit
```
Expected: Tests pass, build succeeds

- [ ] **Step 8: Commit**

```bash
git add crates/vol-deribit/src/client.rs crates/vol-core/src/error.rs
git commit -m "feat: add OAuth authentication for private channels"
```

---

### Task 4: Add Portfolio Subscription Channel

**Files:**
- Modify: `crates/vol-deribit/src/subscription.rs`, `crates/vol-deribit/src/client.rs:551-611`

- [ ] **Step 1: Add `UserPortfolio` channel type**

```rust
// In crates/vol-deribit/src/lib.rs or subscription.rs, add to ChannelType enum

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ChannelType {
    // ... existing variants ...
    UserPortfolio(String),  // currency or "any"
}
```

- [ ] **Step 2: Add channel name implementation**

```rust
// In ChannelType impl, add match arm

impl ChannelType {
    pub fn channel_name(&self) -> String {
        match self {
            // ... existing ...
            ChannelType::UserPortfolio(currency) => format!("user.portfolio.{}", currency),
        }
    }
}
```

- [ ] **Step 3: Add subscription builder**

```rust
// In crates/vol-deribit/src/subscription.rs, add

impl ChannelType {
    /// Subscribe to portfolio updates for all currencies
    pub fn user_portfolio_any() -> Self {
        ChannelType::UserPortfolio("any".to_string())
    }

    /// Subscribe to portfolio updates for specific currency
    pub fn user_portfolio(currency: impl Into<String>) -> Self {
        ChannelType::UserPortfolio(currency.into())
    }
}
```

- [ ] **Step 4: Add portfolio routing in `parse_and_route`**

```rust
// In client.rs parse_and_route, add match arm after Trade

DeribitNotification::Portfolio(n) => {
    if n.method != "subscription" {
        return None;
    }
    let currency = n
        .params
        .channel
        .strip_prefix("user.portfolio.")?
        .to_string();
    Some((
        ChannelType::UserPortfolio(currency),
        ChannelData::Portfolio(n.params.data),
    ))
}
```

- [ ] **Step 5: Add `Portfolio` variant to `ChannelData`**

```rust
// In crates/vol-deribit/src/lib.rs, add to ChannelData enum

use crate::portfolio::PortfolioData;

#[derive(Debug, Clone)]
pub enum ChannelData {
    // ... existing ...
    Portfolio(PortfolioData),
}
```

- [ ] **Step 6: Run tests**

```bash
cargo test -p vol-deribit
cargo build -p vol-deribit
```
Expected: All tests pass

- [ ] **Step 7: Commit**

```bash
git add crates/vol-deribit/src/subscription.rs crates/vol-deribit/src/client.rs crates/vol-deribit/src/lib.rs
git commit -m "feat: add user.portfolio subscription channel"
```

---

### Task 5: Create Portfolio Alert Handler

**Files:**
- Create: `crates/vol-alert/src/portfolio.rs`

- [ ] **Step 1: Define PortfolioAlert struct**

```rust
// In crates/vol-alert/src/portfolio.rs

use vol_core::{Alert, AlertHandler, Tenor};
use vol_config::metrics::MetricConfig;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Portfolio data snapshot for alert evaluation
#[derive(Debug, Clone)]
pub struct PortfolioSnapshot {
    pub currency: String,
    pub timestamp: u64,
    pub margin_ratio: Option<f64>,
    pub free_balance: f64,
    pub delta_exposure: f64,
    pub session_pnl: f64,
    pub options_gamma: f64,
    pub options_vega: f64,
    pub options_theta: f64,
}

/// Portfolio alert handler with configurable metrics
pub struct PortfolioAlertHandler {
    metrics: Arc<RwLock<Vec<MetricConfig>>>,
    cooldown_secs: u64,
    last_alert: Arc<RwLock<std::collections::HashMap<String, u64>>>,
}

impl PortfolioAlertHandler {
    pub fn new(metrics: Vec<MetricConfig>, cooldown_secs: u64) -> Self {
        Self {
            metrics: Arc::new(RwLock::new(metrics)),
            cooldown_secs,
            last_alert: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// Update metrics configuration
    pub async fn update_metrics(&self, metrics: Vec<MetricConfig>) {
        let mut self_metrics = self.metrics.write().await;
        *self_metrics = metrics;
    }

    /// Check if alert is in cooldown
    async fn in_cooldown(&self, key: &str) -> bool {
        let last = self.last_alert.read().await;
        if let Some(&timestamp) = last.get(key) {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            return now - timestamp < self.cooldown_secs;
        }
        false
    }

    /// Record alert timestamp
    async fn record_alert(&self, key: String) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let mut last = self.last_alert.write().await;
        last.insert(key, now);
    }

    /// Evaluate snapshot against configured metrics
    pub async fn evaluate(&self, snapshot: &PortfolioSnapshot) -> Vec<Alert> {
        let mut alerts = Vec::new();
        let metrics = self.metrics.read().await;

        for metric in metrics.iter() {
            if !metric.enabled() {
                continue;
            }

            match metric {
                MetricConfig::MarginRatio(cfg) => {
                    if let Some(ratio) = snapshot.margin_ratio {
                        if let Some(min) = cfg.min_threshold {
                            if ratio < min {
                                let key = format!("margin_ratio_{}", snapshot.currency);
                                if !self.in_cooldown(&key).await {
                                    alerts.push(Alert {
                                        alert_type: vol_core::AlertType::PortfolioMargin {
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
                                        option_type: vol_core::OptionType::Call,
                                        moneyness: 0.0,
                                        mark_price_coin: snapshot.free_balance,
                                    });
                                    self.record_alert(key).await;
                                }
                            }
                        }
                    }
                }
                MetricConfig::FreeBalance(cfg) => {
                    if let Some(min) = cfg.min_threshold {
                        if snapshot.free_balance < min {
                            let key = format!("free_balance_{}", snapshot.currency);
                            if !self.in_cooldown(&key).await {
                                alerts.push(Alert {
                                    alert_type: vol_core::AlertType::PortfolioBalance {
                                        current: snapshot.free_balance,
                                        threshold: min
                                    },
                                    tenor: Tenor::Medium,
                                    symbol: format!("PORTFOLIO_{}", snapshot.currency),
                                    iv: 0.0,
                                    message: format!("Free balance {:.2} below threshold {:.2}", snapshot.free_balance, min),
                                    timestamp: snapshot.timestamp,
                                    source: "deribit".to_string(),
                                    index_price: 0.0,
                                    dte: 0,
                                    option_type: vol_core::OptionType::Call,
                                    moneyness: 0.0,
                                    mark_price_coin: snapshot.free_balance,
                                });
                                self.record_alert(key).await;
                            }
                        }
                    }
                }
                MetricConfig::DeltaExposure(cfg) => {
                    let delta = snapshot.delta_exposure;
                    let triggered = cfg.min_threshold.map(|min| delta < min).unwrap_or(false)
                        || cfg.max_threshold.map(|max| delta > max).unwrap_or(false);

                    if triggered {
                        let key = format!("delta_exposure_{}", snapshot.currency);
                        if !self.in_cooldown(&key).await {
                            alerts.push(Alert {
                                alert_type: vol_core::AlertType::PortfolioDelta {
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
                                option_type: vol_core::OptionType::Call,
                                moneyness: 0.0,
                                mark_price_coin: 0.0,
                            });
                            self.record_alert(key).await;
                        }
                    }
                }
                MetricConfig::SessionPnl(cfg) => {
                    if let Some(max) = cfg.max_threshold {
                        if snapshot.session_pnl < max {
                            let key = format!("session_pnl_{}", snapshot.currency);
                            if !self.in_cooldown(&key).await {
                                alerts.push(Alert {
                                    alert_type: vol_core::AlertType::PortfolioPnL {
                                        current: snapshot.session_pnl,
                                        threshold: max
                                    },
                                    tenor: Tenor::Medium,
                                    symbol: format!("PORTFOLIO_{}", snapshot.currency),
                                    iv: 0.0,
                                    message: format!("Session PnL {:.2} below threshold {:.2}", snapshot.session_pnl, max),
                                    timestamp: snapshot.timestamp,
                                    source: "deribit".to_string(),
                                    index_price: 0.0,
                                    dte: 0,
                                    option_type: vol_core::OptionType::Call,
                                    moneyness: 0.0,
                                    mark_price_coin: 0.0,
                                });
                                self.record_alert(key).await;
                            }
                        }
                    }
                }
                MetricConfig::TotalGreeks(cfg) => {
                    // Check gamma
                    if let Some(threshold) = cfg.gamma_threshold {
                        if snapshot.options_gamma.abs() > threshold {
                            let key = format!("gamma_{}", snapshot.currency);
                            if !self.in_cooldown(&key).await {
                                alerts.push(Alert {
                                    alert_type: vol_core::AlertType::PortfolioGreek {
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
                                    option_type: vol_core::OptionType::Call,
                                    moneyness: 0.0,
                                    mark_price_coin: 0.0,
                                });
                                self.record_alert(key).await;
                            }
                        }
                    }
                    // Similar checks for vega, theta...
                }
            }
        }

        alerts
    }
}
```

- [ ] **Step 2: Add new AlertType variants**

```rust
// In crates/vol-core/src/event.rs, add to AlertType enum

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AlertType {
    // ... existing ...

    /// Portfolio margin ratio alert
    PortfolioMargin { current: f64, threshold: f64 },
    /// Portfolio free balance alert
    PortfolioBalance { current: f64, threshold: f64 },
    /// Portfolio delta exposure alert
    PortfolioDelta { current: f64 },
    /// Portfolio PnL alert
    PortfolioPnL { current: f64, threshold: f64 },
    /// Portfolio Greeks alert
    PortfolioGreek { greek: String, current: f64, threshold: f64 },
}
```

- [ ] **Step 3: Run build**

```bash
cargo build -p vol-alert
```
Expected: Build succeeds

- [ ] **Step 4: Commit**

```bash
git add crates/vol-alert/src/portfolio.rs crates/vol-core/src/event.rs
git commit -m "feat: add portfolio alert handler with configurable metrics"
```

---

### Task 6: Create Portfolio JSONL Output Handler

**Files:**
- Create: `crates/vol-notification/src/portfolio_output.rs`

- [ ] **Step 1: Create JSONL output handler**

```rust
// In crates/vol-notification/src/portfolio_output.rs

use vol_core::Result;
use vol_deribit::portfolio::PortfolioData;
use tokio::sync::mpsc;
use tokio::io::{AsyncWriteExt, BufWriter};
use std::path::PathBuf;
use tracing::{info, error};

/// Portfolio data JSONL output handler
pub struct PortfolioOutput {
    output_dir: PathBuf,
    file_format: String,
    rotate_interval: String,
}

impl PortfolioOutput {
    pub fn new(output_dir: PathBuf, file_format: String, rotate_interval: String) -> Self {
        Self {
            output_dir,
            file_format,
            rotate_interval,
        }
    }

    /// Run the output loop, writing portfolio data to JSONL files
    pub async fn run(mut self, mut rx: mpsc::Receiver<PortfolioData>) -> Result<()> {
        // Create output directory if needed
        tokio::fs::create_dir_all(&self.output_dir).await?;

        let mut current_file = self.current_file_path();
        let mut writer = BufWriter::new(
            tokio::fs::File::create(&current_file).await?
        );

        while let Some(data) = rx.recv().await {
            // Check if rotation needed
            let new_file = self.current_file_path();
            if new_file != current_file {
                writer.flush().await?;
                current_file = new_file;
                writer = BufWriter::new(
                    tokio::fs::File::create(&current_file).await?
                );
            }

            // Write as JSONL
            let json = serde_json::to_string(&data)?;
            writer.write_all(json.as_bytes()).await?;
            writer.write_all(b"\n").await?;

            if self.file_format == "jsonl" {
                info!("Portfolio data written: {}", data.currency);
            }
        }

        writer.flush().await?;
        Ok(())
    }

    fn current_file_path(&self) -> PathBuf {
        let now = chrono::Utc::now();
        let filename = match self.rotate_interval.as_str() {
            "hourly" => format!("portfolio_{}.jsonl", now.format("%Y%m%d_%H")),
            "daily" => format!("portfolio_{}.jsonl", now.format("%Y%m%d")),
            "weekly" => format!("portfolio_{}.jsonl", now.format("%Y%m%d_week%V")),
            _ => format!("portfolio_{}.jsonl", now.format("%Y%m%d")),
        };
        self.output_dir.join(filename)
    }
}
```

- [ ] **Step 2: Add to notification registry**

```rust
// May need to add dependencies to Cargo.toml
// chrono = "0.4"
```

- [ ] **Step 3: Commit**

```bash
git add crates/vol-notification/src/portfolio_output.rs
git commit -m "feat: add JSONL portfolio data output handler"
```

---

### Task 7: Wire Up in Main Application

**Files:**
- Modify: `crates/vol-monitor/src/main.rs`, `crates/vol-monitor/Cargo.toml`

- [ ] **Step 1: Update main.rs to use auth config**

```rust
// In crates/vol-monitor/src/main.rs

// When creating DeribitClient:
let mut client = DeribitClient::new(&config.data_sources.deribit.ws_url);

// Add auth if configured
if let Some(auth) = &config.data_sources.deribit.auth {
    if let (Some(client_id), Some(client_secret)) = (auth.client_id(), auth.client_secret()) {
        client = client.with_auth(client_id, client_secret);
        info!("OAuth authentication configured");
    }
}
```

- [ ] **Step 2: Subscribe to portfolio channel**

```rust
// Add portfolio subscription
let portfolio_rx = client.subscribe(ChannelType::user_portfolio_any()).await?;

// Clone for output handler
let portfolio_output_rx = portfolio_rx.resubscribe();
```

- [ ] **Step 3: Create portfolio alert handler**

```rust
use vol_alert::PortfolioAlertHandler;
use vol_config::metrics::MetricConfig;

// Create handler with configured metrics
let portfolio_alert_handler = PortfolioAlertHandler::new(
    config.alerts.metrics.clone(),
    config.alerts.cooldown_secs,
);
```

- [ ] **Step 4: Add portfolio processing task**

```rust
// Spawn task to process portfolio data
tokio::spawn(async move {
    let mut rx = portfolio_rx;
    while let Some(ChannelData::Portfolio(data)) = rx.recv().await {
        // Create snapshot
        let snapshot = PortfolioSnapshot {
            currency: data.currency.clone(),
            timestamp: data.timestamp.unwrap_or(0),
            margin_ratio: data.margin_ratio(),
            free_balance: data.free_balance(),
            delta_exposure: data.delta_exposure(),
            session_pnl: data.session_upl + data.session_rpl,
            options_gamma: data.options_gamma,
            options_vega: data.options_vega,
            options_theta: data.options_theta,
        };

        // Evaluate alerts
        let alerts = portfolio_alert_handler.evaluate(&snapshot).await;

        // Send alerts to notification handler
        for alert in alerts {
            // Send to alert channel
        }
    }
});
```

- [ ] **Step 5: Add config validation**

```rust
// Validate that auth is configured if portfolio subscription is used
if config.alerts.metrics.iter().any(|m| m.enabled()) {
    if config.data_sources.deribit.auth.is_none() {
        eprintln!("Warning: Portfolio metrics enabled but no auth configured");
    }
}
```

- [ ] **Step 6: Run build**

```bash
cargo build --release
```
Expected: Build succeeds

- [ ] **Step 7: Commit**

```bash
git add crates/vol-monitor/src/main.rs
git commit -m "feat: wire up portfolio subscription and alerts in main"
```

---

## Self-Review Checklist

1. **Spec coverage:** All requirements covered (auth config, portfolio subscription, metrics, JSONL output)
2. **No placeholders:** All code steps have complete implementations
3. **Type consistency:** `MetricConfig`, `PortfolioData`, `ChannelType` used consistently across tasks

---

Plan complete and saved to `docs/superpowers/plans/2026-03-31-deribit-auth-portfolio-monitor.md`.

Two execution options:

1. **Subagent-Driven (recommended)** - Dispatch fresh subagent per task with two-stage review (spec compliance + code quality)

2. **Inline Execution** - Execute tasks in this session using executing-plans

Which approach?
