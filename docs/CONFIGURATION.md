# Configuration & Environment Variables

## Overview

Config files live under `configs/` — one example per server type. Secrets go in `.env` (gitignored).

| File | Purpose |
|------|---------|
| `configs/vol-monitor.example.toml` | Vol monitor pipeline config example |
| `configs/vol-agent-server.example.toml` | Agent server config example (data/control plane) |
| `configs/vol-monitor.env.example` | Env template for vol-monitor |
| `configs/vol-agent-server.env.example` | Env template for agent server |
| `.env` | Local secrets (gitignored, **never commit**) |

**Quick Start — Agent Server**

```bash
cp configs/vol-agent-server.env.example .env   # edit API keys
source .env
cargo run -p vol-agent-server -- --config configs/vol-agent-server.example.toml
```

**Quick Start — Vol Monitor**

```bash
cp configs/vol-monitor.env.example .env        # edit credentials
source .env
cargo run -p vol-monitor -- --config configs/vol-monitor.example.toml
```

---

## Subsystem A — Volatility Monitoring Pipeline

Configuration for the Deribit market data pipeline: WebSocket connection, tenor definitions, alert rules, notifications, tracing.

### A.1 Environment Variables

All secrets are injected via environment variables. Copy `configs/vol-monitor.env.example` to `.env` and fill in:

#### Deribit API

| Variable | Description | Example |
|----------|-------------|---------|
| `DERIBIT_CLIENT_ID` | Deribit API client ID | `nhXng7Bj` |
| `DERIBIT_CLIENT_SECRET` | Deribit API client secret | `OxCGY...` |
| `DERIBIT_WS_URL` | WebSocket endpoint | `wss://www.deribit.com/ws/api/v2` |

For testnet use `wss://test.deribit.com/ws/api/v2`.

#### Feishu / Lark Notifications

| Variable | Description |
|----------|-------------|
| `FEISHU_APP_ID` | Feishu app ID |
| `FEISHU_APP_SECRET` | Feishu app secret |
| `FEISHU_RECEIVE_ID` | Message recipient (chat_id, open_id, or user_id) |

#### Proxy (required in China)

| Variable | Description |
|----------|-------------|
| `HTTPS_PROXY` | Proxy for HTTPS requests (e.g. `http://192.168.2.98:8890`) |
| `HTTP_PROXY` | Proxy for HTTP requests |
| `NO_PROXY` | Bypass list: `localhost,127.0.0.1,192.168.0.0/16,10.0.0.0/8` |

#### Logging & Tracing

| Variable | Description | Default |
|----------|-------------|---------|
| `RUST_LOG` | Log level filter | `info` |
| `OTEL_ENDPOINT` | OTLP collector endpoint | `http://localhost:4317` |
| `OTEL_SERVICE_NAME` | Service name in traces | `vol-monitor-dev` |
| `OTEL_SERVICE_NAMESPACE` | Namespace in traces | `deribit-dev` |
| `OTEL_DEPLOYMENT_ENVIRONMENT` | Environment tag | `development` |
| `OTEL_SAMPLE_RATE` | Sampling rate (0.0–1.0) | `1.0` |

#### App Config

| Variable | Description |
|----------|-------------|
| `VOL_MONITOR_CONFIG` | Path to TOML config file (e.g. `./configs/vol-monitor.example.toml`) |

### A.2 TOML Config Sections

The config file is selected at runtime (via `VOL_MONITOR_CONFIG` or `--config`). Available config presets:

#### `configs/vol-monitor.example.toml` — Reference for all deployments

This is the canonical example for the vol-monitor pipeline. Copy and tune for your environment
(tighten thresholds for production, adjust cooldowns, enable/disable Feishu, switch log format).

### A.3 Config Reference

#### `[engine]`

| Key | Type | Description |
|-----|------|-------------|
| `hot_reload` | bool | Watch config file for changes |
| `hot_reload_interval_secs` | int | Config reload check interval |
| `channel_buffer_size` | int | Event bus channel capacity |
| `alert_cooldown_secs` | int | Global minimum seconds between same-type alerts |

#### `[engine.tenor_cooldowns]`

| Key | Type | Description |
|-----|------|-------------|
| `short_secs` | int | Cooldown for short-tenor alerts |
| `medium_secs` | int | Cooldown for medium-tenor alerts |
| `long_secs` | int | Cooldown for long-tenor alerts |

#### `[tenors]`

DTE (Days to Expiry) bucketing:

| Key | Type | Description |
|-----|------|-------------|
| `short_max_dte` | int | Short tenor max DTE (default 7) |
| `medium_min_dte` | int | Medium tenor min DTE (default 20) |
| `medium_max_dte` | int | Medium tenor max DTE (default 40) |
| `long_min_dte` | int | Long tenor min DTE (default 80) |
| `long_max_dte` | int | Long tenor max DTE (default 200) |

#### `[clients.deribit]`

| Key | Type | Description |
|-----|------|-------------|
| `ws_url` | string | Deribit WebSocket URL. Credentials from `DERIBIT_CLIENT_ID` / `DERIBIT_CLIENT_SECRET` env vars. |

#### `[[datasources]]` (array)

Each entry defines one data feed:

| Key | Type | Description |
|-----|------|-------------|
| `id` | string | Unique identifier |
| `type` | string | `"volatility"` or `"portfolio"` |
| `symbols` | []string | For volatility: `["BTC", "ETH"]` |
| `currencies` | []string | For portfolio: `["BTC", "ETH"]` |
| `poll_interval_secs` | int | Poll interval (portfolio only) |

#### `[[rules]]` (array)

Each entry defines one alert rule. Common fields:

| Key | Type | Description |
|-----|------|-------------|
| `id` | string | Unique rule identifier |
| `type` | string | Rule type (see below) |
| `enabled` | bool | Enable/disable this rule |
| `notifications` | []string | Notification IDs to route alerts to |

**Rule types:**

`absolute-iv` — Trigger when IV exceeds a threshold:
| Key | Type |
|-----|------|
| `symbol` | string |
| `short_threshold` / `medium_threshold` / `long_threshold` | float |
| `short_atm_threshold` / `medium_atm_threshold` / `long_atm_threshold` | float |
| `dte_atm_thresholds` | map (DTE → threshold) |

`rate-change` — Trigger on IV change over time windows:
| Key | Type |
|-----|------|
| `symbol` | string |
| `window_1h_threshold` / `window_4h_threshold` / `window_24h_threshold` | float |

`term-structure` — Trigger on spread anomalies:
| Key | Type |
|-----|------|
| `short_long_spread_threshold` | float |

`skew` — Trigger on put/call skew divergence:
| Key | Type |
|-----|------|
| `symbol` | string |
| `threshold` | float |

`margin-ratio` — Trigger on portfolio margin ratio:
| Key | Type |
|-----|------|
| `datasources` | []string |
| `min_threshold` | float |

`portfolio` — Trigger on Greek/balance metrics:
```toml
metrics = [
    { type = "delta_exposure", enabled = true, min_threshold = -100.0, max_threshold = 100.0 },
    { type = "total_greeks", enabled = true, gamma_threshold = 50.0, vega_threshold = 200.0, theta_threshold = 100.0 },
    { type = "free_balance", enabled = true, min_threshold = 0.5 },
    { type = "margin_ratio", enabled = true, min_threshold = 1.25 },
]
```

#### `[[notifications]]` (array)

| Key | Type | Description |
|-----|------|-------------|
| `id` | string | Unique ID (referenced by rules) |
| `type` | string | `"stdout"` or `"feishu"` |
| `enabled` | bool | Enable/disable this channel |

Feishu credentials are read from env vars: `FEISHU_APP_ID`, `FEISHU_APP_SECRET`, `FEISHU_RECEIVE_ID`.

#### `[tracing]`

```toml
[tracing.logging]
log_dir = "./logs"
log_prefix = "vol-monitor-dev"
retention_days = 3
max_file_size_mb = 100
json_format = false
console_level = "debug"
file_level = "debug"
error_file = true

[tracing.opentelemetry]
enabled = false
endpoint = "http://localhost:4317"
service_name = "vol-monitor-dev"
service_namespace = "deribit-dev"
deployment_environment = "development"
sample_rate = 1.0

[tracing.opentelemetry.batch]
max_queue_size = 512
max_batch_size = 128
scheduled_delay_millis = 1000
max_export_timeout_millis = 5000
```

### A.4 Dev vs Prod Summary

| Setting | Dev | Prod |
|---------|-----|------|
| Global cooldown | 60s | 300s |
| Short tenor cooldown | 120s | 600s |
| Medium tenor cooldown | 300s | 3600s |
| Long tenor cooldown | 600s | 14400s |
| BTC short IV threshold | 0.80 | 0.55 |
| BTC medium IV threshold | 0.75 | 0.53 |
| BTC long IV threshold | 0.70 | 0.51 |
| Log directory | `./logs` | `/var/log/vol-monitor` |
| Log format | text | JSON |
| Console level | debug | info |
| Retention | 3 days | 7 days |
| OTEL | disabled | enabled |
| Feishu | disabled | enabled |

