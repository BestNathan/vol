# Parse and Route Optimization Design

**Date:** 2026-03-31
**Author:** Claude Code
**Status:** Approved

## Overview

Refactor `parse_and_route` in `crates/vol-deribit/src/client.rs` to use serde's `#[serde(untagged)]` enum for more efficient message parsing.

## Problem

Current implementation attempts to parse each WebSocket message up to 4 times:

```rust
// Current code (lines 547-606)
fn parse_and_route(text: &str) -> Option<(ChannelType, ChannelData)> {
    if let Ok(notification) = serde_json::from_str::<SubscriptionNotification<Vec<OptionMarkPrice>>>(text) { ... }
    if let Ok(notification) = serde_json::from_str::<SubscriptionNotification<PriceIndex>>(text) { ... }
    if let Ok(notification) = serde_json::from_str::<SubscriptionNotification<Vec<DeribitTicker>>>(text) { ... }
    if let Ok(notification) = serde_json::from_str::<SubscriptionNotification<Vec<Trade>>>(text) { ... }
    None
}
```

Issues:
1. **Performance**: Each message may be parsed up to 4 times before finding the correct type
2. **Code clarity**: Repetitive pattern with 4 nearly-identical blocks
3. **Maintainability**: Adding new message types requires copying the entire block

## Solution

Use serde's `#[serde(untagged)]` enum to let serde handle type discrimination internally.

### Architecture

```
┌─────────────────────────────────────────────────────────┐
│                  WebSocket Message                       │
│                     (JSON text)                          │
└─────────────────────┬───────────────────────────────────┘
                      │
                      ▼
         ┌────────────────────────┐
         │   serde_json::from_str │
         │   <DeribitNotification>│
         └────────────┬───────────┘
                      │
                      ▼
    ┌─────────────────────────────────┐
    │  DeribitNotification (untagged) │
    │  ┌──────────────────────────┐   │
    │  │ Markprice(Vec<...>)      │   │
    │  │ PriceIndex(...)          │   │  ← serde tries in order
    │  │ Ticker(Vec<...>)         │   │
    │  │ Trade(Vec<...>)          │   │
    │  └──────────────────────────┘   │
    └─────────────┬───────────────────┘
                  │
                  ▼
         match notification { ... }
                  │
                  ▼
    (ChannelType, ChannelData)
```

### Implementation Details

**New enum definition** (in `message.rs` or inline in `client.rs`):

```rust
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum DeribitNotification {
    Markprice(SubscriptionNotification<Vec<OptionMarkPrice>>),
    PriceIndex(SubscriptionNotification<PriceIndex>),
    Ticker(SubscriptionNotification<Vec<DeribitTicker>>),
    Trade(SubscriptionNotification<Vec<Trade>>),
}
```

**Simplified parse_and_route**:

```rust
fn parse_and_route(text: &str) -> Option<(ChannelType, ChannelData)> {
    let notification = serde_json::from_str::<DeribitNotification>(text).ok()?;

    match notification {
        DeribitNotification::Markprice(n) => {
            if n.method != "subscription" { return None; }
            let index = n.params.channel.strip_prefix("markprice.options.")?.to_string();
            Some((ChannelType::MarkpriceOptions(index), ChannelData::OptionMarkPrice(n.params.data)))
        }
        DeribitNotification::PriceIndex(n) => {
            if n.method != "subscription" { return None; }
            let index = n.params.channel.strip_prefix("deribit_price_index.")?.to_string();
            Some((ChannelType::PriceIndex(index), ChannelData::PriceIndex(n.params.data)))
        }
        DeribitNotification::Ticker(n) => {
            if n.method != "subscription" { return None; }
            let base = n.params.channel.strip_prefix("ticker.")?.split('.').next()?.to_string();
            let ticker = n.params.data.into_iter().next()?;
            Some((ChannelType::Ticker(base), ChannelData::Ticker(ticker)))
        }
        DeribitNotification::Trade(n) => {
            if n.method != "subscription" { return None; }
            let instrument = n.params.channel.strip_prefix("trades.")?.to_string();
            let trade = n.params.data.into_iter().next()?;
            Some((ChannelType::Trade(instrument), ChannelData::Trade(trade)))
        }
    }
}
```

### Trade-offs

| Aspect | Before | After |
|--------|--------|-------|
| Parse attempts per message | Up to 4 | 1 (serde internal) |
| Lines of code | ~60 | ~40 |
| Adding new type | Copy block + edit | Add enum variant + match arm |
| Type safety | Full | Full |

### Why untagged works here

1. **Structural differences**: Each notification type has distinct payload structure:
   - `OptionMarkPrice`: array with `price`, `instrument_name`, `expiration`, `iv`, etc.
   - `PriceIndex`: single object with `price`, `time`
   - `DeribitTicker`: array with `last_price`, `mark_price`, `volume`, etc.
   - `Trade`: array with `trade_seq`, `price`, `amount`, `side`, etc.

2. **serde's fail-fast**: When deserializing fails, serde stops at the first mismatched field rather than parsing the entire payload

3. **Order matters**: Enum variants are tried in definition order. Current order is fine as `Markprice` and `Ticker` are the most common.

## Error Handling

- Failed JSON parse → `None`
- Failed type match → `None`
- Missing channel prefix → `None`
- Empty data array → `None`

Behavior remains identical to current implementation.

## Testing

Existing tests should pass without modification:
- `test_notification_parse` in `message.rs`
- `test_dispatch_sends_to_subscribers` in `subscription_manager.rs`

## Files to Modify

1. `crates/vol-deribit/src/client.rs` — `parse_and_route` method
2. Optionally `crates/vol-deribit/src/message.rs` — add `DeribitNotification` enum
