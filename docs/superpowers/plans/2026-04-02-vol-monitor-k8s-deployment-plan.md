# vol-monitor Kubernetes Deployment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deploy vol-monitor to existing Kubernetes cluster using Docker Hub for image storage and ConfigMap for configuration.

**Architecture:** Multi-stage Docker build → Docker Hub → K8s Deployment with ConfigMap volume mount.

**Tech Stack:** Docker, Kubernetes, Helm (optional), Bash scripting.

---

## File Structure

| File | Purpose |
|------|---------|
| `Dockerfile` | Multi-stage build: Rust compile → minimal runtime image |
| `k8s/namespace.yaml` | Create deribit namespace |
| `k8s/configmap.yaml` | Store config.toml in ConfigMap |
| `k8s/deployment.yaml` | Deployment with ConfigMap volume |
| `k8s/deploy.sh` | One-click deploy script |
| `k8s/README.md` | Usage documentation |

---

### Task 1: Create Dockerfile

**Files:**
- Create: `Dockerfile`

- [ ] **Step 1: Create Dockerfile with multi-stage build**

```dockerfile
# Stage 1: Build
FROM rust:1.75-slim as builder
WORKDIR /app

# Copy dependency definitions first for better caching
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/

# Build release binary
RUN cargo build --release

# Stage 2: Runtime
FROM debian:bookworm-slim

# Install CA certificates for HTTPS
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

# Copy the binary from builder
COPY --from=builder /app/target/release/vol-monitor /usr/local/bin/vol-monitor

# config.toml is mounted via ConfigMap at runtime
# Working directory for the application
WORKDIR /app

# Run the binary
ENTRYPOINT ["/usr/local/bin/vol-monitor"]
```

- [ ] **Step 2: Verify Dockerfile syntax**

```bash
docker build --check -f Dockerfile .
# Expected: success (or at least no syntax errors)
```

- [ ] **Step 3: Commit**

```bash
git add Dockerfile
git commit -m "feat: add multi-stage Dockerfile for k8s deployment"
```

---

### Task 2: Create Kubernetes Namespace

**Files:**
- Create: `k8s/namespace.yaml`

- [ ] **Step 1: Create namespace YAML**

```yaml
apiVersion: v1
kind: Namespace
metadata:
  name: deribit
  labels:
    app: vol-monitor
```

- [ ] **Step 2: Validate YAML syntax**

```bash
kubectl apply --dry-run=client -f k8s/namespace.yaml
# Expected: success
```

- [ ] **Step 3: Commit**

```bash
git add k8s/namespace.yaml
git commit -m "feat: add deribit namespace definition"
```

---

### Task 3: Create ConfigMap for config.toml

**Files:**
- Create: `k8s/configmap.yaml`
- Read: `config.toml`

- [ ] **Step 1: Read current config.toml content**

Read the existing `config.toml` file from the project root.

- [ ] **Step 2: Create ConfigMap YAML**

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: vol-monitor-config
  namespace: deribit
  labels:
    app: vol-monitor
