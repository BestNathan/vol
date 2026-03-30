# Per-Symbol IV Threshold Configuration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 IV 阈值配置从全局单一配置改为按标的（BTC、ETH 等）独立配置，支持每个标的有自己的 IV 阈值和 ATM 阈值。

**Architecture:** 在 `AbsoluteIvConfig` 中使用 `HashMap<String, SymbolIvConfig>` 来存储每个标的的配置，`AbsoluteIvHandler` 在评估告警时根据 instrument name 提取 symbol 并查找对应的配置。

**Tech Stack:** Rust, serde (TOML 解析), HashMap, vol-core, vol-config, vol-alert

---

## File Structure

| 文件 | 操作 | 说明 |
|------|------|------|
| `crates/vol-config/src/lib.rs` | Modify | 添加 `SymbolIvConfig` struct，修改 `AbsoluteIvConfig` 使用 HashMap |
| `crates/vol-alert/src/absolute_iv.rs` | Modify | 修改 `AbsoluteIvHandler` 根据 symbol 查找配置 |
| `crates/vol-monitor/src/main.rs` | Modify | 更新 `create_default_config()` 使用新结构 |
| `config.toml` | Modify | 更新配置文件使用新的 per-symbol 格式 |

---

### Task 1: 修改 vol-config 添加 SymbolIvConfig

**Files:**
- Modify: `crates/vol-config/src/lib.rs`

- [ ] **Step 1: 添加 SymbolIvConfig struct**

在 `AbsoluteIvConfig` 定义之前添加：

```rust
/// Per-symbol IV and ATM threshold configuration
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

- [ ] **Step 2: 修改 AbsoluteIvConfig 使用 HashMap**

替换现有的 `AbsoluteIvConfig`：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbsoluteIvConfig {
    /// Per-symbol configuration keyed by lowercase symbol name (e.g., "btc", "eth")
    pub symbols: std::collections::HashMap<String, SymbolIvConfig>,
}

impl AbsoluteIvConfig {
    /// Get symbol-specific config (case-insensitive)
    pub fn get_symbol_config(&self, symbol: &str) -> Option<&SymbolIvConfig> {
        self.symbols.get(&symbol.to_lowercase())
    }
}
```

- [ ] **Step 3: 移除不再需要的全局字段和默认函数**

删除以下字段（因为它们现在在 `SymbolIvConfig` 中）：
- `short_threshold`, `medium_threshold`, `long_threshold`
- `short_atm_threshold`, `medium_atm_threshold`, `long_atm_threshold`
- `default_short_atm()`, `default_medium_atm()`, `default_long_atm()`

- [ ] **Step 4: 编译检查**

```bash
cargo check -p vol-config
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vol-config/src/lib.rs
git commit -m "refactor: add SymbolIvConfig for per-symbol IV thresholds"
```

---

### Task 2: 修改 vol-alert AbsoluteIvHandler 支持按标的查找配置

**Files:**
- Modify: `crates/vol-alert/src/absolute_iv.rs`

- [ ] **Step 1: 更新导入**

```rust
use vol_core::{AlertHandler, Alert, AlertType, AlertAction, VolatilityData, Tenor, Result};
use vol_config::{AbsoluteIvConfig, SymbolIvConfig};
```

- [ ] **Step 2: 添加 extract_symbol 辅助函数**

在 `impl AbsoluteIvHandler` 块之前添加：

```rust
/// Extract underlying symbol from instrument name
/// e.g., "BTC-29MAR24-70000-C" -> "BTC"
fn extract_symbol(instrument_name: &str) -> Option<&str> {
    instrument_name.split('-').next()
}
```

- [ ] **Step 3: 添加 get_symbol_config 方法**

在 `impl AbsoluteIvHandler` 块中添加：

```rust
impl AbsoluteIvHandler {
    pub fn new(config: AbsoluteIvConfig) -> Self {
        Self { config }
    }

    fn get_symbol_config(&self, symbol: &str) -> Option<&SymbolIvConfig> {
        self.config.get_symbol_config(symbol)
    }

    // ... existing get_threshold and get_atm_threshold will be removed
}
```

