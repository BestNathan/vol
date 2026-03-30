# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
# Build all workspace members
cargo build --release

# Check without building
cargo check --workspace

# Run the monitor
HTTPS_PROXY=http://<proxy>:<port> ./target/release/vol-monitor

# Run with logging
RUST_LOG=info HTTPS_PROXY=http://<proxy>:<port> ./target/release/vol-monitor

# Run tests (if any exist)
cargo test --workspace
```

## Architecture Overview

### Workspace Structure

This is a Cargo workspace with 7 crates:

| Crate | Purpose |
|-------|---------|
| `vol-core` | Shared traits (`DataSource`, `AlertHandler`, `NotificationHandler`) and data models (`VolatilityData`, `Alert`) |
| `vol-config` | TOML-based configuration loading |
| `vol-eventbus` | Tokio broadcast channels for pub/sub messaging |
| `vol-datasource` | Data providers (Deribit WebSocket, CSV fallback) - uses vol-deribit client |
| `vol-deribit` | Deribit client + data models: WebSocket connection, proxy support, instrument parsing, market data, JSON-RPC messages, subscriptions |
| `vol-alert` | Alert evaluation logic (4 types) |
| `vol-notification` | Alert delivery (stdout, Feishu webhook) |
| `vol-monitor` | Main binary - wires everything together |

### Data Flow

```
Deribit WebSocket → DeribitDataSource → mpsc channel → main event loop
                                               ↓
                                    AlertManager (cooldown check)
                                               ↓
                                    NotificationHandler → stdout/Feishu
```

### Key Design Patterns

- **Trait-based plugin architecture**: All extension points use traits from `vol-core`
- **Async-first**: All crates use tokio; no blocking I/O
- **Channel-based communication**: `tokio::mpsc` for data streaming between components
- **State persistence**: Alert state saved to `~/.deribit-vol-monitor/state.json` on shutdown

### Alert Types (in `vol-alert`)

1. **AbsoluteIvHandler** - Triggers when IV exceeds tenor-specific thresholds
2. **RateChangeHandler** - Detects rapid IV changes (1h/4h/24h windows)
3. **TermStructureHandler** - Monitors short-long IV spread anomalies
4. **SkewHandler** - Detects call-put skew divergence

### Tenor Classification (from config)

- **Short**: DTE ≤ 7 days
- **Medium**: 20 < DTE < 40 days
- **Long**: DTE ≥ 80 days

## Configuration

Main config file: `config.toml`

Key sections:
- `[data_sources.deribit]` - WebSocket URL, symbols (BTC/ETH), poll interval
- `[tenors]` - DTE boundaries for tenor classification
- `[alerts]` - Cooldown period and per-type thresholds
- `[notifications]` - Notification channels (stdout, feishu)

### Feishu (Lark) Configuration

Feishu notification uses OAuth 2.0 app access token:

```toml
[notifications.feishu]
app_id = "cli_xxx"           # Get from https://open.feishu.cn/app
app_secret = "xxx"           # App secret from Feishu console
receive_id = "oc_xxx"        # Chat ID or user ID to receive messages
message_template = "🚨 {tenor} {alert_type}: {symbol} IV={value}"
```

**Getting Feishu credentials:**
1. Create a bot app at https://open.feishu.cn/app
2. Get `app_id` and `app_secret` from app credentials
3. Add bot to a group chat, get the `chat_id` as `receive_id`
4. Enable "Bot" and "Send messages" permissions

**API Reference:**
- Access Token: `POST /open-apis/auth/v3/app_access_token/internal`
- Send Message: `POST /open-apis/im/v1/messages`
## Deribit Integration

### vol-deribit Package

The `vol-deribit` crate (`crates/vol-deribit/src/`) contains all Deribit-specific types and client logic:

| Module | Purpose |
|--------|---------|
| `client.rs` | `DeribitClient` - WebSocket connection, HTTP proxy support, auto-reconnect, message parsing |
| `feishu.rs` | `FeishuClient` - Feishu/Lark API client, OAuth 2.0 app access token, message sending |
| `instrument.rs` | `DeribitInstrument`, `OptionType`, `InstrumentType`; `parse_instrument_name()`, `calculate_dte()` |
| `market_data.rs` | `OptionMarkPrice`, `DeribitTicker`, `IndexMarkPrice`, `OrderBook`, `Trade` with `to_volatility_data()` converters |
| `message.rs` | JSON-RPC 2.0 types: `SubscriptionNotification`, `JsonRpcRequest`, `JsonRpcResponse` |
| `subscription.rs` | Channel builders (`markprice_options()`, `ticker_base()`, etc.) and presets |

### Deribit DataSource

The Deribit datasource (`crates/vol-datasource/src/deribit.rs`) uses `DeribitClient`:

1. Wraps `DeribitClient` for low-level WebSocket communication
2. Implements `DataSource` trait from `vol-core`
3. Subscribes to `markprice.options.btc_usd` and `markprice.options.eth_usd` channels
4. Parses incoming ticker data: `instrument_name`, `iv`, `timestamp`, `mark_price`
5. Extracts DTE from instrument name format: `BTC-29MAR24-70000-C`
6. Streams `VolatilityData` via mpsc channel

### Architecture: vol-deribit vs vol-datasource

- **vol-deribit**: Deribit-specific client and data models (WebSocket, JSON-RPC, parsing)
- **vol-datasource**: DataSource trait implementation that uses vol-deribit client

### Proxy Support

Set `HTTPS_PROXY` environment variable for HTTP proxy tunneling:
```bash
HTTPS_PROXY=http://192.168.2.98:8890 ./target/release/vol-monitor
```

## Deribit API Documentation

Local copy of Deribit API documentation is available at `docs/deribit/`:

```
docs/deribit/
├── api-reference/      # API 方法参考文档
├── articles/           # 指南和最佳实践
├── fix-api/           # FIX API 文档
├── specifications/     # OpenAPI/AsyncAPI 规范
├── subscriptions/      # WebSocket 订阅频道
├── index.md           # 文档首页
└── llms.txt          # 文档索引
```

**常用文档路径：**
- 市场数据：`docs/deribit/api-reference/market-data/`
- 交易操作：`docs/deribit/api-reference/trading/`
- WebSocket 订阅：`docs/deribit/subscriptions/`
- 快速入门：`docs/deribit/articles/deribit-quickstart.md`
- 错误处理：`docs/deribit/articles/errors.md`

## Common Modifications

### Adding a new alert type
1. Create new handler struct in `crates/vol-alert/src/your_alert.rs`
2. Implement `AlertHandler` trait from `vol-core`
3. Register in `crates/vol-monitor/src/main.rs`

### Adding a new data source
1. Implement `DataSource` trait from `vol-core`
2. Add to `crates/vol-datasource/src/registry.rs`

### Adding a notification channel
1. Implement `NotificationHandler` trait from `vol-core`
2. Register in `crates/vol-monitor/src/main.rs`
