# Portfolio & Greeks Monitoring Design Spec

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add portfolio position and Greeks monitoring to vol-monitor, reusing existing Rule/Alert/Notification architecture.

**Architecture:** Extend the existing DataSource → Rule → Alert → Notification pipeline:
1. Add `PortfolioDataSource` that subscribes to Deribit `user.portfolio.*` WebSocket channels and polls `private/get_positions` REST API
2. Emit `PortfolioUpdate` events to the existing event bus
3. Add new Rule types (`DeltaExposureRule`, `GammaExposureRule`, `GreeksChangeRateRule`, `ConcentrationRule`, `MarginRatioRule`)
4. Reuse existing `AlertManager`, cooldown mechanism, and `NotificationHandler` implementations

**Tech Stack:** Rust, tokio async, Deribit API (WebSocket + REST), existing vol-core traits

---

## 1. Overview

### 1.1 Purpose

Add real-time monitoring of user's Deribit portfolio:
- Account-level Greeks (total delta, gamma, vega, theta)
- Position-level Greeks (per-contract and per-underlying)
- Margin and balance metrics
- Configurable alert rules with cooldown

### 1.2 Scope

**In Scope:**
- `PortfolioDataSource` implementation (WebSocket subscription + REST polling)
- Portfolio data models (`PortfolioSnapshot`, `Position` with Greeks)
- New Rule types for Greeks/margin/concentration monitoring
- Configuration in `config.toml`
- Feishu/Stdout notifications (reuse existing)

**Out of Scope:**
- Historical data persistence (only in-memory cache for rate calculations)
- Trading/hedging recommendations (alert-only)
- Multi-exchange portfolio aggregation

---

## 2. Architecture

### 2.1 Data Flow

```
┌─────────────────────────────────────────────────────────────┐
│  PortfolioDataSource                                         │
│  - WebSocket: user.portfolio.btc, user.portfolio.eth         │
│  - REST Poll: private/get_positions (every 30-60s)           │
│  - Reuse existing Deribit authentication                     │
└─────────────────────────────────────────────────────────────┘
                          │
                          │ PortfolioUpdate events
                          ↓
┌─────────────────────────────────────────────────────────────┐
│  EventBus (tokio::broadcast<MonitoringEvent>)                │
│  - Existing event bus, new event variant                     │
└─────────────────────────────────────────────────────────────┘
                          │
                          │ Filtered events
                          ↓
┌─────────────────────────────────────────────────────────────┐
│  Rule Processors                                             │
│  - DeltaExposureRule: abs(delta) > threshold                 │
│  - GammaExposureRule: abs(gamma) > threshold                 │
│  - VegaExposureRule: abs(vega) > threshold                   │
│  - ThetaExposureRule: abs(theta) > threshold                 │
│  - GreeksChangeRateRule: delta/gamma/vega change over time   │
│  - ConcentrationRule: single position > X% of total          │
│  - MarginRatioRule: margin_balance / initial_margin          │
└─────────────────────────────────────────────────────────────┘
                          │
                          │ Alert
                          ↓
┌─────────────────────────────────────────────────────────────┐
│  AlertManager (existing)                                     │
│  - Cooldown check (configurable per rule type)               │
└─────────────────────────────────────────────────────────────┘
                          │
                          ↓
┌─────────────────────────────────────────────────────────────┐
│  NotificationHandler (existing)                              │
│  - Feishu, Stdout                                            │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 Key Design Decisions

1. **Reuse existing Rule trait** - No separate "PortfolioRule" trait needed. Each Greeks rule implements `RuleProcessor` from `vol-core`.

2. **In-memory snapshot cache** - Keep last N portfolio snapshots in memory for change-rate calculations (e.g., "gamma increased 50% in 1 hour").

3. **Two data sources merged**:
   - WebSocket (`user.portfolio.*`) - Real-time account summary
   - REST polling (`private/get_positions`) - Detailed per-position data

4. **Configuration unified** - Greeks rules use same `[[rules]]` array in `config.toml`, not a separate section.

---

## 3. Components

### 3.1 PortfolioDataSource (`vol-datasource/src/portfolio.rs`)

```rust
pub struct PortfolioDataSource {
    id: String,
    client: DeribitClient,  // Reuse existing client
    poll_interval_secs: u64,
    currencies: Vec<String>,  // ["BTC", "ETH", "USDC"]
}

impl DataSource for PortfolioDataSource {
    fn id(&self) -> &str;
    fn event_type(&self) -> EventType;  // EventType::Portfolio
    async fn connect(&mut self) -> Result<()>;
    async fn run(&self, tx: mpsc::Sender<MonitoringEvent>) -> Result<()>;
}
```

**Responsibilities:**
- Subscribe to `user.portfolio.{currency}` WebSocket channels
- Poll `private/get_positions` every N seconds
- Merge WebSocket summary + REST positions into `PortfolioSnapshot`
- Emit `MonitoringEvent::Portfolio(snapshot)` to event bus

### 3.2 Portfolio Data Models (`vol-deribit/src/portfolio.rs` - extend existing)

```rust
/// Extended with per-position Greeks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub instrument_name: String,
    pub size: f64,  // Positive = long, negative = short
    pub average_price: f64,
    pub mark_price: f64,
    pub unrealized_pnl: f64,
    pub delta: f64,
    pub gamma: f64,
    pub vega: f64,
    pub theta: f64,
    pub dte: u32,
    pub underlying: String,  // "BTC" or "ETH"
}