- [ ] **Step 4: 重写 evaluate 方法**

替换现有的 `evaluate` 方法：

```rust
fn evaluate(&self, data: &VolatilityData) -> Option<Alert> {
    // Extract symbol from instrument name
    let symbol_name = extract_symbol(&data.symbol)?;

    // Get symbol-specific config
    let symbol_config = self.get_symbol_config(symbol_name)?;

    let tenor = data.tenor();

    // Get IV threshold for this symbol and tenor
    let iv_threshold = match tenor {
        Tenor::Short => symbol_config.short_threshold,
        Tenor::Medium => symbol_config.medium_threshold,
        Tenor::Long => symbol_config.long_threshold,
    };

    // Get ATM threshold for this symbol and tenor
    let atm_threshold = match tenor {
        Tenor::Short => symbol_config.short_atm_threshold,
        Tenor::Medium => symbol_config.medium_atm_threshold,
        Tenor::Long => symbol_config.long_atm_threshold,
    };

    // ATM filter - skip if not ATM for this symbol's threshold
    if !data.is_atm(atm_threshold) {
        return None;
    }

    // IV threshold check
    if data.iv >= iv_threshold {
        let moneyness = data.moneyness();
        Some(Alert::new(
            AlertType::AbsoluteIv { threshold: iv_threshold },
            tenor,
            data.symbol.clone(),
            data.iv,
            format!(
                "{} {} IV {:.1}% (symbol: {}, moneyness: {:.2}%, ATM: {:.1}%) >= threshold {:.1}%",
                data.symbol, tenor,
                data.iv * 100.0, symbol_name, moneyness * 100.0, atm_threshold * 100.0, iv_threshold * 100.0
            ),
            data.timestamp,
            data.source.clone(),
        ))
    } else {
        None
    }
}
```

- [ ] **Step 5: 移除旧的 get_threshold 和 get_atm_threshold 方法**

删除这两个方法（因为它们现在在 evaluate 中直接处理）。

- [ ] **Step 6: 编译检查**

```bash
cargo check -p vol-alert
```

Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/vol-alert/src/absolute_iv.rs
git commit -m "feat: support per-symbol IV threshold lookup"
```

---

### Task 3: 更新 vol-monitor 的默认配置

**Files:**
- Modify: `crates/vol-monitor/src/main.rs`

- [ ] **Step 1: 添加 HashMap 导入**

在 `create_default_config` 函数附近添加：

```rust
use std::collections::HashMap;
```

- [ ] **Step 2: 重写 create_default_config 中的 AbsoluteIvConfig**

替换现有的 `absolute_iv` 配置：

```rust
let mut symbols = HashMap::new();

// BTC config - BTC typically has lower IV than ETH
symbols.insert("btc".to_string(), vol_config::SymbolIvConfig {
    short_threshold: 0.80,
    medium_threshold: 0.70,
    long_threshold: 0.60,
    short_atm_threshold: 0.05,
    medium_atm_threshold: 0.10,
    long_atm_threshold: 0.15,
});

// ETH config - ETH typically has higher IV, allow wider ATM ranges
symbols.insert("eth".to_string(), vol_config::SymbolIvConfig {
    short_threshold: 0.90,
    medium_threshold: 0.80,
    long_threshold: 0.70,
    short_atm_threshold: 0.08,
    medium_atm_threshold: 0.12,
    long_atm_threshold: 0.18,
});

// ...
absolute_iv: vol_config::AbsoluteIvConfig { symbols },
```

- [ ] **Step 3: 编译检查**

```bash
cargo check -p vol-monitor
```

Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/vol-monitor/src/main.rs
git commit -m "chore: update default config for per-symbol IV thresholds"
```

---

### Task 4: 更新 config.toml 配置文件

**Files:**
- Modify: `config.toml`

- [ ] **Step 1: 替换 alerts.absolute_iv 部分**

