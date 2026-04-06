# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
# Build all workspace members
cargo build --release

# Check without building
cargo check --workspace

# Run with environment variables (after setting up .env)
source .env && ./target/release/vol-monitor --config config.dev.toml

# Run with logging
RUST_LOG=info ./target/release/vol-monitor --config config.dev.toml

# Run tests
cargo test --workspace
```

## Docker Build

### Prerequisites

**Cargo Registry Mirror (required for reliable builds in China):**

The project uses rsproxy.cn mirror for crates.io. Ensure `.cargo/config.toml` exists:

```toml
[source.crates-io]
replace-with = 'rsproxy-sparse'
[source.rsproxy]
registry = "https://rsproxy.cn/crates.io-index"
[source.rsproxy-sparse]
registry = "sparse+https://rsproxy.cn/index/"
[registries.rsproxy]
index = "https://rsproxy.cn/crates.io-index"
[net]
git-fetch-with-cli = true
```

### Dockerfile (Multi-stage Build)

```dockerfile
# Stage 1: Build
FROM rust:latest AS builder

WORKDIR /app

# Copy cargo config for registry mirror
COPY .cargo ./.cargo

# Copy dependency definitions first for better caching
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/

# Build release binary
RUN cargo build --release -vv

# Stage 2: Runtime
FROM debian:bookworm-slim

# Install CA certificates using Aliyun mirror
RUN sed -i 's|deb.debian.org|mirrors.aliyun.com|g' /etc/apt/sources.list.d/debian.sources && \
    apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

# Copy the binary from builder
COPY --from=builder /app/target/release/vol-monitor /usr/local/bin/vol-monitor

WORKDIR /app

# Run the binary
ENTRYPOINT ["/usr/local/bin/vol-monitor"]
```

**Key points:**
- `.cargo/config.toml` must be copied into the image for rsproxy mirror to work
- Using sparse registry protocol for faster dependency resolution
- Aliyun mirror for apt packages (`deb.debian.org` → `mirrors.aliyun.com`)
- Multi-stage build keeps final image ~95MB

### Build and Deploy to K8s

```bash
# Build image (single architecture - current platform)
docker build -t crpi-ck06yio90i1ttwlz.cn-beijing.personal.cr.aliyuncs.com/n_common/vol-monitor:latest .

# Push to ACR
docker push crpi-ck06yio90i1ttwlz.cn-beijing.personal.cr.aliyuncs.com/n_common/vol-monitor:latest

# Deploy to k8s
kubectl apply -f k8s/namespace.yaml
kubectl apply -f k8s/configmap.yaml
kubectl apply -f k8s/deployment.yaml
```

Or use the one-click deploy script:
```bash
./k8s/deploy.sh latest
```

### Multi-Architecture Builds

The project supports building multi-architecture images for `linux/amd64` and `linux/arm64`.

**Setup (one-time):**

```bash
# Create multi-arch builder (requires Docker buildx)
docker buildx create --use --name multiarch --driver docker-container
docker buildx inspect multiarch --bootstrap
```

**Build multi-arch image:**

```bash
# Build and push to ACR
docker buildx build --platform linux/amd64,linux/arm64 \
    --push -t crpi-ck06yio90i1ttwlz.cn-beijing.personal.cr.aliyuncs.com/n_common/vol-monitor:latest .