---

## Subsystem B — LLM Agent Framework

Configuration for LLM providers, ReAct agents, MCP servers, skills, and the Agent Advice bridge.

### B.1 LLM Provider Environment Variables

| Variable | Description |
|----------|-------------|
| `ANTHROPIC_AUTH_TOKEN` | Anthropic API key (or DashScope proxy token) |
| `OPENAI_API_KEY` | OpenAI API key |

### B.2 `[[llm_providers]]` — Provider Definitions

Define one or more LLM providers in the TOML config. The `api_key` field supports three formats:

| Format | Example | Behavior |
|--------|---------|----------|
| Literal | `"sk-abc123"` | Use the value directly |
| Env var | `"${ANTHROPIC_AUTH_TOKEN}"` | Read from environment variable |
| Env + fallback | `"${OPENAI_API_KEY:sk-default}"` | Use env var, fall back to literal if unset |

```toml
# Anthropic via DashScope proxy
[[llm_providers]]
id = "anthropic-main"
provider = "anthropic"
model = "claude-sonnet-4-6"
api_key = "${ANTHROPIC_AUTH_TOKEN}"
base_url = "https://coding.dashscope.aliyuncs.com/apps/anthropic"

# Local model service
[[llm_providers]]
id = "qwen-local"
provider = "openai"
model = "qwen3.6-plus"
api_key = "not-needed"
base_url = "http://192.168.2.162:31693/v1"
```

| Key | Type | Description |
|-----|------|-------------|
| `id` | string | Unique ID referenced by `[agent_advice]` and agents |
| `provider` | string | `"anthropic"` or `"openai"` |
| `model` | string | Model name |
| `api_key` | string | API key (literal or `${ENV_VAR}`) |
| `base_url` | string | API base URL |

### B.3 `[agent_advice]` — Agent Advice Bridge

Connects the monitoring pipeline to LLM analysis. When an alert fires, the Agent Advice system uses a ReAct agent to analyze it and sends AI-generated recommendations via Feishu.

```toml
[agent_advice]
enabled = true
cooldown_secs = 300
max_analyses_per_hour = 20
llm_provider_id = "anthropic-main"
```

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | bool | `false` | Enable AI analysis of alerts |
| `cooldown_secs` | int | `300` | Minimum seconds between analyses |
| `max_analyses_per_hour` | int | `20` | Rate limit per rolling hour |
| `llm_provider_id` | string | — | Must match a `[[llm_providers]]` id |

Rate limiting uses both cooldown and hourly cap — both must be satisfied before an analysis proceeds.

### B.4 Agent Server Configuration

See `configs/vol-agent-server.example.toml` for the full annotated example covering server roles
(standalone data-plane, standalone control-plane, combined), control-plane node registration and
routing, data-plane identity, runtime store configuration, and tracing.

### B.5 Model Service

The default model service runs at `http://192.168.2.162:31693` with these available models:

| Model ID | Provider Type |
|----------|---------------|
| `gpt5.5` | openai-compatible |
| `coding` | openai-compatible |
| `qwen3.6-plus` | openai-compatible |
| `glm5.1` | openai-compatible |

Configure in `[[llm_providers]]` with `provider = "openai"` and the appropriate `base_url`.

---

## Kubernetes Deployment

### Secrets

Credentials are injected via K8s Secrets, not baked into the ConfigMap:

```bash
kubectl create secret generic vol-monitor-secrets \
  --from-literal=deribit-client-id=<id> \
  --from-literal=deribit-client-secret=<secret> \
  --from-literal=feishu-app-id=<app-id> \
  --from-literal=feishu-app-secret=<app-secret> \
  --from-literal=feishu-receive-id=<receive-id> \
  -n deribit
```

### ConfigMap

The TOML config (without secrets) is deployed as a ConfigMap and mounted at `/etc/vol-monitor/config.toml`:

```bash
kubectl apply -f k8s/vol-monitor/configmap.yaml
```

### Deploy

```bash
cd k8s && bash deploy.sh
```

### Security Checklist

- [ ] `.env` is in `.gitignore`
- [ ] No credentials in ConfigMap — only env var references
- [ ] K8s Secrets used, not ConfigMap literals
- [ ] Consider `sealed-secrets` or `external-secrets` for production
- [ ] Rotate credentials after team changes
