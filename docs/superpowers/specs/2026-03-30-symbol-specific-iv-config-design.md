# Symbol-Specific IV Threshold Configuration Design

**Date:** 2026-03-30
**Author:** Claude
**Status:** Approved

## 1. Overview

### 1.1 Purpose

Enable per-symbol (BTC, ETH, etc.) configuration of IV thresholds and ATM moneyness thresholds in the volatility monitoring system. This allows different underlying assets to have customized alert sensitivities based on their distinct volatility characteristics.

### 1.2 Problem Statement

The current configuration uses global IV thresholds for all symbols:
- BTC and ETH have different volatility profiles (ETH typically has higher IV)
- A single threshold value cannot optimally serve both assets
- ETH may trigger too many alerts with BTC-tuned thresholds, or miss important signals with ETH-tuned thresholds

### 1.3 Requirements

1. **Per-symbol IV thresholds** — Each symbol (BTC, ETH) has independent short/medium/long IV threshold values
2. **Per-symbol ATM thresholds** — Each symbol has independent ATM moneyness threshold values (recognizing ETH has wider natural price swings)
3. **Backward compatible** — Existing configurations should continue to work
4. **Extensible** — Adding new symbols (e.g., SOL) should require only config changes, not code changes

## 2. Architecture

### 2.1 Current Architecture

```
┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
│  Config (TOML)  │ ──→ │ AbsoluteIvConfig │ ──→ │ AbsoluteIvHandler │
│  global values  │     │  single set      │     │  single threshold │
└─────────────────┘     └──────────────────┘     └─────────────────┘
```

### 2.2 New Architecture

```
┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
│  Config (TOML)  │ ──→ │ AbsoluteIvConfig │ ──→ │ AbsoluteIvHandler │
│  per-symbol     │     │  HashMap<Symbol, │     │  lookup by symbol │
│  sections       │     │  SymbolConfig>   │     │  at evaluation    │
└─────────────────┘     └──────────────────┘     └─────────────────┘
                                                      │
                                                      ▼
                                         ┌─────────────────────┐
                                         │ VolatilityData      │
                                         │ (symbol: "BTC")     │
                                         └─────────────────────┘
```

### 2.3 Data Flow

```
Incoming VolatilityData (symbol="ETH", iv=0.85, dte=5, ...)
         │
         ▼
AbsoluteIvHandler::evaluate()
         │
         ├─→ Extract symbol from VolatilityData ("ETH")
         │
         ├─→ Lookup SymbolConfig from HashMap
         │
         ├─→ Classify tenor (Short, Medium, Long)
         │
         ├─→ Get IV threshold for tenor (e.g., 0.90 for ETH Short)
         │
         ├─→ Get ATM threshold for tenor (e.g., 0.08 for ETH Short)
         │
         ├─→ Check is_atm(atm_threshold)
         │
         └─→ Check iv >= iv_threshold → Alert if both pass
```

## 3. Configuration Design

### 3.1 New Configuration Structure

```toml
[alerts.absolute_iv]
# No global thresholds here anymore

# Per-symbol configuration
[alerts.absolute_iv.btc]
short_threshold = 0.80
medium_threshold = 0.70
long_threshold = 0.60
short_atm_threshold = 0.05    # ±5% for short-term
medium_atm_threshold = 0.10   # ±10% for medium-term
long_atm_threshold = 0.15     # ±15% for long-term

[alerts.absolute_iv.eth]
short_threshold = 0.90
medium_threshold = 0.80
long_threshold = 0.70
short_atm_threshold = 0.08    # ±8% for short-term (ETH more volatile)
medium_atm_threshold = 0.12   # ±12% for medium-term
long_atm_threshold = 0.18     # ±18% for long-term
```

### 3.2 TOML Structure Notes

- Symbol names are lowercase keys (`btc`, `eth`, `sol`, etc.)
- All six fields (3 IV thresholds + 3 ATM thresholds) are required per symbol
- No fallback to global values — each symbol is self-contained

## 4. Code Changes

### 4.1 vol-config (`crates/vol-config/src/lib.rs`)

**Add new struct for per-symbol config:**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolIvConfig {
    pub short_threshold: f64,
    pub medium_threshold: f64,
    pub long_threshold: f64,
    pub short_atm_threshold: f64,
    pub medium_atm_threshold: f64,
    pub long_atm_threshold: f64,
}
```

**Modify AbsoluteIvConfig:**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbsoluteIvConfig {
    pub symbols: std::collections::HashMap<String, SymbolIvConfig>,
}
```

**Optional:** Provide helper method to get symbol config with fallback:

```rust
impl AbsoluteIvConfig {
    pub fn get_symbol_config(&self, symbol: &str) -> Option<&SymbolIvConfig> {
        self.symbols.get(&symbol.to_lowercase())
    }
}
```

### 4.2 vol-alert (`crates/vol-alert/src/absolute_iv.rs`)

**Modify AbsoluteIvHandler:**

```rust
pub struct AbsoluteIvHandler {
    config: AbsoluteIvConfig,  // Now contains HashMap
}

impl AbsoluteIvHandler {
    pub fn new(config: AbsoluteIvConfig) -> Self {
        Self { config }
    }

    fn get_symbol_config(&self, symbol: &str) -> Option<&SymbolIvConfig> {
        self.config.get_symbol_config(symbol)
    }
}
```

**Modify evaluate() method:**