```

**Verify multi-arch image:**

```bash
docker buildx imagetools inspect crpi-ck06yio90i1ttwlz.cn-beijing.personal.cr.aliyuncs.com/n_common/vol-monitor:latest
# Output should show:
#   Manifests:
#     ...  # linux/amd64
#     ...  # linux/arm64
```

**Notes:**
- First build may take 5-10 minutes due to QEMU emulation for arm64
- `--push` is required (multi-arch images cannot be loaded locally)
- The resulting image is a manifest list containing both architectures
- Kubernetes will automatically pull the correct architecture for each node
- After multi-arch setup, `./k8s/deploy.sh` uses buildx automatically

## Kubernetes Deployment

### Cluster Architecture

- **Nodes**: 3-node cluster (k8s-master, k8s-worker1/amd64, rock-5b-plus/arm64)
- **Ingress**: Higress
- **Registry**: Aliyun Container Registry (ACR) private

### Deployment Configuration

| Resource | Name | Namespace |
|----------|------|-----------|
| Namespace | `deribit` | - |
| Deployment | `vol-monitor` | `deribit` |
| ConfigMap | `vol-monitor-config` | `deribit` |
| Secret | `vol-monitor-secrets` | `deribit` |

See **Configuration** section above for pod spec details and management commands.

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
Deribit WebSocket → VolatilityDataSource → mpsc channel → main event loop
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

**IMPORTANT (v0.5.0+):** Sensitive credentials must be injected via environment variables.
Do not commit actual credentials to config files.

### Configuration Files

| File | Purpose | Contains Secrets |
|------|---------|------------------|
| `config.toml` | Default (production mode) | No (env var injection) |
| `config.dev.toml` | Local development | No |
| `config.prod.toml` | Production | No |
| `.env` | Local environment vars | **Yes** (gitignored) |

### Environment Variables

**Required for Deribit:**
```bash
DERIBIT_CLIENT_ID="your-client-id"
DERIBIT_CLIENT_SECRET="your-client-secret"
DERIBIT_WS_URL="wss://www.deribit.com/ws/api/v2"
```

**Required for Feishu Notifications:**
```bash
FEISHU_APP_ID="cli_xxx"
FEISHU_APP_SECRET="xxx"
FEISHU_RECEIVE_ID="oc_xxx"
```

**Optional:**
```bash
HTTPS_PROXY="http://proxy:port"
RUST_LOG="info"
OTEL_ENDPOINT="http://jaeger:4317"
```

### Quick Start (Local Development)

```bash
# 1. Copy environment template
cp .env.example .env

# 2. Edit with your credentials
vim .env

# 3. Run development mode
./scripts/run-dev.sh dev

# Or manually:
source .env
cargo run --release -- --config config.dev.toml
```

### Command Line

```bash
# Use specific config file
./target/release/vol-monitor --config config.prod.toml
./target/release/vol-monitor -c config.dev.toml

# Show help
./target/release/vol-monitor --help
```

### Configuration Structure

```toml
[engine]
hot_reload = true
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
long_max_dte = 200

[clients.deribit]
ws_url = "wss://www.deribit.com/ws/api/v2"
# Credentials from env vars (not in file)

[[datasources]]
id = "deribit-markets"
type = "volatility"
symbols = ["BTC", "ETH"]

[[datasources]]
id = "portfolio"
type = "portfolio"
currencies = ["BTC", "ETH"]
poll_interval_secs = 30

[[notifications]]
id = "feishu-alerts"
type = "feishu"
# Credentials from env vars
enabled = true

[[notifications]]
id = "stdout"
type = "stdout"
enabled = true

[[rules]]
# ... rule configurations
```

### Dev vs Prod Differences

| Setting | Dev | Prod |
|---------|-----|------|
| Cooldowns | Short (60-600s) | Long (300-14400s) |
| IV Thresholds | Relaxed (0.70-0.90) | Strict (0.51-0.75) |
| Log Level | debug | info |
| Log Format | Human-readable | JSON |
| Feishu | Disabled | Enabled |
| OpenTelemetry | Disabled | Enabled |

### Kubernetes Deployment

**1. Create Secrets (one-time):**
```bash
kubectl create namespace deribit

kubectl create secret generic vol-monitor-secrets \
  --from-literal=DERIBIT_CLIENT_ID=<actual-id> \
  --from-literal=DERIBIT_CLIENT_SECRET=<actual-secret> \
  --from-literal=FEISHU_APP_ID=<actual-app-id> \
  --from-literal=FEISHU_APP_SECRET=<actual-app-secret> \
  --from-literal=FEISHU_RECEIVE_ID=<actual-receive-id> \
  -n deribit
