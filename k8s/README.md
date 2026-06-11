# Kubernetes Deployment

```
k8s/
├── namespace.yaml                        # Shared namespace
├── vol-monitor/
│   ├── deploy.sh                         # Build + push + deploy
│   ├── deployment.yaml                   # Standard deployment
│   ├── deployment-arm64-test.yaml        # ARM64 test deployment
│   ├── configmap.yaml                    # TOML config (no secrets)
│   └── secrets.yaml                      # Secret template (do not commit real secrets)
├── agent-server/
│   ├── deployment.yaml                   # vol-agent-server deployment
│   ├── configmap.yaml                    # Agent server TOML config
│   └── secret.yaml                       # API key secret template
└── mcp/
    ├── deploy.sh                         # Build + push + deploy MCP server
    └── deployment-template.yaml          # MCP server deployment template
```

## Vol Monitor

### Prerequisites

- Docker installed and logged in
- kubectl configured with cluster access

### Quick Start

1. **Update configuration:**
   - Edit `k8s/vol-monitor/deploy.sh` and set `DOCKERHUB_USERNAME`
   - Edit `k8s/vol-monitor/deployment.yaml` and update the image field

2. **Update ConfigMap:**
   - Edit `k8s/vol-monitor/configmap.yaml` with actual `config.toml` content

3. **Deploy:**
   ```bash
   ./k8s/vol-monitor/deploy.sh v0.1.0
   ```

### Manual Operations

```bash
# View status
kubectl -n deribit get pods -l app=vol-monitor

# View logs
kubectl -n deribit logs -f deployment/vol-monitor

# Rollback
kubectl -n deribit rollout undo deployment/vol-monitor
```

## Agent Server

```bash
# Deploy
kubectl apply -f k8s/namespace.yaml
kubectl apply -f k8s/agent-server/configmap.yaml
kubectl apply -f k8s/agent-server/secret.yaml
kubectl apply -f k8s/agent-server/deployment.yaml

# View status
kubectl -n deribit get pods -l app=vol-agent-server
kubectl -n deribit logs -f deployment/vol-agent-server
```

## MCP Server

```bash
./k8s/mcp/deploy.sh docs-rs-mcp v0.1.0
```

## Troubleshooting

### Pod not starting

```bash
kubectl -n deribit describe pod -l app=vol-monitor
kubectl -n deribit logs deployment/vol-monitor
```

### Image pull errors

```bash
kubectl -n deribit create secret docker-registry regcred \
  --docker-server=docker.io \
  --docker-username=<user> \
  --docker-password=<pass> \
  --docker-email=<email>
```
