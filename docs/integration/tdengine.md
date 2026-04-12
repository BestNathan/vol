# TDengine Integration Guide

## Overview

The vol-llm-tool crate integrates with TDengine for historical data queries via HTTP REST API.

## Configuration

### TDengine Server

Default configuration connects to:
- Host: `192.168.2.106`
- Port: `6041` (HTTP REST API)
- User: `root`
- Password: `taosdata`
- Database: `deribit`

### Environment Variables

Override defaults via environment:

```bash
export TDENGINE_HOST="192.168.2.106"
export TDENGINE_PORT="6041"
export TDENGINE_USER="root"
export TDENGINE_PASSWORD="your-password"
export TDENGINE_DATABASE="deribit"
```

### Configuration in Code

```rust
use vol_llm_tool::{TdengineConfig, TdengineClient};

// Use defaults
let config = TdengineConfig::default();

// Or customize
let config = TdengineConfig {
    host: "192.168.2.106".to_string(),
    port: 6041,
    user: "admin".to_string(),
    password: "custom".to_string(),
    database: "deribit".to_string(),
};

let client = TdengineClient::new(config);
```

## Database Schema

The `deribit` database contains the following super tables:

| Stable Table | Purpose | Columns |
|--------------|---------|---------|
| `deribit_volatility_index` | Real-time volatility index | `_ts`, `volatility`, `index_name` (TAG) |
| `deribit_rv` | Realized volatility | `_ts`, `rv`, `index_name` (TAG) |
| `deribit_index_price` | Index prices | `_ts`, `price`, `index_name` (TAG) |
| `deribit_options` | Options chain data (IV, mark price) | `_ts`, `mark_price`, `iv`, `index_name` (TAG), `expiry_date` (TAG), `strike_price` (TAG), `type` (TAG), `instrument_name` (TAG) |

### Tool to Table Mapping

| Tool | TDengine Table | Purpose |
|------|----------------|---------|
| `alert_history` | `deribit_volatility_index` | Query volatility history |
| `iv_curve` | `deribit_options` | Query IV curves for options |
| `market_data` | `deribit_index_price` | Query current market prices |
| `rule_info` | `deribit_rv` | Query realized volatility data |

## Available Tools

### AlertHistoryTool

Query historical volatility index data:

```rust
let tool = AlertHistoryTool::new(Some(config));
let result = tool.execute(
    &json!({
        "symbol": "btc_usd",
        "limit": 10,
        "hours": 24
    }),
    &context
).await?;
```

### IvCurveTool

Query implied volatility data for options:

```rust
let tool = IvCurveTool::new(Some(config));
let result = tool.execute(
    &json!({
        "instrument": "BTC-29DEC23-3000-C",
    }),
    &context
).await?;
```

### MarketDataTool

Query current market price data:

```rust
let tool = MarketDataTool::new(Some(config));
let result = tool.execute(
    &json!({
        "instrument": "btc_usd",
        "data_type": "price"
    }),
    &context
).await?;
```

### RuleInfoTool

Query realized volatility data:

```rust
let tool = RuleInfoTool::new(Some(config));
let result = tool.execute(
    &json!({
        "index_name": "btc_usd",
        "list_all": false
    }),
    &context
).await?;
```

## Testing

Run integration tests:

```bash
# Test TDengine connection
cargo test --test tdengine_integration -- --nocapture

# Test specific query
cargo test --test tdengine_integration test_alert_history_query
```

## Troubleshooting

### Connection Errors

```
error sending request for url (http://192.168.2.106:6041/rest/sql/deribit)
```

- Check TDengine server is running: `systemctl status taosd`
- Verify network connectivity: `ping 192.168.2.106`
- Test REST API: `curl -u root:taosdata http://192.168.2.106:6041/rest/sql/deribit -d "SELECT 1"`

### Syntax Error

```
code: 9728, desc: "syntax error near..."
```

- TDengine SQL syntax differs from standard SQL
- Check TDengine documentation: https://docs.tdengine.com/
- Verify table/column names match schema

### No Data Returned

```
data: [], rows: 0
```

- Verify the instrument/index name exists: `SELECT DISTINCT index_name FROM deribit_volatility_index`
- Check time range - data may not exist for the specified time window
- For options data, use full instrument name (e.g., `BTC-31JAN25-3000-C`)

## TDengine REST API Reference

### Endpoint

```
POST http://host:6041/rest/sql/database
```

### Authentication

HTTP Basic Auth:
- Username: TDengine user (default: `root`)
- Password: TDengine password (default: `taosdata`)

### Request Body

Raw SQL string:

```
SELECT * FROM deribit_volatility_index LIMIT 10
```

### Response

```json
{
  "code": 0,
  "column_meta": [["field", "type", "size"]],
  "data": [[...]],
  "rows": 10
}
```

Error response:

```json
{
  "code": 9728,
  "desc": "syntax error...",
  "data": null
}
```

## Architecture

```
┌─────────────────┐
│  vol-llm-tool   │
│                 │
│  ┌───────────┐  │
│  │ Tool      │  │
│  │ - execute │──┼──┐
│  └───────────┘  │  │
│                 │  │
│  ┌───────────┐  │  │
│  │ Tdengine  │◄─┘  │
│  │ Client    │    │
│  └─────┬─────┘    │
└────────┼──────────┘
         │ HTTP
         │
         ▼
┌─────────────────┐
│  TDengine       │
│  192.168.2.106  │
│  :6041          │
└─────────────────┘
```