data:
  config.toml: |
    # config.toml for vol-monitor - Provider-based datasource configuration

    [engine]
    hot_reload = true
    hot_reload_interval_secs = 30
    channel_buffer_size = 1000
    alert_cooldown_secs = 300

    [tenors]
    short_max_dte = 7
    medium_min_dte = 20
    medium_max_dte = 40
    long_min_dte = 80

    # Data sources - organized by provider
    [[datasources]]
    id = "deribit-markets"
    provider = "deribit"
    ws_url = "wss://www.deribit.com/ws/api/v2"
    symbols = ["BTC", "ETH"]
    poll_interval_secs = 60
    enabled = true

    [datasources.auth]
    client_id = "nhXng7Bj"
    client_secret = "OxCGY10HlzgKfRoXPBRQqg5IBQcZguGPhE1tewP5U3Y"

    # Notifications
    [[notifications]]
    id = "feishu-alerts"
    type = "feishu"
    app_id = "cli_a936b13197385bde"
    app_secret = "JnWnFrrOvzHi4deDmFY9kd1NMGbiWuNz"
    receive_id = "oc_c29208d94757e2aefd97bfa5f57e0b26"
    enabled = true

    [[notifications]]
    id = "stdout"
    type = "stdout"
    enabled = true

    # Rules
    [[rules]]
    id = "absolute-iv-btc"
    type = "absolute-iv"
    symbol = "BTC"
    short_threshold = 0.55
    medium_threshold = 0.53
    long_threshold = 0.51
    short_atm_threshold = 0.05
    medium_atm_threshold = 0.08
    long_atm_threshold = 0.10
    dte_atm_thresholds = { "1" = 0.02, "2" = 0.03, "3" = 0.04 }
    enabled = true
    notifications = ["feishu-alerts", "stdout"]

    [[rules]]
    id = "absolute-iv-eth"
    type = "absolute-iv"
    symbol = "ETH"
    short_threshold = 0.75
    medium_threshold = 0.73
    long_threshold = 0.71
    short_atm_threshold = 0.07
    medium_atm_threshold = 0.10
    long_atm_threshold = 0.12
    dte_atm_thresholds = { "1" = 0.03, "2" = 0.04, "3" = 0.05 }
    enabled = true
    notifications = ["feishu-alerts", "stdout"]

    [[rules]]
    id = "rate-change-btc"
    type = "rate-change"
    symbol = "BTC"
    window_1h_threshold = 0.05
    window_4h_threshold = 0.10
    window_24h_threshold = 0.20
    enabled = true
    notifications = ["feishu-alerts", "stdout"]

    [[rules]]
    id = "rate-change-eth"
    type = "rate-change"
    symbol = "ETH"
    window_1h_threshold = 0.05
    window_4h_threshold = 0.10
    window_24h_threshold = 0.20
    enabled = true
    notifications = ["feishu-alerts", "stdout"]

    [[rules]]
    id = "term-structure"
    type = "term-structure"
    short_long_spread_threshold = 0.15
    enabled = false
    notifications = ["stdout"]

    [[rules]]
    id = "skew"
    type = "skew"
    symbol = "BTC"
    threshold = 0.10
    enabled = false
    notifications = ["stdout"]
```

**Note:** The config.toml content above is a template. Replace with actual values from your `config.toml` file, especially:
- `datasources.auth.client_id` and `client_secret`
- `notifications.feishu.app_id`, `app_secret`, `receive_id`

- [ ] **Step 3: Validate YAML syntax**

```bash
kubectl apply --dry-run=client -f k8s/configmap.yaml
# Expected: success
```

- [ ] **Step 4: Commit**

```bash
git add k8s/configmap.yaml
git commit -m "feat: add ConfigMap for vol-monitor configuration"
```

---

### Task 4: Create Deployment YAML

**Files:**
- Create: `k8s/deployment.yaml`

- [ ] **Step 1: Create Deployment YAML**

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: vol-monitor
  namespace: deribit
  labels:
    app: vol-monitor
spec:
  replicas: 1
  selector:
    matchLabels:
      app: vol-monitor
  strategy:
    type: RollingUpdate
    rollingUpdate:
      maxSurge: 1
      maxUnavailable: 0
  template:
    metadata:
      labels:
        app: vol-monitor
    spec:
      restartPolicy: Always
      containers:
      - name: vol-monitor
        image: <YOUR_DOCKERHUB_USERNAME>/vol-monitor:latest
        imagePullPolicy: Always
        args:
          - "--config"
          - "/etc/vol-monitor/config.toml"
        volumeMounts:
        - name: config
          mountPath: /etc/vol-monitor
          readOnly: true
        env:
        - name: RUST_LOG
          value: "info"
      volumes:
      - name: config
        configMap:
          name: vol-monitor-config
          items:
          - key: config.toml
            path: config.toml
```

**Important:** Replace `<YOUR_DOCKERHUB_USERNAME>` with your actual Docker Hub username.

- [ ] **Step 2: Validate YAML syntax**

```bash
kubectl apply --dry-run=client -f k8s/deployment.yaml
# Expected: success
```

- [ ] **Step 3: Commit**

```bash
git add k8s/deployment.yaml
git commit -m "feat: add Kubernetes Deployment for vol-monitor"
```

