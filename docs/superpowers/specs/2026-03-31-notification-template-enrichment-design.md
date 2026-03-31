# Notification Template Enrichment Design

**Date:** 2026-03-31
**Author:** Claude Code
**Status:** Approved

## Overview

Enrich the notification template and Feishu card format with additional market data fields to provide more context for traders when alerts fire.

## Current State

### Current Template Fields
```toml
message_template = "🚨 {tenor} {alert_type}: {symbol} IV={value}"
```

| Field | Source | Description |
|-------|--------|-------------|
| `{tenor}` | `Alert.tenor` | short/medium/long |
| `{alert_type}` | `Alert.alert_type` | absolute_iv/rate_change/etc. |
| `{symbol}` | `Alert.symbol` | Option symbol |
| `{value}` | `Alert.iv` | IV percentage |

### Current Feishu Card
Simple card with:
- Contract name
- Tenor type (短期/中期/长期)
- Option type (Call/Put)
- IV value

## Problem

The current notification lacks critical trading context:
1. **No index price** - Traders need to know the underlying price (e.g., BTC at $95,000)
2. **No DTE** - How many days until expiry
3. **No option price** - What's the mark price of the option itself
4. **No moneyness** - Is the option ITM or OTM

Traders currently need to switch to Deribit to look up this information.

## Solution

Add 5 new template fields by extracting data from `VolatilityData` and enriching the `Alert` structure.

### New Fields

| Field | Source | Example | Description |
|-------|--------|---------|-------------|
| `{index_price}` | `VolatilityData.index_price` | 95123.45 | Underlying index price |
| `{dte}` | `VolatilityData.dte` | 28 | Days to expiry |
| `{option_type}` | `VolatilityData.option_type` | C/P | Call or Put |
| `{moneyness}` | `VolatilityData.moneyness()` | ITM +2.3% | How far ITM/OTM |
| `{mark_price}` | `VolatilityData.extra["mark_price"]` | 1250.50 | Option mark price |

### Default Template Update
```toml
message_template = "🚨 {tenor} {alert_type}: {symbol} | IV={value:.1}% | 指数={index_price} | DTE={dte}天 | {option_type} | 价格={mark_price}"
```

Example output:
```
🚨 short absolute_iv: BTC-29MAR24-70000-C | IV=72.5% | 指数=95123 | DTE=28 天 | C | 价格=1250.50
```

### Feishu Card Redesign

```
┌─────────────────────────────────────────────────┐
│  🚨 IV 阈值告警                    [红色背景]   │
├─────────────────────────────────────────────────┤
│  **合约**: BTC-29MAR24-70000-C                  │
│  **期限**: 短期 | **类型**: Call                │
│  **IV**: 72.5%                                  │
│  **指数价格**: 95,123.45 USD                    │
│  **DTE**: 28 天                                 │
│  **合约价格**: 1,250.50 USD                     │
│  **实虚值**: ITM +2.3%                          │
├─────────────────────────────────────────────────┤
│  Deribit Volatility Monitor                     │
└─────────────────────────────────────────────────┘
```

## Architecture

### Data Flow

```
Deribit WebSocket → DeribitDataSource → VolatilityData
                                              ↓
                                   (includes index_price,
                                    dte, option_type,
                                    mark_price in extra)
                                              ↓
                                    AlertHandler.evaluate()
                                              ↓
                                    Alert { tenor, iv, symbol,
                                            index_price, dte,
                                            option_type, moneyness,
                                            mark_price }
                                              ↓
                                    NotificationHandler.format()
                                              ↓
                                    stdout / Feishu card
```

### Implementation Details

