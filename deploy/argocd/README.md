# ArgoCD GitOps Deployment

This directory is the self-contained ArgoCD deployment entrypoint for vol agent services.

It does not reference `k8s/`. The existing `k8s/` directory remains available for manual or legacy deployment workflows.

## Scope

Managed here:

- `agent-server`
- `docs-rs-mcp`

Not managed here:

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

| Path | Contents |
|---|---|
| `base/namespace.yaml` | `vol-agent-system` namespace |
| `base/ghcr-image-pull-secret.yaml` | SealedSecret for GHCR image pulls |
| `base/ansible-ssh-key-sealed.yaml` | SealedSecret for SSH identity |
| `agents/agent-def-{name}.yaml` | One ConfigMap per agent definition |
| `sandboxes/sandbox-{name}.yaml` | One ConfigMap per sandbox definition |
| `providers/provider-{name}.yaml` | One ConfigMap per provider definition |
| `cli-tools/cli-tool-{name}.yaml` | One ConfigMap per CLI tool definition |
| `skills/skill-{name}.yaml` | One ConfigMap per skill definition |
| `mcp/mcp-configmap.yaml` | MCP server configuration from `.mcp.json` |
| `mcp/agent-dingtalk-secrets.example.yaml` | Example DingTalk MCP secret |
| `secrets/provider-secrets.example.yaml` | Example provider API key secret |

These ConfigMaps are **auto-generated** by `.github/workflows/sync-runtime-config.yml`. Any push to main that modifies source files under `.agents/` or `.mcp.json` triggers the workflow to regenerate the per-entity ConfigMap manifests, which ArgoCD then syncs.

### workloads

The `workloads` application owns:

| Workload | Path |
|---|---|
| `agent-server` | `deploy/argocd/manifests/workloads/agent-server/` |
| `docs-rs-mcp` | `deploy/argocd/manifests/workloads/mcp/docs-rs-mcp/` |

## Runtime Config Mounts

The `agent-server` deployment mounts per-entity ConfigMaps via projected volumes into `/app/.agents`:

- `agent-definitions` (projected from `agent-def-*`) → `/app/.agents/agents` (all agent `.md` files)
- `agent-providers` (projected from `provider-*`) → `/app/.agents/providers` (all provider `.toml` files)
- `agent-skills` (projected from `skill-*` with `items[].path`) → `/app/.agents/skills/{name}/SKILL.md`
- `agent-sandboxes` (projected from `sandbox-*`) → `/app/.agents/sandboxes` (all sandbox `.toml` files)
- `mcp-config` → `/app/.mcp.json` (subPath mount from `mcp.json`)

This keeps runtime configuration centralized and shared across workloads. New agents, providers, skills, or sandboxes added to the source directories are automatically reflected in the per-entity ConfigMaps via the sync workflow.

## ConfigMap Sync Workflow

`.github/workflows/sync-runtime-config.yml` auto-generates the per-entity ConfigMap manifests when source files change on main:

| Source | Generated ConfigMap(s) |
|--------|------------------------|
| `.agents/agents/{name}.md` | `agents/agent-def-{name}.yaml` |
| `.agents/providers/{name}.toml` | `providers/provider-{name}.yaml` |
| `.agents/skills/{name}/SKILL.md` | `skills/skill-{name}.yaml` |
| `.agents/sandboxes/{name}.toml` | `sandboxes/sandbox-{name}.yaml` |
| `.agents/cli-tools/{name}.toml` | `cli-tools/cli-tool-{name}.yaml` |
| `.mcp.json` | `mcp/mcp-configmap.yaml` |

The workflow also writes `.summary.json` listing all generated ConfigMap names per category.

**Adding a new agent/tool/sandbox/etc.:** After committing the source file, the CI syncs the new per-entity ConfigMap. Update the projected-volume `sources` list in the relevant workload deployment(s) to include the new ConfigMap name.

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

### GHCR Image Pull Secret

All workloads use images from GHCR (`ghcr.io/bestnathan/*`) and expect the image pull secret `ghcr-bestnathan` in `vol-agent-system`:

```bash
kubectl -n vol-agent-system create secret docker-registry ghcr-bestnathan \
  --docker-server='ghcr.io' \
  --docker-username='<github-username>' \
  --docker-password='<github-pat-with-read-packages-scope>'
```

## MCP Image Updates

The `.github/workflows/build-mcp-images.yml` workflow builds `docs-rs-mcp`, pushes it to GHCR, and updates:

```text
deploy/argocd/manifests/workloads/mcp/docs-rs-mcp/deployment.yaml
```

The workflow uses immutable git short SHA tags. ArgoCD deploys the new image by syncing the committed manifest change.