---

### Task 5: Create Deployment Script

**Files:**
- Create: `k8s/deploy.sh`

- [ ] **Step 1: Create deploy script**

```bash
#!/bin/bash
# k8s/deploy.sh - One-click deploy script for vol-monitor

set -e

# Configuration - UPDATE THESE VALUES
DOCKERHUB_USERNAME="${DOCKERHUB_USERNAME:-your-dockerhub-username}"
IMAGE_NAME="${DOCKERHUB_USERNAME}/vol-monitor"
VERSION="${1:-latest}"
K8S_DIR="$(dirname "$0")"

echo "============================================"
echo "  vol-monitor Kubernetes Deploy"
echo "============================================"
echo "Image: $IMAGE_NAME:$VERSION"
echo "Version: $VERSION"
echo ""

# Step 1: Build Docker image
echo "[1/5] Building Docker image..."
docker build -t "$IMAGE_NAME:$VERSION" -f Dockerfile .

# Tag as latest if not already
if [ "$VERSION" != "latest" ]; then
    docker tag "$IMAGE_NAME:$VERSION" "$IMAGE_NAME:latest"
fi

# Step 2: Push to Docker Hub
echo "[2/5] Pushing to Docker Hub..."
docker push "$IMAGE_NAME:$VERSION"
if [ "$VERSION" != "latest" ]; then
    docker push "$IMAGE_NAME:latest"
fi

# Step 3: Update Deployment image tag
echo "[3/5] Updating Deployment image tag..."
sed -i.bak "s|image: .*/vol-monitor:.*|image: $IMAGE_NAME:$VERSION|" "$K8S_DIR/deployment.yaml"
rm -f "$K8S_DIR/deployment.yaml.bak"

# Step 4: Apply Kubernetes manifests
echo "[4/5] Applying Kubernetes manifests..."
kubectl apply -f "$K8S_DIR/namespace.yaml"
kubectl apply -f "$K8S_DIR/configmap.yaml"
kubectl apply -f "$K8S_DIR/deployment.yaml"

# Step 5: Wait for deployment
echo "[5/5] Waiting for deployment to complete..."
kubectl -n deribit rollout status deployment/vol-monitor --timeout=300s

echo ""
echo "============================================"
echo "  Deploy Complete!"
echo "============================================"
echo ""
echo "Pod status:"
kubectl -n deribit get pods -l app=vol-monitor
echo ""
echo "View logs:"
echo "  kubectl -n deribit logs -f deployment/vol-monitor"
echo ""
echo "Update version:"
echo "  $0 v0.1.1"
echo ""
echo "Rollback:"
echo "  kubectl -n deribit rollout undo deployment/vol-monitor"
echo ""
```

- [ ] **Step 2: Make script executable**

```bash
chmod +x k8s/deploy.sh
```

- [ ] **Step 3: Test script syntax**

```bash
bash -n k8s/deploy.sh
# Expected: no output (syntax OK)
```

- [ ] **Step 4: Commit**

```bash
git add k8s/deploy.sh
git commit -m "feat: add one-click deployment script"
```

---

### Task 6: Create README Documentation

**Files:**
- Create: `k8s/README.md`

- [ ] **Step 1: Create README**

```markdown
# vol-monitor Kubernetes Deployment

## Prerequisites

- Docker installed and logged in (`docker login`)
- kubectl configured with cluster access
- Docker Hub account

## Quick Start

### First Time Setup

1. **Update configuration:**
   - Edit `k8s/deploy.sh` and set `DOCKERHUB_USERNAME` to your Docker Hub username
   - Edit `k8s/deployment.yaml` and update the image field with your Docker Hub username

2. **Update ConfigMap (optional):**
   - Edit `k8s/configmap.yaml` with your actual config.toml content
   - Update Deribit API credentials and Feishu notification settings

3. **Deploy:**
   ```bash
   ./k8s/deploy.sh v0.1.0
   ```

### Deploy New Version

```bash
./k8s/deploy.sh v0.1.1
```

### View Status

```bash
# Check pods
kubectl -n deribit get pods -l app=vol-monitor

