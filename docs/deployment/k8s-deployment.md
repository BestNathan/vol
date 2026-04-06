# Kubernetes Deployment Guide

## Cluster Architecture

- **Nodes**: 3-node cluster (k8s-master, k8s-worker1/amd64, rock-5b-plus/arm64)
- **Ingress**: Higress
- **Registry**: Aliyun Container Registry (ACR) private

## Resources

| Resource | Name | Namespace |
|----------|------|-----------|
| Namespace | `deribit` | - |
| Deployment | `vol-monitor` | `deribit` |
| ConfigMap | `vol-monitor-config` | `deribit` |
| Secret | `vol-monitor-secrets` | `deribit` |

## Prerequisites

### 1. Create Namespace

```bash
kubectl create namespace deribit
```

### 2. Create Secrets (Sensitive Credentials)

```bash
kubectl create secret generic vol-monitor-secrets \
  --from-literal=DERIBIT_CLIENT_ID=<actual-client-id> \
  --from-literal=DERIBIT_CLIENT_SECRET=<actual-client-secret> \
  --from-literal=FEISHU_APP_ID=<actual-app-id> \
  --from-literal=FEISHU_APP_SECRET=<actual-app-secret> \
  --from-literal=FEISHU_RECEIVE_ID=<actual-receive-id> \
  -n deribit
```

**Verify:**
```bash
kubectl get secret vol-monitor-secrets -n deribit
```

### 3. Create ConfigMap (Non-Sensitive Configuration)

```bash
kubectl apply -f k8s/configmap.yaml
```

**Update ConfigMap:**
```bash
# Delete and recreate
kubectl -n deribit delete configmap vol-monitor-config
kubectl -n deribit create configmap vol-monitor-config --from-file=config.toml=/root/nq-deribit/config.prod.toml

# Or apply updated manifest
kubectl apply -f k8s/configmap.yaml
```

## Deploy Application

### Option 1: Manual Deployment

```bash
kubectl apply -f k8s/deployment.yaml
```

### Option 2: Using Deploy Script

```bash
./k8s/deploy.sh latest
```

## Pod Spec Highlights

```yaml
spec:
  nodeSelector:
    kubernetes.io/arch: amd64  # Required: image is amd64 only
  containers:
  - name: vol-monitor
    image: crpi-ck06yio90i1ttwlz.cn-beijing.personal.cr.aliyuncs.com/n_common/vol-monitor:latest
    imagePullPolicy: Always
    workingDir: /etc/vol-monitor
    args:
      - "--config"
      - "config.toml"
    volumeMounts:
    - name: config
      mountPath: /etc/vol-monitor
      readOnly: true
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
    - name: FEISHU_APP_ID
      valueFrom:
        secretKeyRef:
          name: vol-monitor-secrets
          key: FEISHU_APP_ID
    - name: FEISHU_APP_SECRET
      valueFrom:
        secretKeyRef:
          name: vol-monitor-secrets
          key: FEISHU_APP_SECRET
    - name: FEISHU_RECEIVE_ID
      valueFrom:
        secretKeyRef:
          name: vol-monitor-secrets
          key: FEISHU_RECEIVE_ID
    - name: RUST_LOG
      value: "info"
    - name: HTTPS_PROXY
      value: "http://192.168.2.98:8890"
  volumes:
  - name: config
    configMap:
      name: vol-monitor-config
      items:
      - key: config.toml
        path: config.toml
      defaultMode: 0644
```

## Management Commands

### View Logs

```bash
# Stream logs
kubectl -n deribit logs -f deployment/vol-monitor

# View recent logs
kubectl -n deribit logs deployment/vol-monitor --tail=100
```

### View Status

```bash
# View pods
kubectl -n deribit get pods -l app=vol-monitor

# View deployment status
kubectl -n deribit get deployment vol-monitor

# Describe pod (for troubleshooting)
kubectl -n deribit describe pod -l app=vol-monitor
```

### Restart Deployment

```bash
kubectl -n deribit rollout restart deployment/vol-monitor
```

### Rollback

```bash
# Undo last rollout
kubectl -n deribit rollout undo deployment/vol-monitor

# View rollout history
kubectl -n deribit rollout history deployment/vol-monitor
```

### Update Secrets

```bash
# Update secret (then restart deployment)
kubectl create secret generic vol-monitor-secrets \
  --from-literal=DERIBIT_CLIENT_ID=<new-id> \
  --from-literal=DERIBIT_CLIENT_SECRET=<new-secret> \
  -n deribit --dry-run=client -o yaml | kubectl apply -f -

kubectl -n deribit rollout restart deployment/vol-monitor
```

## Troubleshooting

### Pod Not Starting

```bash
# Check events
kubectl -n deribit get events --sort-by='.lastTimestamp'

# Check pod description
kubectl -n deribit describe pod -l app=vol-monitor
```

### Configuration Issues

```bash
# Verify ConfigMap content
kubectl -n deribit get configmap vol-monitor-config -o yaml

# Verify Secret exists
kubectl -n deribit get secret vol-monitor-secrets
```

### CrashLoopBackOff

```bash
# Check previous container logs
kubectl -n deribit logs deployment/vol-monitor --previous
```
