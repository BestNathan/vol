# Vol Monitor Configuration Guide

## Overview

This project uses environment-specific configuration files with sensitive credentials managed separately:

| File | Purpose | Contains Sensitive Data |
|------|---------|------------------------|
| `config.dev.toml` | Local development | No (uses placeholders) |
| `config.prod.toml` | Production | No (uses env vars) |
| `config.toml` | Default (symlink) | No |
| `.env` | Local environment | **Yes** (gitignored) |
| `k8s/secrets.yaml` | K8s credentials | **Yes** (encrypted in production) |
| `k8s/configmap.yaml` | K8s non-sensitive config | No |

## Quick Start

### Local Development

1. **Setup environment:**
   ```bash
   # Copy the template
   cp .env.example .env

   # Edit .env with your credentials
   vim .env
   ```

2. **Run in development mode:**
   ```bash
   # Using the helper script
   ./scripts/run-dev.sh dev

   # Or manually
   source .env
   cargo run --release -- --config config.dev.toml
   ```

3. **Run with production config locally (for testing):**
   ```bash
   ./scripts/run-dev.sh prod
   ```

### Kubernetes Deployment

1. **Create secrets (one-time setup):**
   ```bash
   # Create namespace if it doesn't exist
   kubectl create namespace deribit

   # Create secrets with actual values
   kubectl create secret generic vol-monitor-secrets \
     --from-literal=deribit-client-id=<your-client-id> \
     --from-literal=deribit-client-secret=<your-client-secret> \
     --from-literal=feishu-app-id=<your-app-id> \
     --from-literal=feishu-app-secret=<your-app-secret> \
     --from-literal=feishu-receive-id=<your-receive-id> \
     -n deribit
   ```

2. **Deploy ConfigMap:**
   ```bash
   kubectl apply -f k8s/configmap.yaml
   ```

3. **Deploy application:**
   ```bash
   # One-click deploy
   ./k8s/deploy.sh latest

   # Or manual
   kubectl apply -f k8s/deployment.yaml
   ```

## Configuration Files

### config.dev.toml

Development configuration with:
- Shorter cooldown periods for testing
- Relaxed alert thresholds
- Feishu notifications disabled by default
- Human-readable log format
- OpenTelemetry disabled by default
- Local log output (`./logs`)

### config.prod.toml

Production configuration with:
- Standard cooldown periods
- Strict alert thresholds
- All notifications enabled
- JSON log format
- OpenTelemetry enabled
- Centralized logging (`/var/log/vol-monitor`)

## Environment Variables

### Required for Deribit Integration

| Variable | Description | Example |
|----------|-------------|---------|
| `DERIBIT_CLIENT_ID` | Deribit API client ID | `nhXng7Bj` |
| `DERIBIT_CLIENT_SECRET` | Deribit API client secret | `OxCGY...` |
| `DERIBIT_WS_URL` | WebSocket URL | `wss://www.deribit.com/ws/api/v2` |

### Required for Feishu Notifications

| Variable | Description | Example |
|----------|-------------|---------|
| `FEISHU_APP_ID` | Feishu app ID | `cli_a936b...` |
| `FEISHU_APP_SECRET` | Feishu app secret | `JnWnF...` |
| `FEISHU_RECEIVE_ID` | Message recipient ID | `oc_c2920...` |

### Optional Configuration

| Variable | Description | Default |
|----------|-------------|---------|
| `HTTPS_PROXY` | HTTP proxy for API calls | - |
| `RUST_LOG` | Log level filter | `info` |
| `OTEL_ENDPOINT` | Jaeger/OTLP endpoint | `http://localhost:4317` |
| `OTEL_SERVICE_NAME` | Service name for tracing | `vol-monitor` |

## Security Considerations

### Sensitive Data Handling

1. **Never commit `.env` files to git** - Already in `.gitignore`
2. **Use Kubernetes Secrets** - Not ConfigMaps for credentials
3. **Consider sealed-secrets or external-secrets** - For production K8s clusters
4. **Rotate credentials regularly** - Especially after team changes

### Git Safety

```bash
# Verify .env is ignored
git check-ignore .env

# If .env was accidentally committed:
git rm --cached .env
echo ".env" >> .gitignore
git commit -m "chore: remove .env from tracking"
```

## Configuration Differences

### Cooldown Periods

| Setting | Dev | Prod |
|---------|-----|------|
| Global cooldown | 60s | 300s |
| Short tenor | 120s | 600s |
| Medium tenor | 300s | 3600s |
| Long tenor | 600s | 14400s |

### Alert Thresholds (BTC Absolute IV)

| Setting | Dev | Prod |
|---------|-----|------|
| Short threshold | 0.80 | 0.55 |
| Medium threshold | 0.75 | 0.53 |
| Long threshold | 0.70 | 0.51 |

### Logging

| Setting | Dev | Prod |
|---------|-----|------|
| Log directory | `./logs` | `/var/log/vol-monitor` |
| Log format | Human-readable | JSON |
| Console level | debug | info |
| Retention | 3 days | 7 days |

## Troubleshooting

### "Config file not found"

Ensure you're running from the project root:
```bash
pwd  # Should be /root/nq-deribit
```

### "Missing credentials"

Check environment variables are loaded:
```bash
# For local dev
source .env
echo $DERIBIT_CLIENT_ID

# For K8s, check secret exists
kubectl get secret vol-monitor-secrets -n deribit
```

### "Proxy connection failed"

Update proxy settings:
```bash
# In .env for local dev
HTTPS_PROXY="http://your-proxy:port"

# In k8s/deployment.yaml for production
env:
- name: HTTPS_PROXY
  value: "http://your-proxy:port"
```

## Migration from v0.3.x

If migrating from the old single-file config:

1. **Backup existing config:**
   ```bash
   cp config.toml config.toml.backup
   ```

2. **Extract sensitive values:**
   ```bash
   # From old config.toml, copy:
   # - clients.deribit.client_id
   # - clients.deribit.client_secret
   # - notifications[].app_id, app_secret, receive_id
   ```

3. **Update to new format:**
   ```bash
   # Use config.prod.toml as base
   # Set credentials via environment variables or K8s Secrets
   ```

4. **Update deployment:**
   ```bash
   # Recreate ConfigMap without secrets
   kubectl apply -f k8s/configmap.yaml

   # Create Secrets
   kubectl create secret generic vol-monitor-secrets ...

   # Restart deployment
   kubectl rollout restart deployment/vol-monitor
   ```