#### 1. Add fields to `Alert` struct (`vol-core/src/event.rs`)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub alert_type: AlertType,
    pub tenor: Tenor,
    pub symbol: String,
    pub iv: f64,
    pub message: String,
    pub timestamp: u64,
    pub source: String,

    // NEW FIELDS
    /// Underlying index price (e.g., BTC/USD)
    pub index_price: f64,

    /// Days to expiry
    pub dte: u32,

    /// Option type (Call/Put)
    pub option_type: OptionType,

    /// Moneyness percentage (positive = ITM, negative = OTM)
    pub moneyness: f64,

    /// Option mark price in USD
    pub mark_price: f64,
}
```

#### 2. Update `DeribitDataSource` to populate extra fields (`vol-datasource/src/deribit.rs`)

Extract `mark_price` from `OptionMarkPrice` and add to `VolatilityData.extra`:

```rust
VolatilityData {
    symbol: ...,
    dte: ...,
    iv: ...,
    timestamp: ...,
    source: "deribit".to_string(),
    strike: ...,
    option_type: ...,
    index_price: ...,
    delta: None,
    extra: {
        "mark_price": json!(mark_price),
    },
}
```

#### 3. Update `AlertHandler`s to copy fields from `VolatilityData` to `Alert`

All 4 handlers in `vol-alert/` need to be updated:
- `AbsoluteIvHandler`
- `RateChangeHandler`
- `TermStructureHandler`
- `SkewHandler`

Each `evaluate()` method that returns `Some(Alert)` should:
```rust
Some(Alert {
    alert_type: ...,
    tenor: data.tenor(),
    symbol: data.symbol.clone(),
    iv: data.iv,
    message: "...".to_string(),
    timestamp: data.timestamp,
    source: data.source.clone(),
    index_price: data.index_price,
    dte: data.dte,
    option_type: data.option_type,
    moneyness: data.moneyness(),
    mark_price: *data.extra.get("mark_price").as_f64().unwrap_or(&0.0),
})
```

#### 4. Update `FeishuNotification.format_message()` (`vol-notification/src/feishu.rs`)

Replace the template variables:

```rust
fn format_message(&self, alert: &Alert) -> String {
    self.message_template
        .replace("{tenor}", &alert.tenor.to_string())
        .replace("{alert_type}", &alert.alert_type.to_string())
        .replace("{symbol}", &alert.symbol)
        .replace("{value}", &format!("{:.1}%", alert.iv * 100.0))
        .replace("{index_price}", &format!("{:.2}", alert.index_price))
        .replace("{dte}", &alert.dte.to_string())
        .replace("{option_type}", &alert.option_type.to_string())
        .replace("{moneyness}", &format!(
            "{}{:.1}%",
            if alert.moneyness > 0.0 { "ITM +" } else { "OTM " },
            alert.moneyness.abs() * 100.0
        ))
        .replace("{mark_price}", &format!("{:.2}", alert.mark_price))
}
```

#### 5. Update `FeishuNotification.format_interactive_card()`

Redesign the card with all new fields:

```rust
fn format_interactive_card(&self, alert: &Alert) -> String {
    let title = match &alert.alert_type {
        AlertType::AbsoluteIv { .. } => "🚨 IV 阈值告警",
        AlertType::RateChange { .. } => "📈 IV 快速变化告警",
        AlertType::TermStructure { .. } => "📊 期限结构异常告警",
        AlertType::Skew { .. } => "⚖️ Skew 偏离告警",
    };

    let tenor_cn = match alert.tenor {
        Tenor::Short => "短期",
        Tenor::Medium => "中期",
        Tenor::Long => "长期",
    };

    let option_type_cn = match alert.option_type {
        OptionType::Call => "Call",
        OptionType::Put => "Put",
    };

    let moneyness_str = if alert.moneyness > 0.0 {
        format!("ITM +{:.1}%", alert.moneyness * 100.0)
    } else {
        format!("OTM {:.1}%", alert.moneyness * 100.0)
    };

    serde_json::to_string(&serde_json::json!({
        "config": {
            "wide_screen_mode": true
        },
        "header": {
            "title": {
                "tag": "plain_text",
                "content": title
            },
            "template": "red"
        },
        "elements": [
            {
                "tag": "div",
                "text": {
                    "tag": "lark_md",
                    "content": format!(
                        "**合约**: {}\n**期限**: {} | **类型**: {}\n**IV**: {:.1}%\n**指数价格**: {:.2} USD\n**DTE**: {} 天\n**合约价格**: {:.2} USD\n**实虚值**: {}",
                        alert.symbol,
                        tenor_cn,
                        option_type_cn,
                        alert.iv * 100.0,
                        alert.index_price,
                        alert.dte,
                        alert.mark_price,
                        moneyness_str
                    )
                }
            },
            {
                "tag": "hr"
            },
            {
                "tag": "note",
                "elements": [
                    {
                        "tag": "plain_text",
                        "content": "Deribit Volatility Monitor"
                    }
                ]
            }
        ]
    })).unwrap_or_default()
}
```

#### 6. Update `StdoutNotification` (`vol-notification/src/stdout.rs`)

```rust
async fn send(&self, alert: &Alert) -> Result<()> {
    let message = format!(
        "[ALERT] {} | {} | {} | IV: {:.1}% | 指数：{:.2} | DTE: {}天 | {} | 价格：{:.2}",
        alert.tenor,
        alert.alert_type,
        alert.symbol,
        alert.iv * 100.0,
        alert.index_price,
        alert.dte,
        alert.option_type,
        alert.mark_price,
    );
    info!("{}", message);
    println!("{}", message);
    Ok(())
}
```

#### 7. Update default config (`vol-config/src/lib.rs`)

```rust
fn default_message_template() -> String {
    "🚨 {tenor} {alert_type}: {symbol} | IV={value:.1}% | 指数={index_price} | DTE={dte}天 | {option_type} | 价格={mark_price}".to_string()
}
```

## Files to Modify

| File | Change |
|------|--------|
| `vol-core/src/event.rs` | Add 5 fields to `Alert` struct |
| `vol-core/src/alert.rs` | Update `Alert::new()` signature |
| `vol-datasource/src/deribit.rs` | Populate `mark_price` in `VolatilityData.extra` |
| `vol-alert/src/absolute_iv.rs` | Copy fields to `Alert` |
| `vol-alert/src/rate_change.rs` | Copy fields to `Alert` |
| `vol-alert/src/term_structure.rs` | Copy fields to `Alert` |
| `vol-alert/src/skew.rs` | Copy fields to `Alert` |
| `vol-notification/src/feishu.rs` | Update `format_message()` and `format_interactive_card()` |
| `vol-notification/src/stdout.rs` | Update `send()` |
| `vol-config/src/lib.rs` | Update `default_message_template()` |
| `config.toml` | Update user's config with new template |

## Error Handling

1. **Missing `mark_price`**: Default to `0.0` if not found in `extra`
2. **Template field missing**: Template `replace()` will leave literal `{field_name}` if `Alert` doesn't have the data - this is acceptable as it indicates a bug

## Testing

1. **Unit tests** for `format_message()` with all template variables
2. **Unit tests** for `format_interactive_card()` JSON structure
3. **Integration test**: Run monitor, trigger alert, verify stdout output

## Trade-offs

| Aspect | Before | After |
|--------|--------|-------|
| Template fields | 4 | 9 |
| Alert struct size | ~100 bytes | ~150 bytes |
| Feishu card info | 4 lines | 8 lines |
| Trader context | Limited | Complete |
| Breaking change | N/A | Yes - requires config update |

## Migration

Users with existing `config.toml` need to update their `message_template`:

**Old:**
```toml
message_template = "🚨 {tenor} {alert_type}: {symbol} IV={value}"
```

**New:**
```toml
message_template = "🚨 {tenor} {alert_type}: {symbol} | IV={value:.1}% | 指数={index_price} | DTE={dte}天 | {option_type} | 价格={mark_price}"
```

The code change updates the default, but existing configs won't break - they'll just use the old format until manually updated.
