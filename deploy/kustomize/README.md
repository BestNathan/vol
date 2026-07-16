# Kustomize Deployment

Kustomize overlays for agent-server variants. Replaces the duplicated raw YAML
in `deploy/argocd/manifests/workloads/agent-server*/`.

## Structure

```
base/                          # Shared deployment + service template
├── deployment.yaml
├── service.yaml
└── kustomization.yaml
overlays/
├── control-plane/             # Control plane (cp-latest image)
│   ├── config/agent-server.toml
│   └── kustomization.yaml
├── data-plane/                # Data plane (dp-latest image, registers with CP)
│   ├── config/agent-server.toml
│   └── kustomization.yaml
└── dingtalk/                  # DingTalk-specific data plane
    ├── config/agent-server.toml
    └── kustomization.yaml
```

## Usage

### Preview rendered manifests

```bash
kubectl kustomize deploy/kustomize/overlays/control-plane
kubectl kustomize deploy/kustomize/overlays/data-plane
kubectl kustomize deploy/kustomize/overlays/dingtalk
```

### Apply

```bash
kubectl apply -k deploy/kustomize/overlays/control-plane
kubectl apply -k deploy/kustomize/overlays/data-plane
kubectl apply -k deploy/kustomize/overlays/dingtalk
```

### Integrate with ArgoCD

Point the ArgoCD Application at `deploy/kustomize/overlays/<name>` instead of
`deploy/argocd/manifests/workloads/agent-server*/`.

## What the base provides

| Resource | Description |
|----------|-------------|
| `volumes` | agent-definitions, agent-providers, agent-skills, agent-sandboxes, mcp-config, data (emptyDir) |
| `env` | OTEL, ANTHROPIC_AUTH_TOKEN, OPENAI_API_KEY, proxy, RUST_LOG |
| `securityContext` | non-root (1000:1000), readOnlyRootFilesystem, drop ALL capabilities |
| `resources` | sensible defaults (100m CPU / 128Mi memory requests) |
| `imagePullSecrets` | ghcr-bestnathan |

## What each overlay customizes

| Overlay | Image | Port | Resources | Special |
|---------|-------|------|-----------|---------|
| control-plane | `cp-latest` | 3001 | 512Mi limit | `control_plane=true` |
| data-plane | `dp-latest` | 3002 | 2Gi limit | `control_url`, readiness/liveness probes |
| dingtalk | `dp-latest` | 3001 | 512Mi limit | dingtalk-specific agents/providers ConfigMaps |