```rust
fn evaluate(&self, data: &VolatilityData) -> Option<Alert> {
    // Extract symbol (e.g., "BTC" from "BTC-29MAR24-70000-C")
    let symbol_name = extract_symbol(&data.symbol)?;

    // Get symbol-specific config
    let symbol_config = self.get_symbol_config(symbol_name)?;

    let tenor = data.tenor();

    // Get thresholds from symbol config
    let iv_threshold = match tenor {
        Tenor::Short => symbol_config.short_threshold,
        Tenor::Medium => symbol_config.medium_threshold,
        Tenor::Long => symbol_config.long_threshold,
    };

    let atm_threshold = match tenor {
        Tenor::Short => symbol_config.short_atm_threshold,
        Tenor::Medium => symbol_config.medium_atm_threshold,
        Tenor::Long => symbol_config.long_atm_threshold,
    };

    // ATM filter
    if !data.is_atm(atm_threshold) {
        return None;
    }

    // IV threshold check
    if data.iv >= iv_threshold {
        // ... create alert
    }

    None
}
```

**Add helper to extract symbol from instrument name:**

```rust
/// Extract underlying symbol from instrument name
/// e.g., "BTC-29MAR24-70000-C" -> "BTC"
fn extract_symbol(instrument_name: &str) -> Option<&str> {
    instrument_name.split('-').next()
}
```

### 4.3 vol-monitor (`crates/vol-monitor/src/main.rs`)

**Update default config creation:**

```rust
fn create_default_config() -> Config {
    use vol_config::*;
    use std::collections::HashMap;

    let mut symbols = HashMap::new();

    // BTC config
    symbols.insert("btc".to_string(), SymbolIvConfig {
        short_threshold: 0.80,
        medium_threshold: 0.70,
        long_threshold: 0.60,
        short_atm_threshold: 0.05,
        medium_atm_threshold: 0.10,
        long_atm_threshold: 0.15,
    });

    // ETH config
    symbols.insert("eth".to_string(), SymbolIvConfig {
        short_threshold: 0.90,
        medium_threshold: 0.80,
        long_threshold: 0.70,
        short_atm_threshold: 0.08,
        medium_atm_threshold: 0.12,
        long_atm_threshold: 0.18,
    });

    Config {
        // ...
        alerts: AlertsConfig {
            absolute_iv: AbsoluteIvConfig { symbols },
            // ...
        },
    }
}
```

## 5. Error Handling

### 5.1 Missing Symbol Configuration

**Scenario:** Incoming data for symbol "SOL" but no config for "sol" in HashMap.

**Behavior:** Skip alert evaluation for that symbol (graceful degradation).

```rust
let symbol_config = match self.get_symbol_config(symbol_name) {
    Some(config) => config,
    None => {
        tracing::warn!("No IV config for symbol: {}", symbol_name);
        return None;  // Skip, don't alert
    }
};
```

### 5.2 Empty Symbol Map

**Scenario:** Config file has empty `[alerts.absolute_iv]` section.

**Behavior:** Log warning at startup, no IV alerts will fire.

## 6. Migration Path

### 6.1 For Existing Users

Users upgrading from the previous version will need to update their `config.toml`:

**Old format (no longer supported):**
```toml
[alerts.absolute_iv]
short_threshold = 0.80
medium_threshold = 0.70
long_threshold = 0.60
```

**New format:**
```toml
[alerts.absolute_iv.btc]
short_threshold = 0.80
medium_threshold = 0.70
long_threshold = 0.60
short_atm_threshold = 0.05
medium_atm_threshold = 0.10
long_atm_threshold = 0.15

[alerts.absolute_iv.eth]
short_threshold = 0.90
medium_threshold = 0.80
long_threshold = 0.70
short_atm_threshold = 0.08
medium_atm_threshold = 0.12
long_atm_threshold = 0.18
```

### 6.2 Backward Compatibility Option

If backward compatibility is desired, we could support both formats:
- Check for `[alerts.absolute_iv.btc]` first
- If not present, fall back to `[alerts.absolute_iv]` global values

This adds complexity. For now, we assume a clean migration is acceptable.

## 7. Testing

### 7.1 Unit Tests

1. **Symbol extraction** — Test `extract_symbol()` with various instrument names
2. **Config loading** — Test TOML parsing with new structure
3. **Threshold lookup** — Test that BTC data uses BTC thresholds, ETH uses ETH
4. **Missing symbol** — Test graceful handling of unknown symbols

### 7.2 Integration Tests

1. Run monitor with BTC and ETH symbols
2. Verify BTC IV at 75% does NOT trigger (threshold 80%)
3. Verify ETH IV at 85% does NOT trigger (threshold 90%)
4. Verify BTC IV at 82% DOES trigger (threshold 80%)
5. Verify ETH IV at 92% DOES trigger (threshold 90%)

## 8. Trade-offs and Alternatives

### 8.1 Considered Alternatives

| Approach | Pros | Cons |
|----------|------|------|
| **HashMap (chosen)** | Extensible, clean lookup, arbitrary symbols | Slightly more complex config |
| **Fixed fields (btc, eth)** | Simpler code, explicit | Hard to add new symbols |
| **Global + multiplier** | Compact config | Implicit values harder to understand |

### 8.2 Design Decisions

1. **Lowercase symbol keys** — Consistent with TOML conventions, case-insensitive lookup
2. **No fallback** — Each symbol must be explicitly configured (fail-fast)
3. **HashMap over fixed struct** — Supports adding SOL, DOGE, etc. without code changes

## 9. Success Criteria

1. ✅ Config file supports per-symbol IV and ATM thresholds
2. ✅ AbsoluteIvHandler correctly routes symbol data to symbol-specific thresholds
3. ✅ BTC and ETH can have different alert sensitivities
4. ✅ Adding a new symbol requires only config changes
5. ✅ Missing symbol config logs warning and skips gracefully

## 10. Open Questions

None — design is complete and approved.
