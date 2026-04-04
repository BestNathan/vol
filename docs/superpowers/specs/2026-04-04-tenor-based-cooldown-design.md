# Tenor-Based Alert Cooldown Configuration Design

**日期:** 2026-04-04
**作者:** Claude Code
**状态:** 待实施

## 概述

为 vol-monitor 的告警系统设计基于期限 (tenor) 的可配置 cooldown 机制，允许为短期、中期、长期告警设置不同的冷却时间。

## 背景

当前系统使用单一全局 cooldown 值 (`alert_cooldown_secs = 300` / 5 分钟) 应用于所有告警。实际业务需求：

- **短期期权** (DTE ≤ 7 天): 市场波动快，需要较短 cooldown (如 10 分钟) 避免告警风暴
- **中期期权** (20 < DTE < 40 天): 适中 cooldown (如 1 小时)
- **长期期权** (DTE ≥ 80 天): 变化缓慢，需要较长 cooldown (如 4 小时) 避免重复通知

## 设计目标

1. **可配置**: 通过 config.toml 配置各 tenor 的 cooldown 时间
2. **向后兼容**: 保留全局 `alert_cooldown_secs` 作为 fallback
3. **tenor-specific**: 告警根据其 tenor 使用对应的 cooldown 值
4. **统一适用**: 所有基于 tenor 的告警类型 (absolute_iv, rate_change, term_structure, skew) 使用相同的 cooldown 规则

## 配置结构

### TOML 格式

```toml
[engine]
alert_cooldown_secs = 300  # 全局默认 fallback (5 分钟)

[engine.tenor_cooldowns]
short_secs = 600    # 10 分钟 - 短期告警 cooldown
medium_secs = 3600  # 1 小时 - 中期告警 cooldown  
long_secs = 14400   # 4 小时 - 长期告警 cooldown
```

### 默认行为

- 如果未配置 `tenor_cooldowns`，所有 tenor 使用 `alert_cooldown_secs`
- 如果仅配置部分 tenor，未配置的 tenor 使用 `alert_cooldown_secs`

## 代码架构

### 1. vol-config (`crates/vol-config/src/lib.rs`)

**新增结构体:**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TenorCooldownsConfig {
    #[serde(default)]
    pub short_secs: Option<u64>,
    #[serde(default)]
    pub medium_secs: Option<u64>,
    #[serde(default)]
    pub long_secs: Option<u64>,
}
```

**修改 `EngineConfigFile`:**

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
    #[serde(default)]
    pub tenor_cooldowns: TenorCooldownsConfig,
}
```

**新增方法:**

```rust
impl EngineConfigFile {
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

### 2. vol-alert (`crates/vol-alert/src/manager.rs`)

**修改 `AlertManager` 结构:**

```rust
pub struct AlertManager {
    config: EngineConfigFile,
    last_alert_time: Arc<Mutex<HashMap<String, u64>>>,
}
```

**修改构造函数:**

```rust
impl AlertManager {
    pub fn new(config: EngineConfigFile) -> Self {
        Self {
            config,
            last_alert_time: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn can_send(&self, alert: &Alert) -> bool {
        let key = format!("{}:{}:{}", alert.alert_type, alert.tenor, alert.symbol);
        let cooldown_secs = self.config.get_cooldown_for_tenor(alert.tenor);
        let cooldown_ms = cooldown_secs * 1000;

        let mut last_times = self.last_alert_time.lock().unwrap();
        let now = alert.timestamp;

        let last_time = last_times.entry(key).or_insert(0);

        if now - *last_time >= cooldown_ms {
            *last_time = now;
            true
        } else {
            false
        }
    }
}
```

### 3. vol-monitor (`crates/vol-monitor/src/main.rs`)

**修改 AlertManager 初始化:**

```rust
fn main() -> Result<()> {
    let config = Config::load("config.toml")?;
    
    // 传递完整 engine config 给 AlertManager
    let alert_manager = AlertManager::new(config.engine.clone());
    
    // ... 其余初始化代码
}
```

## 修改的文件清单

| 文件 | 修改内容 |
|------|----------|
| `crates/vol-config/src/lib.rs` | 新增 `TenorCooldownsConfig` 结构体，修改 `EngineConfigFile`，新增 `get_cooldown_for_tenor()` 方法 |
| `crates/vol-alert/src/manager.rs` | 修改 `AlertManager` 持有 `EngineConfigFile`，更新 `can_send()` 使用 tenor-specific cooldown |
| `crates/vol-monitor/src/main.rs` | 修改 `AlertManager::new()` 调用传入完整 config |
| `config.toml` | 示例配置添加 `[engine.tenor_cooldowns]` 段落 |

## 告警 Tenor 映射

告警的 tenor 由产生告警的数据决定：

- **AbsoluteIvHandler**: 根据期权的 DTE 分类到 short/medium/long
- **RateChangeHandler**: 同上
- **TermStructureHandler**: 涉及 short 和 long tenor 的 spread，两个 tenor 分别计算 cooldown
- **SkewHandler**: 根据期权 tenor 分类

Cooldown key 格式保持不变：`alert_type:tenor:symbol`

示例：
- `absolute_iv:short:BTC` - BTC 短期 IV 告警
- `absolute_iv:long:ETH` - ETH 长期 IV 告警
- `rate_change:medium:BTC` - BTC 中期 IV 变化告警

## 向后兼容性

- 现有配置不添加 `tenor_cooldowns` 时，行为与之前完全一致
- 新增字段均为 `Option<u64>` + `#[serde(default)]`，不影响现有配置解析
- 迁移路径：用户可逐步添加 tenor-specific 配置，无需一次性迁移

## 测试验证

```bash
# 1. 配置测试
cat > /tmp/test_cooldown.toml <<EOF
[engine]
alert_cooldown_secs = 300

[engine.tenor_cooldowns]
short_secs = 600
medium_secs = 3600
long_secs = 14400

[tenors]
short_max_dte = 7
medium_min_dte = 20
medium_max_dte = 40
long_min_dte = 80

[[datasources]]
id = "test"
provider = "deribit"
ws_url = "wss://www.deribit.com/ws/api/v2"
symbols = ["BTC"]
enabled = true

[[rules]]
id = "test"
type = "absolute-iv"
symbol = "BTC"
short_threshold = 0.5
medium_threshold = 0.5
long_threshold = 0.5
enabled = true
EOF

# 2. 配置解析测试
cargo test -p vol-config test_tenor_cooldowns_config

# 3. 单元测试
cargo test -p vol-alert test_tenor_based_cooldown

# 4. 集成测试 (可选)
# 发送模拟告警验证 cooldown 行为
```

## 回滚方案

如需回滚，移除 config.toml 中的 `[engine.tenor_cooldowns]` 段落即可，系统自动 fallback 到全局 `alert_cooldown_secs`。

## 参考资料

- 现有 cooldown 实现：`crates/vol-alert/src/manager.rs`
- Tenor 分类逻辑：`crates/vol-config/src/lib.rs::TenorConfig::classify()`
