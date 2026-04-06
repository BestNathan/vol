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

⚠️ **Credentials must be injected via environment variables. Never commit secrets.**

### Quick Start
```bash
cp .env.example .env && vim .env
./scripts/run-dev.sh dev
```

### Required Environment Variables
| Variable | Purpose |
|----------|---------|
| `DERIBIT_CLIENT_ID` | Deribit API client ID |
| `DERIBIT_CLIENT_SECRET` | Deribit API client secret |
| `FEISHU_APP_ID` | Feishu app ID |
| `FEISHU_APP_SECRET` | Feishu app secret |
| `FEISHU_RECEIVE_ID` | Feishu recipient ID |

### Common Commands
```bash
# Run with config
./target/release/vol-monitor --config config.dev.toml

# Kubernetes deploy
kubectl apply -f k8s/

# Restart deployment
kubectl -n deribit rollout restart deployment/vol-monitor
```

### Full Documentation
- **Configuration Guide**: [docs/CONFIGURATION.md](docs/CONFIGURATION.md)
- **Design Document**: [docs/superpowers/specs/2026-04-06-config-separation-design.md](docs/superpowers/specs/2026-04-06-config-separation-design.md)

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