替换现有的：

```toml
[alerts.absolute_iv]
short_threshold = 0.80
medium_threshold = 0.70
long_threshold = 0.60
short_atm_threshold = 0.05
medium_atm_threshold = 0.10
long_atm_threshold = 0.15
```

为：

```toml
# BTC IV thresholds - typically lower than ETH
[alerts.absolute_iv.btc]
short_threshold = 0.80
medium_threshold = 0.70
long_threshold = 0.60
short_atm_threshold = 0.05    # ±5% for short-term
medium_atm_threshold = 0.10   # ±10% for medium-term
long_atm_threshold = 0.15     # ±15% for long-term

# ETH IV thresholds - typically higher than BTC, wider ATM ranges
[alerts.absolute_iv.eth]
short_threshold = 0.90
medium_threshold = 0.80
long_threshold = 0.70
short_atm_threshold = 0.08    # ±8% for short-term
medium_atm_threshold = 0.12   # ±12% for medium-term
long_atm_threshold = 0.18     # ±18% for long-term
```

- [ ] **Step 2: 验证配置解析**

```bash
cargo run --bin vol-monitor -- --help 2>&1 | head -5
```

Expected: No config parse errors

- [ ] **Step 3: Commit**

```bash
git add config.toml
git commit -m "config: update to per-symbol IV threshold format"
```

---

### Task 5: 添加单元测试

**Files:**
- Create: `crates/vol-config/src/lib.rs` (tests module)
- Create: `crates/vol-alert/src/absolute_iv.rs` (tests module)

- [ ] **Step 1: 在 vol-config 中添加配置解析测试**

