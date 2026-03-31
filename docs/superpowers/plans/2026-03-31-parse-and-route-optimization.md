# Parse and Route Optimization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor `parse_and_route` to use serde's `#[serde(untagged)]` enum for more efficient message parsing.

**Architecture:** Add a new `DeribitNotification` untagged enum in `message.rs`, then simplify `parse_and_route` in `client.rs` to use a single `from_str` call followed by a match.

**Tech Stack:** Rust, serde, serde_json, tokio-tungstenite

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `crates/vol-deribit/src/message.rs` | Modify | Add `DeribitNotification` untagged enum |
| `crates/vol-deribit/src/client.rs` | Modify | Simplify `parse_and_route` method |
| `crates/vol-deribit/src/client.rs` | Modify | Update imports if needed |

No new files needed. Existing test files should pass without modification.

---

### Task 1: Add `DeribitNotification` enum to `message.rs`

**Files:**
- Modify: `crates/vol-deribit/src/message.rs`
- Test: `cargo test -p vol-deribit`

- [ ] **Step 1: Add imports for all required types**

At the top of `message.rs`, ensure all types are imported:

```rust
use crate::{OptionMarkPrice, PriceIndex, DeribitTicker, Trade};
```

- [ ] **Step 2: Add the `DeribitNotification` enum**

Add after the `ChannelData` enum (around line 43):

```rust
/// Untagged enum for Deribit WebSocket notifications.
///
/// Serde tries variants in order, using fail-fast deserialization.
/// This is more efficient than manually trying each type.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum DeribitNotification {
    Markprice(SubscriptionNotification<Vec<OptionMarkPrice>>),
    PriceIndex(SubscriptionNotification<PriceIndex>),
    Ticker(SubscriptionNotification<Vec<DeribitTicker>>),
    Trade(SubscriptionNotification<Vec<Trade>>),
}
```

- [ ] **Step 3: Verify compilation**

Run:
```bash
cargo check -p vol-deribit
```

Expected: PASS (no new errors)

- [ ] **Step 4: Commit**

```bash
git add crates/vol-deribit/src/message.rs
git commit -m "feat: add DeribitNotification untagged enum for efficient parsing"
```

---

### Task 2: Refactor `parse_and_route` to use the new enum

**Files:**
- Modify: `crates/vol-deribit/src/client.rs:546-606`
- Test: `cargo test -p vol-deribit`

- [ ] **Step 1: Replace the `parse_and_route` method**

Replace lines 546-606 with:

```rust
/// Parse message and extract channel type and data.
///
/// Uses a single JSON parse with an untagged enum to discriminate
/// the notification type, which is more efficient than trying
/// each type sequentially.
fn parse_and_route(text: &str) -> Option<(ChannelType, ChannelData)> {
    let notification = serde_json::from_str::<DeribitNotification>(text).ok()?;

    match notification {
        DeribitNotification::Markprice(n) => {
            if n.method != "subscription" {
                return None;
            }
            let index = n.params.channel.strip_prefix("markprice.options.")?.to_string();
            Some((ChannelType::MarkpriceOptions(index), ChannelData::OptionMarkPrice(n.params.data)))
        }
        DeribitNotification::PriceIndex(n) => {
            if n.method != "subscription" {
                return None;
            }
            let index = n.params.channel.strip_prefix("deribit_price_index.")?.to_string();
            Some((ChannelType::PriceIndex(index), ChannelData::PriceIndex(n.params.data)))
        }
        DeribitNotification::Ticker(n) => {
            if n.method != "subscription" {
                return None;
            }
            let base = n.params.channel.strip_prefix("ticker.")?.split('.').next()?.to_string();
            let ticker = n.params.data.into_iter().next()?;
            Some((ChannelType::Ticker(base), ChannelData::Ticker(ticker)))
        }
        DeribitNotification::Trade(n) => {
            if n.method != "subscription" {
                return None;
            }
            let instrument = n.params.channel.strip_prefix("trades.")?.to_string();
            let trade = n.params.data.into_iter().next()?;
            Some((ChannelType::Trade(instrument), ChannelData::Trade(trade)))
        }
    }
}
```

- [ ] **Step 2: Verify compilation**

Run:
```bash
cargo check -p vol-deribit
```

Expected: PASS

- [ ] **Step 3: Run unit tests**

Run:
```bash
cargo test -p vol-deribit
```

Expected: All 15 tests pass (same as before)