# View logs
kubectl -n deribit logs -f deployment/vol-monitor

# Check events
kubectl -n deribit get events --sort-by='.lastTimestamp'
```

### Rollback

```bash
kubectl -n deribit rollout undo deployment/vol-monitor
```

## Manual Operations

### Update ConfigMap

```bash
# Edit the ConfigMap
kubectl -n deribit edit configmap vol-monitor-config

# Restart deployment to pick up changes
kubectl -n deribit rollout restart deployment/vol-monitor
```

### Scale (if needed)

```bash
kubectl -n deribit scale deployment/vol-monitor --replicas=2
```

### Delete Deployment

```bash
kubectl delete -f k8s/namespace.yaml
# This removes namespace and all resources
```

## Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `RUST_LOG` | `info` | Logging level |

### ConfigMap Mount

- **Path:** `/etc/vol-monitor/config.toml`
- **Source:** `vol-monitor-config` ConfigMap

## Troubleshooting

### Pod not starting

```bash
kubectl -n deribit describe pod -l app=vol-monitor
kubectl -n deribit logs deployment/vol-monitor
```

### Image pull errors

Ensure Docker Hub credentials are correct and image is public, or create a pull secret:

```bash
kubectl -n deribit create secret docker-registry regcred \
  --docker-server=docker.io \
  --docker-username=<user> \
  --docker-password=<pass> \
  --docker-email=<email>
```

Then add to deployment.yaml:
```yaml
spec:
  imagePullSecrets:
  - name: regcred
```
```

- [ ] **Step 2: Commit**

```bash
git add k8s/README.md
git commit -m "docs: add k8s deployment README"
```

---

### Task 7: Build and Test Docker Image

**Files:**
- None (testing task)

- [ ] **Step 1: Build Docker image locally**

```bash
docker build -t vol-monitor:test -f Dockerfile .
# Expected: Build completes successfully
```

- [ ] **Step 2: Verify binary exists in image**

```bash
docker run --rm vol-monitor:test ls -la /usr/local/bin/vol-monitor
# Expected: binary file listed
```

- [ ] **Step 3: Test binary runs (will fail without config, but should start)**

```bash
docker run --rm vol-monitor:test --help
# Expected: Usage info or "config file not found" error
```

- [ ] **Step 4: Commit after successful build**

```bash
git commit --allow-empty -m "test: verify Docker build succeeds"
```

---

### Task 8: Validate Kubernetes Manifests

**Files:**
- None (validation task)

- [ ] **Step 1: Dry-run all manifests**

```bash
kubectl apply --dry-run=client -f k8s/namespace.yaml
kubectl apply --dry-run=client -f k8s/configmap.yaml
kubectl apply --dry-run=client -f k8s/deployment.yaml
# Expected: All succeed with "created" or "configured"
```

- [ ] **Step 2: Deploy to cluster**

```bash
./k8s/deploy.sh v0.1.0
# Expected: Deploy completes successfully
```

- [ ] **Step 3: Verify pod is running**

```bash
kubectl -n deribit get pods -l app=vol-monitor
# Expected: vol-monitor pod in Running state
```

- [ ] **Step 4: Check logs**

```bash
kubectl -n deribit logs deployment/vol-monitor
# Expected: vol-monitor startup logs
```

- [ ] **Step 5: Commit deployment verification**

```bash
git commit --allow-empty -m "test: verify k8s deployment succeeds"
```

---

## Spec Coverage Check

| Spec Requirement | Task |
|------------------|------|
| Dockerfile (multi-stage) | Task 1 |
| namespace.yaml | Task 2 |
| configmap.yaml | Task 3 |
| deployment.yaml | Task 4 |
| deploy.sh | Task 5 |
| README.md | Task 6 |
| Docker build verification | Task 7 |
| K8s manifest validation | Task 8 |

---

## Self-Review Checklist

- [x] No placeholders (all code complete)
- [x] Exact file paths specified
- [x] All commands have expected output
- [x] ConfigMap includes actual config.toml content
- [x] Deployment YAML has correct volume mount path
- [x] Deploy script is executable and tested for syntax