/// Snapshot combining account summary + positions
#[derive(Debug, Clone)]
pub struct PortfolioSnapshot {
    pub timestamp: u64,
    pub currency: String,
    pub equity: f64,
    pub balance: f64,
    pub available_funds: f64,
    pub initial_margin: f64,
    pub maintenance_margin: f64,
    pub delta_total: f64,
    pub gamma_total: f64,
    pub vega_total: f64,
    pub theta_total: f64,
    pub delta_by_underlying: HashMap<String, f64>,
    pub positions: Vec<Position>,
}
```

### 3.3 New Rule Types (`vol-rules/src/`)

| Rule | File | Config | Trigger Condition |
|------|------|--------|-------------------|
| `DeltaExposureRule` | `delta_exposure.rs` | `max_threshold`, `min_threshold` | `abs(delta_total) > max OR abs(delta_total) < min` |
| `GammaExposureRule` | `gamma_exposure.rs` | `max_threshold` | `abs(gamma_total) > max` |
| `VegaExposureRule` | `vega_exposure.rs` | `max_threshold` | `abs(vega_total) > max` |
| `ThetaExposureRule` | `theta_exposure.rs` | `max_threshold` | `abs(theta_total) > max` |
| `GreeksChangeRateRule` | `greeks_change_rate.rs` | `greek_type`, `window_minutes`, `threshold` | `abs(current - prev) / abs(prev) > threshold` |
| `ConcentrationRule` | `concentration.rs` | `max_position_pct` | `abs(position_delta) / abs(total_delta) > pct` |
| `MarginRatioRule` | `margin_ratio.rs` | `min_threshold` | `margin_balance / initial_margin < min` |

**Example rule implementation pattern:**

```rust
pub struct DeltaExposureRule {
    config: DeltaExposureConfig,
    id: String,
}

impl RuleProcessor for DeltaExposureRule {
    fn id(&self) -> &str { &self.id }
    
    fn interests(&self) -> &[EventType] { &[EventType::Portfolio] }
    
    fn process(&self, event: &MonitoringEvent) -> Result<Option<Alert>> {
        let MonitoringEvent::Portfolio(snapshot) = event else { return Ok(None) };
        
        if snapshot.delta_total.abs() > self.config.max_threshold {
            return Ok(Some(self.create_alert(snapshot)));
        }
        Ok(None)
    }
}
```

### 3.4 Configuration (`config.toml`)

```toml
# Portfolio data source (uses existing Deribit auth)
[[datasources]]
id = "portfolio"
provider = "deribit-portfolio"
currencies = ["BTC", "ETH"]
poll_interval_secs = 30
enabled = true

# Greeks exposure rules
[[rules]]
id = "delta-limit"
type = "delta-exposure"
max_threshold = 100.0
enabled = true
notifications = ["feishu-alerts", "stdout"]

[[rules]]
id = "gamma-limit"
type = "gamma-exposure"
max_threshold = 50.0
enabled = true
notifications = ["feishu-alerts"]

[[rules]]
id = "vega-limit"
type = "vega-exposure"
max_threshold = 200.0
enabled = true
notifications = ["feishu-alerts"]

# Change rate rules
[[rules]]
id = "gamma-spike-1h"
type = "greeks-change-rate"
greek_type = "gamma"
window_minutes = 60
threshold = 0.50  # 50% change
enabled = true
notifications = ["feishu-alerts"]

# Concentration rules
[[rules]]
id = "position-concentration"
type = "concentration"
max_position_pct = 0.30  # No single position > 30% of total delta
enabled = true
notifications = ["feishu-alerts"]

# Margin rules
[[rules]]
id = "margin-ratio"
type = "margin-ratio"
min_threshold = 1.25
enabled = true
notifications = ["feishu-alerts"]
```

### 3.5 Alert Message Format

```
🚨 Delta 敞口告警
规则：delta-limit
账户总 Delta: 150.5 (阈值：100.0)
- BTC Delta: 120.3
- ETH Delta: 30.2
时间：2026-04-04 12:00:00 UTC
```

---

## 4. Error Handling

| Error | Handling |
|-------|----------|
| Deribit API auth failure | Log error, retry with exponential backoff, emit Degraded health status |
| WebSocket disconnect | Auto-reconnect, buffer events during reconnect |
| REST rate limit hit | Slow down polling (increase interval), log warning |
| Incomplete snapshot | Skip rule evaluation for this cycle, log debug message |

---

## 5. Testing

### 5.1 Unit Tests

- `PortfolioDataSource::merge_snapshot()` - WebSocket + REST data merge
- Each rule's `process()` method with mock `PortfolioSnapshot`
- Change-rate calculation with synthetic historical data

### 5.2 Integration Tests

- Mock Deribit API responses
- Verify event emission to bus
- Verify alert generation when thresholds exceeded

### 5.3 Manual Testing

- Connect to Deribit testnet (if available)
- Use small account with known positions
- Verify alert notifications in Feishu

---

## 6. Implementation Order

1. **Extend data models** - Add per-position Greeks to `PortfolioData`
2. **Implement PortfolioDataSource** - WebSocket subscription + REST polling
3. **Add EventType::Portfolio** - Extend `vol-core/src/event.rs`
4. **Implement Greeks rules** - One PR per rule type (small, reviewable)
5. **Update main.rs** - Register datasource and rules
6. **Update config.toml** - Add example configuration
7. **Documentation** - Update README with new features

---

## 7. Future Extensions (Not in Initial Scope)

- Historical data persistence (SQLite/PostgreSQL)
- P&L attribution analysis
- Scenario analysis ("what if BTC drops 10%")
- Hedging suggestions
- Multi-account aggregation