- [ ] **Step 4: Run full workspace tests**

Run:
```bash
cargo test --workspace
```

Expected: All 21 tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/vol-deribit/src/client.rs
git commit -m "refactor: simplify parse_and_route using DeribitNotification enum"
```

---

### Task 3: Add unit test for `DeribitNotification` parsing

**Files:**
- Modify: `crates/vol-deribit/src/message.rs` (add test module)
- Test: `cargo test -p vol-deribit test_notification_parse`

- [ ] **Step 1: Add test for the new enum**

Add at the end of `message.rs` (or extend existing test module):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deribit_notification_markprice() {
        let json = r#"{
            "jsonrpc": "2.0",
            "method": "subscription",
            "params": {
                "channel": "markprice.options.btc_usd",
                "data": [{"instrument_name": "BTC-29MAR24-70000-C", "price": 0.0123, "iv": 0.65}]
            }
        }"#;

        let notification: DeribitNotification = serde_json::from_str(json).unwrap();
        assert!(matches!(notification, DeribitNotification::Markprice(_)));
    }

    #[test]
    fn test_deribit_notification_price_index() {
        let json = r#"{
            "jsonrpc": "2.0",
            "method": "subscription",
            "params": {
                "channel": "deribit_price_index.btc_usd",
                "data": {"price": 45000.0, "time": 1234567890}
            }
        }"#;

        let notification: DeribitNotification = serde_json::from_str(json).unwrap();
        assert!(matches!(notification, DeribitNotification::PriceIndex(_)));
    }

    #[test]
    fn test_deribit_notification_ticker() {
        let json = r#"{
            "jsonrpc": "2.0",
            "method": "subscription",
            "params": {
                "channel": "ticker.BTC.raw",
                "data": [{"last_price": 45000.0}]
            }
        }"#;

        let notification: DeribitNotification = serde_json::from_str(json).unwrap();
        assert!(matches!(notification, DeribitNotification::Ticker(_)));
    }

    #[test]
    fn test_deribit_notification_trade() {
        let json = r#"{
            "jsonrpc": "2.0",
            "method": "subscription",
            "params": {
                "channel": "trades.BTC-PERPETUAL.raw",
                "data": [{"trade_seq": 123, "price": 45000.0, "amount": 0.1, "side": "buy"}]
            }
        }"#;

        let notification: DeribitNotification = serde_json::from_str(json).unwrap();
        assert!(matches!(notification, DeribitNotification::Trade(_)));
    }
}
```

- [ ] **Step 2: Run new tests**

Run:
```bash
cargo test -p vol-deribit test_deribit_notification
```

Expected: 4 tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/vol-deribit/src/message.rs
git commit -m "test: add unit tests for DeribitNotification parsing"
```

---

### Task 4: Integration test with real Deribit data

**Files:**
- Test: `cargo test --workspace` and manual run

- [ ] **Step 1: Run full test suite**

Run:
```bash
cargo test --workspace
```

Expected: All tests pass (21+ tests)

- [ ] **Step 2: Build and run the monitor**

Run:
```bash
cargo build --release
HTTPS_PROXY=http://192.168.2.98:8890 timeout 10 ./target/release/vol-monitor 2>&1 | head -50
```

Expected: Monitor starts, receives data, no parse errors

- [ ] **Step 3: Commit final changes if any**

```bash
git add .
git commit -m "chore: verify integration with live Deribit data"
```

---

## Self-Review Checklist

**1. Spec coverage:**
- [x] Add `DeribitNotification` untagged enum → Task 1
- [x] Simplify `parse_and_route` → Task 2
- [x] Error handling identical to before → Task 2 (returns `None` on any failure)
- [x] Tests pass → Task 2, 3, 4

**2. Placeholder scan:** No TBD/TODO/add-vague-handling patterns found.

**3. Type consistency:**
- `DeribitNotification` variants match `ChannelData` variants
- `parse_and_route` signature unchanged: `fn(text: &str) -> Option<(ChannelType, ChannelData)>`
- All type names match existing definitions

**4. Scope:** Single function refactoring — appropriate for one plan.

---

## Execution Choice

Plan complete and saved to `docs/superpowers/plans/2026-03-31-parse-and-route-optimization.md`.

**Two execution options:**

1. **Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration

2. **Inline Execution** — Execute tasks in this session using executing-plans, batch execution with checkpoints

**Which approach?**
