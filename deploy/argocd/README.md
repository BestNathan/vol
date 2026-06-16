# ArgoCD GitOps Deployment

This directory is the self-contained ArgoCD deployment entrypoint for vol agent services.

It does not reference `k8s/`. The existing `k8s/` directory remains available for manual or legacy deployment workflows.

## Scope

Managed here:

- `agent-server`
- `docs-rs-mcp`

Not managed here:

- `vol-monitor`
- legacy `k8s/` deployment scripts
- secret encryption or external secret operators

## Namespace

All GitOps-managed workloads target:

```text
vol-agent-system
```

## Bootstrap

Apply the root App-of-Apps once:

```bash
kubectl apply -f deploy/argocd/root.yaml
```

The root application syncs child applications from:

```text
deploy/argocd/applications/
```

The child applications sync complete Kubernetes manifests from:

```text
deploy/argocd/manifests/
```

## Applications

| Application | Manifest path |
|---|---|
| `agent-server` | `deploy/argocd/manifests/agent-server` |
| `docs-rs-mcp` | `deploy/argocd/manifests/mcp/docs-rs-mcp` |

## Secrets

`deploy/argocd/manifests/agent-server/secret.example.yaml` documents required keys for `agent-server`, but it is excluded from ArgoCD sync.

Create the real secret in the cluster before syncing `agent-server`:

```bash
kubectl -n vol-agent-system create secret generic agent-server-secrets \
  --from-literal=ANTHROPIC_AUTH_TOKEN='<token>' \
  --from-literal=OPENAI_API_KEY='<key>'
```

`docs-rs-mcp` expects the image pull secret `acr-registry-secret` in `vol-agent-system` if the ACR repository requires authentication.

## MCP Image Updates

The `.github/workflows/build-mcp-images.yml` workflow builds `docs-rs-mcp`, pushes it to ACR, and updates:

```text
deploy/argocd/manifests/mcp/docs-rs-mcp/deployment.yaml
```

The workflow uses immutable git short SHA tags. ArgoCD deploys the new image by syncing the committed manifest change.