```

**2. Deploy ConfigMap (non-sensitive config):**
```bash
kubectl apply -f k8s/configmap.yaml
```

**3. Deploy application:**
```bash
kubectl apply -f k8s/deployment.yaml
# Or use the deploy script
./k8s/deploy.sh latest
```

**Pod Spec Highlights:**
```yaml
spec:
  nodeSelector:
    kubernetes.io/arch: amd64
  containers:
  - name: vol-monitor
    image: <acr-image>:latest
    workingDir: /etc/vol-monitor
    args:
      - "--config"
      - "config.toml"
    env:
    - name: DERIBIT_CLIENT_ID
      valueFrom:
        secretKeyRef:
          name: vol-monitor-secrets
          key: DERIBIT_CLIENT_ID
    - name: DERIBIT_CLIENT_SECRET
      valueFrom:
        secretKeyRef:
          name: vol-monitor-secrets
          key: DERIBIT_CLIENT_SECRET
    # ... more env vars from secrets
    volumeMounts:
    - name: config
      mountPath: /etc/vol-monitor
      readOnly: true
  volumes:
  - name: config
    configMap:
      name: vol-monitor-config
```

### Management Commands

```bash
# View logs
kubectl -n deribit logs -f deployment/vol-monitor

# View status
kubectl -n deribit get pods -l app=vol-monitor

# Restart deployment
kubectl -n deribit rollout restart deployment/vol-monitor

# Rollback
kubectl -n deribit rollout undo deployment/vol-monitor

# Update secrets (then restart)
kubectl create secret generic vol-monitor-secrets \
  --from-literal=DERIBIT_CLIENT_ID=<new-id> \
  --from-literal=DERIBIT_CLIENT_SECRET=<new-secret> \
  -n deribit --dry-run=client -o yaml | kubectl apply -f -
kubectl -n deribit rollout restart deployment/vol-monitor
```

### Migration from v0.4.x

If your config files contain actual credentials:

1. **Move credentials to .env:**
   ```bash
   cp .env.example .env
   # Edit .env with your credentials
   ```

2. **Clean config files:**
   ```bash
   # Remove client_id, client_secret, app_id, app_secret, receive_id from config files
   # They will be loaded from environment variables
   ```

3. **For K8s, create Secrets:**
   ```bash
   # Create secrets as shown above
   # Update deployment.yaml if needed
   ```

**Full documentation:** See [docs/CONFIGURATION.md](docs/CONFIGURATION.md)
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

### Volatility DataSource

The Volatility datasource (`crates/vol-datasource/src/volatility.rs`) uses `DeribitClient`:

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
# In .env file (local development)
HTTPS_PROXY="http://192.168.2.98:8890"

# Or as environment variable
export HTTPS_PROXY="http://192.168.2.98:8890"
./target/release/vol-monitor --config config.toml
```

For Kubernetes, set in `deployment.yaml`:
```yaml
env:
- name: HTTPS_PROXY
  value: "http://192.168.2.98:8890"
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

## Tracing & Logging

### View logs

```bash
tail -f logs/vol-monitor.log
tail -f logs/vol-monitor.error.log  # ERROR only
cat logs/vol-monitor.log | jq '.span.trace_id'  # Extract trace IDs
```

### Query logs by trace_id

```bash
cat logs/vol-monitor.log | jq 'select(.span.trace_id == "tr_abc1234")'
```

### Test tracing locally

```bash
# Start Jaeger
docker run --rm -d --name jaeger -p 4317:4317 -p 16686:16686 jaegertracing/all-in-one:latest

# Run with tracing
./target/release/vol-monitor

# Open Jaeger UI
open http://localhost:16686
```

### Documentation

See [docs/tracing.md](docs/tracing.md) for comprehensive tracing architecture documentation.
