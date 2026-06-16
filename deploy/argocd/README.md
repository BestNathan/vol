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

## Prerequisites

ArgoCD must already be installed in your cluster, and the `Application` CRD (`argoproj.io/v1alpha1`) must exist.

### Repository Access

The `root.yaml` manifest uses `git@github.com:BestNathan/vol.git` as its repository URL. ArgoCD must have SSH access to this repository, or you must change the `repoURL` field to an HTTPS URL configured in your ArgoCD instance.

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

## Verification

After bootstrapping, verify the ArgoCD applications and workloads:

```bash
kubectl -n argocd get applications
kubectl -n vol-agent-system get pods,svc
```

## Applications

The root App-of-Apps syncs two child applications:

| Application | Manifest path | Purpose |
|---|---|---|
| `runtime-config` | `deploy/argocd/manifests/runtime-config` | Namespace + shared runtime configuration |
| `workloads` | `deploy/argocd/manifests/workloads` | Application workload deployments |

### runtime-config

The `runtime-config` application owns:

| Resource | Description |
|---|---|
| `namespace.yaml` | `vol-agent-system` namespace |
| `agents-configmap.yaml` | Agent definitions from `.agents/agents/*.md` |
| `providers-configmap.yaml` | Provider definitions from `.agents/providers/*.toml` |
| `skills-configmap.yaml` | Skill definitions from `.agents/skills/<skill>/SKILL.md` |
| `provider-secrets.example.yaml` | Example secret for provider keys (excluded from sync) |

### workloads

The `workloads` application owns:

| Workload | Path |
|---|---|
| `agent-server` | `deploy/argocd/manifests/workloads/agent-server/` |
| `docs-rs-mcp` | `deploy/argocd/manifests/workloads/mcp/docs-rs-mcp/` |

## Runtime Config Mounts

The `agent-server` deployment mounts all three shared ConfigMaps into `/app/.agents`:

- `agent-definitions` → `/app/.agents/agents`
- `agent-providers` → `/app/.agents/providers`
- `agent-skills` → `/app/.agents/skills`

This keeps runtime configuration centralized and shared across workloads.

## Secrets

`deploy/argocd/manifests/runtime-config/provider-secrets.example.yaml` documents required keys for `agent-server`, but it is excluded from ArgoCD sync.

### Namespace Creation

The `vol-agent-system` namespace is managed by the `runtime-config` application, but secrets may need to be created before the first sync. Create the namespace manually if creating secrets before sync:

```bash
kubectl create namespace vol-agent-system --dry-run=client -o yaml | kubectl apply -f -
```

### Provider Secrets

Create the real provider secret in the cluster before syncing `agent-server`. **Real provider keys live in `agent-provider-secrets`:**

```bash
kubectl -n vol-agent-system create secret generic agent-provider-secrets \
  --from-literal=ANTHROPIC_AUTH_TOKEN='<token>' \
  --from-literal=OPENAI_API_KEY='<key>'
```

### ACR Image Pull Secret

Both `agent-server` and `docs-rs-mcp` use images from ACR and expect the image pull secret `acr-registry-secret` in `vol-agent-system` if the ACR repository requires authentication:

```bash
kubectl -n vol-agent-system create secret docker-registry acr-registry-secret \
  --docker-server='<acr-registry>' \
  --docker-username='<username>' \
  --docker-password='<password>'
```

## MCP Image Updates

The `.github/workflows/build-mcp-images.yml` workflow builds `docs-rs-mcp`, pushes it to ACR, and updates:

```text
deploy/argocd/manifests/workloads/mcp/docs-rs-mcp/deployment.yaml
```

The workflow uses immutable git short SHA tags. ArgoCD deploys the new image by syncing the committed manifest change.