在 `crates/vol-config/src/lib.rs` 末尾添加：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_iv_config_parsing() {
        let toml_str = r#"
            [alerts.absolute_iv.btc]
            short_threshold = 0.80
            medium_threshold = 0.70
            long_threshold = 0.60
            short_atm_threshold = 0.05
            medium_atm_threshold = 0.10
            long_atm_threshold = 0.15
        "#;

        let config: Config = toml::from_str(toml_str).unwrap();
        let btc_config = config.alerts.absolute_iv.get_symbol_config("btc").unwrap();

        assert_eq!(btc_config.short_threshold, 0.80);
        assert_eq!(btc_config.short_atm_threshold, 0.05);
    }

    #[test]
    fn test_case_insensitive_symbol_lookup() {
        let mut symbols = std::collections::HashMap::new();
        symbols.insert("btc".to_string(), SymbolIvConfig {
            short_threshold: 0.80,
            medium_threshold: 0.70,
            long_threshold: 0.60,
            short_atm_threshold: 0.05,
            medium_atm_threshold: 0.10,
            long_atm_threshold: 0.15,
        });

        let config = AbsoluteIvConfig { symbols };

        assert!(config.get_symbol_config("BTC").is_some());
        assert!(config.get_symbol_config("btc").is_some());
        assert!(config.get_symbol_config("Btc").is_some());
    }
}
```

- [ ] **Step 2: 在 vol-alert 中添加 symbol 提取测试**

在 `crates/vol-alert/src/absolute_iv.rs` 末尾添加：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use vol_config::SymbolIvConfig;
    use std::collections::HashMap;

    #[test]
    fn test_extract_symbol() {
        assert_eq!(extract_symbol("BTC-29MAR24-70000-C"), Some("BTC"));
        assert_eq!(extract_symbol("ETH-29MAR24-3500-P"), Some("ETH"));
        assert_eq!(extract_symbol("INVALID"), Some("INVALID"));
    }

    #[test]
    fn test_evaluate_with_symbol_specific_config() {
        let mut symbols = HashMap::new();

        // BTC config - lower thresholds
        symbols.insert("btc".to_string(), SymbolIvConfig {
            short_threshold: 0.80,
            medium_threshold: 0.70,
            long_threshold: 0.60,
            short_atm_threshold: 0.05,
            medium_atm_threshold: 0.10,
            long_atm_threshold: 0.15,
        });

        // ETH config - higher thresholds
        symbols.insert("eth".to_string(), SymbolIvConfig {
            short_threshold: 0.90,
            medium_threshold: 0.80,
            long_threshold: 0.70,
            short_atm_threshold: 0.08,
            medium_atm_threshold: 0.12,
            long_atm_threshold: 0.18,
        });

        let handler = AbsoluteIvHandler::new(AbsoluteIvConfig { symbols });

        // Create test data - BTC at 85% IV (should trigger for BTC)
        let btc_data = VolatilityData {
            symbol: "BTC-6JAN25-95000-C".to_string(),
            dte: 5,
            iv: 0.85,
            timestamp: 1234567890,
            source: "deribit".to_string(),
            strike: 95000.0,
            option_type: vol_core::OptionType::Call,
            index_price: 95000.0,
            delta: None,
            extra: std::collections::HashMap::new(),
        };

        // BTC 85% IV should trigger (threshold 80%)
        let alert = handler.evaluate(&btc_data);
        assert!(alert.is_some());

        // Create ETH data at 85% IV (should NOT trigger for ETH)
        let eth_data = VolatilityData {
            symbol: "ETH-6JAN25-3800-C".to_string(),
            dte: 5,
            iv: 0.85,
            timestamp: 1234567890,
            source: "deribit".to_string(),
            strike: 3800.0,
            option_type: vol_core::OptionType::Call,
            index_price: 3800.0,
            delta: None,
            extra: std::collections::HashMap::new(),
        };

        // ETH 85% IV should NOT trigger (threshold 90%)
        let alert = handler.evaluate(&eth_data);
        assert!(alert.is_none());
    }

    #[test]
    fn test_evaluate_missing_symbol_config() {
        let mut symbols = HashMap::new();
        symbols.insert("btc".to_string(), SymbolIvConfig {
            short_threshold: 0.80,
            medium_threshold: 0.70,
            long_threshold: 0.60,
            short_atm_threshold: 0.05,
            medium_atm_threshold: 0.10,
            long_atm_threshold: 0.15,
        });

        let handler = AbsoluteIvHandler::new(AbsoluteIvConfig { symbols });

        // SOL data - no config for SOL
        let sol_data = VolatilityData {
            symbol: "SOL-6JAN25-100-C".to_string(),
            dte: 5,
            iv: 0.95,
            timestamp: 1234567890,
            source: "deribit".to_string(),
            strike: 100.0,
            option_type: vol_core::OptionType::Call,
            index_price: 100.0,
            delta: None,
            extra: std::collections::HashMap::new(),
        };

        // Should return None for missing symbol config
        let alert = handler.evaluate(&sol_data);
        assert!(alert.is_none());
    }
}
```

- [ ] **Step 3: 运行测试**

```bash
cargo test -p vol-config -p vol-alert -- --nocapture
```

Expected: All tests PASS

- [ ] **Step 4: Commit**

```bash
git add crates/vol-config/src/lib.rs crates/vol-alert/src/absolute_iv.rs
git commit -m "test: add unit tests for per-symbol IV configuration"
```

---

### Task 6: 集成测试和验证

**Files:**
- N/A (runtime verification)

- [ ] **Step 1: 完整构建**

```bash
cargo build --release
```

Expected: PASS

- [ ] **Step 2: 运行 vol-monitor 验证配置加载**

```bash
./target/release/vol-monitor 2>&1 | head -30
```

Expected: Sees "Monitoring started" and alert threshold info without config errors

- [ ] **Step 3: 验证日志显示 symbol 信息**

观察日志输出，确认告警消息中包含 symbol 名称：

```
[1] BTC-6JAN25-95000-C short IV 85.2% (symbol: BTC, moneyness: 0.53%, ATM: 5.0%) >= threshold 80.0%
```

- [ ] **Step 4: Commit (如果有日志验证相关的代码修改)**

```bash
git add .
git commit -m "chore: verify per-symbol IV thresholds in production"
```

---
