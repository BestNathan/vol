# Deribit Integration

## Overview

This project integrates with Deribit API for real-time options market data and portfolio management.

## Architecture

### vol-deribit vs vol-datasource

| Crate | Purpose |
|-------|---------|
| **vol-deribit** | Deribit-specific client and data models (WebSocket, JSON-RPC, parsing) |
| **vol-datasource** | DataSource trait implementation that uses vol-deribit client |

### vol-deribit Package

**Path:** `crates/vol-deribit/src/`

| Module | Purpose |
|--------|---------|
| `client.rs` | `DeribitClient` - WebSocket connection, HTTP proxy support, auto-reconnect, message parsing |
| `feishu.rs` | `FeishuClient` - Feishu/Lark API client, OAuth 2.0 app access token, message sending |
| `instrument.rs` | `DeribitInstrument`, `OptionType`, `InstrumentType`; `parse_instrument_name()`, `calculate_dte()` |
| `market_data.rs` | `OptionMarkPrice`, `DeribitTicker`, `IndexMarkPrice`, `OrderBook`, `Trade` with `to_volatility_data()` converters |
| `message.rs` | JSON-RPC 2.0 types: `SubscriptionNotification`, `JsonRpcRequest`, `JsonRpcResponse` |
| `subscription.rs` | Channel builders (`markprice_options()`, `ticker_base()`, etc.) and presets |

### Volatility DataSource

**Path:** `crates/vol-datasource/src/volatility.rs`

The Volatility datasource uses `DeribitClient`:

1. Wraps `DeribitClient` for low-level WebSocket communication
2. Implements `DataSource` trait from `vol-core`
3. Subscribes to `markprice.options.btc_usd` and `markprice.options.eth_usd` channels
4. Parses incoming ticker data: `instrument_name`, `iv`, `timestamp`, `mark_price`
5. Extracts DTE from instrument name format: `BTC-29MAR24-70000-C`
6. Streams `VolatilityData` via mpsc channel

## Configuration

### Client Configuration

```toml
[clients.deribit]
ws_url = "wss://www.deribit.com/ws/api/v2"
# Credentials from environment variables:
# - DERIBIT_CLIENT_ID
# - DERIBIT_CLIENT_SECRET
```

### Environment Variables

| Variable | Description |
|----------|-------------|
| `DERIBIT_CLIENT_ID` | Deribit API client ID |
| `DERIBIT_CLIENT_SECRET` | Deribit API client secret |
| `DERIBIT_WS_URL` | WebSocket URL (optional, default: production) |

## Proxy Support

Set `HTTPS_PROXY` environment variable for HTTP proxy tunneling:

```bash
# Local development
export HTTPS_PROXY="http://192.168.2.98:8890"
./target/release/vol-monitor --config config.toml

# Kubernetes (in deployment.yaml)
env:
- name: HTTPS_PROXY
  value: "http://192.168.2.98:8890"
```

## Subscription Channels

The system subscribes to the following Deribit channels:

| Channel | Purpose |
|---------|---------|
| `markprice.options.btc_usd` | BTC option mark prices |
| `markprice.options.eth_usd` | ETH option mark prices |
| `ticker.BTC_USD` | BTC index price |
| `ticker.ETH_USD` | ETH index price |

## Data Flow

```
Deribit WebSocket
       │
       ▼
┌─────────────────┐
│ DeribitClient   │
│ (vol-deribit)   │
└────────┬────────┘
         │
         │ ChannelData
         ▼
┌─────────────────┐
│ VolatilityData  │
│ source          │
│ (vol-datasource)│
└────────┬────────┘
         │
         │ VolatilityData
         ▼
┌─────────────────┐
│ MonitoringEngine│
└─────────────────┘
```

## API Documentation

Local copy of Deribit API documentation:

```
docs/deribit/
├── api-reference/      # API reference
├── articles/           # Guides and best practices
├── fix-api/           # FIX API documentation
├── specifications/     # OpenAPI/AsyncAPI specs
├── subscriptions/      # WebSocket subscription channels
├── index.md           # Documentation home
└── llms.txt          # Documentation index
```

**Common paths:**
- Market Data: `docs/deribit/api-reference/market-data/`
- Trading: `docs/deribit/api-reference/trading/`
- WebSocket: `docs/deribit/subscriptions/`
- Quick Start: `docs/deribit/articles/deribit-quickstart.md`
- Errors: `docs/deribit/articles/errors.md`
