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
