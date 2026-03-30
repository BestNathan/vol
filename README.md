# vol-monitor

A Rust-based volatility monitoring service for Deribit options.

## Features

- **Multi-tenor monitoring**: Short (≤7d), Medium (20-40d), Long (>80d)
- **4 alert types**:
  - Absolute IV threshold
  - Rate of change (1h/4h/24h)
  - Term structure anomaly
  - Skew divergence
- **Plugin architecture**: Extensible data sources, alert handlers, and notifications
- **Feishu/Lark integration**: Send alerts to your team chat
- **Event-driven design**: Built with tokio for async operation

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Config Layer                            │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│                   DataSource Layer (trait)                   │
│  DeribitDataSource | BinanceDataSource (future) | CSV       │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│                    EventBus (tokio broadcast)                │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│                  Alert Handler Layer (trait)                 │
│  AbsoluteIv | RateChange | TermStructure | Skew             │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│                Notification Layer (trait)                    │
│  Feishu | Stdout | Slack (future)                           │
└─────────────────────────────────────────────────────────────┘
```

## Project Structure

This is a Cargo workspace (monorepo) with the following packages:

| Package | Description |
|---------|-------------|
| `vol-core` | Core traits and data models |
| `vol-eventbus` | Event bus for publish/subscribe |
| `vol-config` | Configuration management |
| `vol-datasource` | Data source implementations (Deribit, CSV) |
| `vol-alert` | Alert handler implementations |
| `vol-notification` | Notification handlers (Feishu, Stdout) |
| `vol-monitor` | Main binary |

## Quick Start

```bash
# Build
cargo build --release

# Copy config
cp config.toml.example config.toml

# Edit config with your settings
# - Set your Feishu webhook URL
# - Adjust alert thresholds

# Run
cargo run --release
```

## Configuration

See `config.toml.example` for all available options.

### Tenor Definitions

- **Short**: DTE ≤ 7 days
- **Medium**: 20 < DTE < 40 days
- **Long**: DTE > 80 days

### Alert Thresholds

Default thresholds (adjust in config.toml):

| Alert Type | Short | Medium | Long |
|------------|-------|--------|------|
| Absolute IV | 80% | 70% | 60% |
| Rate Change (1h) | 5% | 5% | 5% |
| Rate Change (4h) | 10% | 10% | 10% |
| Rate Change (24h) | 20% | 20% | 20% |

## Feishu Integration

1. Create a bot in your Feishu/Lark group
2. Get the incoming webhook URL
3. Update `config.toml`:
   ```toml
   [notifications.feishu]
   webhook_url = "https://open.feishu.cn/open-apis/bot/v2/hook/YOUR_HOOK_CODE"
   ```

## License

MIT
