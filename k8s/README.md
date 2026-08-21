# Kubernetes Deployment

```
k8s/
├── agent-server/
│   ├── deployment.yaml                   # vol-agent-server deployment
│   ├── configmap.yaml                    # Agent server TOML config
│   └── secret.yaml                       # API key secret template
└── mcp/
    ├── deploy.sh                         # Build + push + deploy MCP server
    └── deployment-template.yaml          # MCP server deployment template
```

## Agent Server

```bash
# Deploy
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

### Image pull errors

```bash
kubectl -n deribit create secret docker-registry regcred \
  --docker-server=docker.io \
  --docker-username=<user> \
  --docker-password=<pass> \
  --docker-email=<email>
```
