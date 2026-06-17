---
type: concept
category: architecture
tags: [gitops, argocd, kubernetes, deployment, ci]
created: 2026-06-16
updated: 2026-06-16
source_count: 1
---

# ArgoCD App-of-Apps GitOps

## Definition

ArgoCD App-of-Apps GitOps deployment in this repository means Kubernetes rollout is driven from committed manifests under `deploy/argocd/`, with ArgoCD reconciling cluster state to Git. CI builds images and updates image tags in Git; CI does not directly run `kubectl apply` against the workload cluster.

## Key Points

- `deploy/argocd/` is self-contained and does not reference the legacy `k8s/` deployment tree.
- `deploy/argocd/root.yaml` implements App-of-Apps by syncing child `Application` manifests from `deploy/argocd/applications/`.
- Child applications are split into two sync roots: `runtime-config` and `workloads`.
- `runtime-config` owns the namespace plus shared ConfigMaps for agents, providers, and skills.
- `workloads` owns `agent-server` and `docs-rs-mcp` deployment manifests.
- `agent-server` mounts `/app/.agents` from shared ConfigMaps for centralized runtime configuration.
- Real provider keys are stored in `agent-provider-secrets`, not `agent-server-secrets`.
- Initial workloads are `agent-server` and `docs-rs-mcp`, both in the `vol-agent-system` namespace.
- `docs-rs-mcp` image updates are committed back into its deployment manifest by `.github/workflows/build-mcp-images.yml`.
- Push path filters intentionally exclude `deploy/argocd/**`, and manifest-update commits include `[skip ci]` to avoid rebuild loops.

## How It Works

```text
operator applies deploy/argocd/root.yaml
  -> ArgoCD syncs deploy/argocd/applications/
     -> runtime-config Application syncs deploy/argocd/manifests/runtime-config/
        - namespace: vol-agent-system
        - ConfigMaps: agents, providers, skills
     -> workloads Application syncs deploy/argocd/manifests/workloads/
        - agent-server deployment + service
        - docs-rs-mcp deployment + service

agent-server mounts:
  /app/.agents/agents   <- agents-configmap (.agents/agents/*.md)
  /app/.agents/providers <- providers-configmap (.agents/providers/*.toml)
  /app/.agents/skills   <- skills-configmap (.agents/skills/<skill>/SKILL.md)
  /etc/agent-server     <- agent-server-config

MCP code changes on main
  -> build-mcp-images workflow builds docs-rs-mcp
  -> workflow pushes short-SHA image to ACR
  -> workflow updates docs-rs-mcp deployment image in deploy/argocd/manifests/workloads/mcp/
  -> workflow commits and pushes the manifest update
  -> ArgoCD detects Git change and rolls out the new image
```

## Operational Constraints

- ArgoCD must already be installed and have the `Application` CRD before applying `root.yaml`.
- The root application uses `git@github.com:BestNathan/vol.git`; ArgoCD needs SSH access to that repository or the `repoURL` must be changed to an HTTPS URL configured in ArgoCD.
- Real runtime secrets are not committed. `provider-secrets.example.yaml` documents required keys and is excluded from sync.
- **Real provider API keys live in `agent-provider-secrets`, not `agent-server-secrets`**.
- `runtime-config` must sync before `workloads` to ensure the namespace and ConfigMaps exist.
- Private ACR pulls require `acr-registry-secret` in `vol-agent-system` for both `agent-server` and `docs-rs-mcp`.

## Examples

Bootstrap:

```bash
kubectl apply -f deploy/argocd/root.yaml
kubectl -n argocd get applications
kubectl -n vol-agent-system get pods,svc
```

Create namespace before pre-sync secrets:

```bash
kubectl create namespace vol-agent-system --dry-run=client -o yaml | kubectl apply -f -
```

## Related

- [[argocd-gitops-deployment]]
- [[vol-agent-server-crate]]
- [[vol-mcp-servers-crate]]
- [[mcp-transport-pattern]]
- [[docs-rs-tools]]
