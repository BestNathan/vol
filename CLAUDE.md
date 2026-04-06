# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Quick Start

```bash
# Build
cargo build --release

# Run (after setting up .env)
source .env && ./target/release/vol-monitor --config config.dev.toml

# Deploy
./k8s/deploy.sh latest
```

## Project Structure

```
nq-deribit/
├── crates/              # 10 workspace crates
│   ├── vol-core/       # Shared traits & data models
│   ├── vol-config/     # TOML configuration loading
│   ├── vol-tracing/    # Tracing utilities & span helpers
│   ├── vol-deribit/    # Deribit WebSocket client
│   ├── vol-datasource/ # Data providers (Deribit, CSV)
│   ├── vol-alert/      # Alert evaluation logic
│   ├── vol-rules/      # Rule processors
│   ├── vol-notification/# Alert delivery (stdout, Feishu)
│   ├── vol-engine/     # Monitoring engine orchestration
│   └── vol-monitor/    # Main binary
├── k8s/                 # Kubernetes manifests
├── docs/                # Documentation (see index below)
└── .cargo/config.toml  # Registry mirror (rsproxy.cn)
```

## Architecture Overview

| Crate | Purpose |
|-------|---------|
| `vol-core` | Shared traits (`DataSource`, `AlertHandler`, `NotificationHandler`) |
| `vol-config` | TOML-based configuration loading |
| `vol-tracing` | Tracing utilities, `TracedEvent<T>`, span helpers |
| `vol-deribit` | Deribit client: WebSocket, JSON-RPC, market data types |
| `vol-datasource` | DataSource trait implementation using vol-deribit |
| `vol-alert` | Alert evaluation logic |
| `vol-rules` | Rule processors |
| `vol-notification` | Alert delivery (stdout, Feishu webhook) |
| `vol-engine` | Monitoring engine orchestration |
| `vol-monitor` | Main binary - wires everything together |

**Data Flow:**
```
Deribit WebSocket → DataSource → mpsc → MonitoringEngine → Rules → Notifications
```

**Key Patterns:**
- Trait-based plugin architecture (vol-core traits)
- Async-first (tokio, no blocking I/O)
- Channel-based communication (mpsc, broadcast)
- Trace context propagation (`TracedEvent<T>`)

See [docs/architecture/](docs/architecture/) for full architecture documentation.

## Common Commands

```bash
# Build & Check
cargo build --release
cargo check --workspace

# Run
./target/release/vol-monitor --config config.dev.toml
RUST_LOG=info ./target/release/vol-monitor

# Test
cargo test --workspace

# Docker
docker build -t crpi-ck06yio90i1ttwlz.cn-beijing.personal.cr.aliyuncs.com/n_common/vol-monitor:latest .
docker push crpi-ck06yio90i1ttwlz.cn-beijing.personal.cr.aliyuncs.com/n_common/vol-monitor:latest

# Kubernetes
kubectl -n deribit logs -f deployment/vol-monitor
kubectl -n deribit rollout restart deployment/vol-monitor
kubectl -n deribit rollout undo deployment/vol-monitor
```

## Documentation Index

| Category | File | Description |
|----------|------|-------------|
| **Architecture** | [docs/architecture/overview.md](docs/architecture/overview.md) | System architecture, data flow diagrams |
| | [docs/architecture/crates.md](docs/architecture/crates.md) | Detailed crate documentation |
| **Deployment** | [docs/deployment/docker-build.md](docs/deployment/docker-build.md) | Docker build, multi-arch, ACR registry |
| | [docs/deployment/k8s-deployment.md](docs/deployment/k8s-deployment.md) | Kubernetes deployment, secrets, troubleshooting |
| **Integration** | [docs/integration/deribit.md](docs/integration/deribit.md) | Deribit API, WebSocket, proxy support |
| **Development** | [docs/development/common-modifications.md](docs/development/common-modifications.md) | Adding alerts, datasources, notifications |
| **Configuration** | [docs/CONFIGURATION.md](docs/CONFIGURATION.md) | Config file structure, environment variables |
| **Tracing** | [docs/tracing.md](docs/tracing.md) | Logging, Jaeger, trace context |

## Configuration

Credentials via environment variables (never commit secrets):

| Variable | Purpose |
|----------|---------|
| `DERIBIT_CLIENT_ID` | Deribit API client ID |
| `DERIBIT_CLIENT_SECRET` | Deribit API client secret |
| `FEISHU_APP_ID` | Feishu app ID |
| `FEISHU_APP_SECRET` | Feishu app secret |
| `FEISHU_RECEIVE_ID` | Feishu recipient ID |

```bash
# Quick start
cp .env.example .env && vim .env
./scripts/run-dev.sh dev
```

See [docs/CONFIGURATION.md](docs/CONFIGURATION.md) for full configuration guide.

## Kubernetes Deployment

| Resource | Name | Namespace |
|----------|------|-----------|
| Namespace | `deribit` | - |
| Deployment | `vol-monitor` | `deribit` |
| ConfigMap | `vol-monitor-config` | `deribit` |
| Secret | `vol-monitor-secrets` | `deribit` |

**Create secrets:**
```bash
kubectl create secret generic vol-monitor-secrets \
  --from-literal=DERIBIT_CLIENT_ID=<id> \
  --from-literal=DERIBIT_CLIENT_SECRET=<secret> \
  --from-literal=FEISHU_APP_ID=<app-id> \
  --from-literal=FEISHU_APP_SECRET=<app-secret> \
  --from-literal=FEISHU_RECEIVE_ID=<receive-id> \
  -n deribit
```

See [docs/deployment/k8s-deployment.md](docs/deployment/k8s-deployment.md) for complete deployment guide.
