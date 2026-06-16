# Design: ArgoCD App-of-Apps Deployment and MCP Image Workflow

## Background

The project currently keeps Kubernetes manifests under `k8s/` for manual or script-driven deployment. The desired GitOps path is to introduce a separate `deploy/argocd/` tree in the same repository. This tree should be self-contained for ArgoCD and should not point back to, copy from, or depend on `k8s/` at sync time.

The deployment is single-environment and focuses on agent and MCP services, not the existing volatility monitor deployment.

## Goals

1. Add an ArgoCD App-of-Apps deployment entrypoint under `deploy/argocd/`.
2. Use `vol-agent-system` as the target Kubernetes namespace for all GitOps-managed resources.
3. Keep `deploy/argocd/` independent from `k8s/` by placing complete Kubernetes manifests under the deploy tree.
4. Include initial GitOps applications for:
   - `agent-server`
   - `docs-rs-mcp`
5. Add a GitHub Actions workflow design for building and pushing MCP service images to ACR.
6. Make the MCP image workflow update the GitOps manifest image tag so ArgoCD performs rollout through Git state.

## Non-Goals

1. Do not migrate or remove existing `k8s/` manifests.
2. Do not include `vol-monitor` in the new ArgoCD deployment tree.
3. Do not introduce Helm or Kustomize in the first version.
4. Do not implement sealed secrets, external secrets, or secret encryption in this change.
5. Do not make GitHub Actions run `kubectl apply` or otherwise deploy directly to the cluster.
6. Do not build a general dynamic MCP templating system in the first version; start with `docs-rs-mcp` as a concrete service.

## Directory Layout

```text
deploy/
  argocd/
    README.md
    root.yaml
    applications/
      agent-server.yaml
      docs-rs-mcp.yaml
    manifests/
      namespace.yaml
      agent-server/
        configmap.yaml
        secret.example.yaml
        deployment.yaml
        service.yaml
      mcp/
        docs-rs-mcp/
          deployment.yaml
          service.yaml
```

## ArgoCD Structure

### Root Application

`deploy/argocd/root.yaml` is the only manifest that needs to be manually applied during bootstrap.

It should:

- Live in the `argocd` namespace.
- Use `spec.source.path: deploy/argocd/applications`.
- Use the current repository as `spec.source.repoURL`.
- Use `targetRevision: main` by default.
- Enable automated sync, prune, and self-heal.

### Child Applications

Each child application lives under `deploy/argocd/applications/` and points to a complete manifest directory under `deploy/argocd/manifests/`.

Initial applications:

| Application | Source path | Destination namespace |
|---|---|---|
| `agent-server` | `deploy/argocd/manifests/agent-server` | `vol-agent-system` |
| `docs-rs-mcp` | `deploy/argocd/manifests/mcp/docs-rs-mcp` | `vol-agent-system` |

Each child application should enable automated sync, prune, and self-heal. Namespace creation can be handled by `deploy/argocd/manifests/namespace.yaml`, and child applications should still target `vol-agent-system` explicitly.

## Kubernetes Manifest Design

### Namespace

`deploy/argocd/manifests/namespace.yaml` defines:

```yaml
apiVersion: v1
kind: Namespace
metadata:
  name: vol-agent-system
```

If the namespace manifest is not referenced by a child application directory, it should be included in the root-managed manifest layout or applied before child services. The preferred first implementation is to include namespace creation in the agent-server application path only if ArgoCD sync ordering is sufficient, or to add a dedicated `system` child application if ordering becomes necessary.

### Agent Server

`deploy/argocd/manifests/agent-server/` contains a complete service deployment:

- `configmap.yaml` for non-sensitive agent server configuration.
- `secret.example.yaml` documenting required secret keys without real values.
- `deployment.yaml` for the `agent-server` workload.
- `service.yaml` for the ClusterIP service.

The deployment should use the existing ACR registry convention and role-specific `vol-agent-server` image tags. A safe initial tag can be the current latest control-plane or combined role image already used by the project, but future automation may update this separately.

### docs-rs-mcp

`deploy/argocd/manifests/mcp/docs-rs-mcp/` contains concrete manifests, not shell templates:

- `deployment.yaml`
- `service.yaml`

The deployment should:

- Use namespace `vol-agent-system`.
- Expose HTTP on port `8080`.
- Run the MCP server with `--http 0.0.0.0:8080`.
- Include readiness and liveness probes on `/health` port `8080`.
- Use the ACR image path:

```text
crpi-ck06yio90i1ttwlz.cn-beijing.personal.cr.aliyuncs.com/n_common/docs-rs-mcp:<tag>
```

The first tag may be a known bootstrap tag, but the GitHub Actions workflow should later replace it with a git short SHA tag.

## MCP Image Build Workflow

Add `.github/workflows/build-mcp-images.yml`.

### Responsibilities

The workflow should:

1. Build one or more MCP service images.
2. Push images to ACR.
3. Tag images with immutable git short SHA tags.
4. Update the corresponding manifest under `deploy/argocd/manifests/mcp/<service>/deployment.yaml`.
5. Commit the manifest image tag update back to the repository.
6. Let ArgoCD perform the deployment from the committed manifest change.

### Initial Scope

The first version supports `docs-rs-mcp` only, but should use a structure that can be extended to additional MCP services.

Suggested service matrix:

```yaml
matrix:
  include:
    - service: docs-rs-mcp
      manifest: deploy/argocd/manifests/mcp/docs-rs-mcp/deployment.yaml
```

### Triggers

Use push triggers for source changes that affect MCP images, plus manual dispatch:

```yaml
on:
  push:
    branches: [main]
    paths:
      - "Cargo.toml"
      - "Cargo.lock"
      - "crates/vol-mcp-servers/**"
      - "dockers/vol-mcp-servers.Dockerfile"
      - ".cargo/config.toml"
      - ".github/workflows/build-mcp-images.yml"
  workflow_dispatch:
    inputs:
      service:
        description: "MCP service to build"
        required: false
        default: "docs-rs-mcp"
```

Manifest-only changes under `deploy/argocd/` should not trigger image rebuilds. This prevents infinite loops after the workflow commits a new image tag.

### Registry and Secrets

Use the same ACR convention as existing workflows:

```yaml
env:
  ACR_REGISTRY: ${{ vars.ACR_REGISTRY || 'crpi-ck06yio90i1ttwlz.cn-beijing.personal.cr.aliyuncs.com' }}
  ACR_NAMESPACE: ${{ vars.ACR_NAMESPACE || 'n_common' }}
```

Required secrets:

- `DOCKER_USERNAME`
- `DOCKER_PASSWORD`

### Dockerfile Requirement

The workflow expects an MCP Dockerfile at:

```text
dockers/vol-mcp-servers.Dockerfile
```

If that Dockerfile does not exist yet, implementation should add it. It should support selecting a specific MCP binary, for example:

```dockerfile
ARG BIN=docs-rs-mcp
```

The Docker build should copy `.cargo/config.toml` into the builder stage to keep the existing Rust mirror behavior.

### Image Tagging

Use immutable short SHA tags:

```text
${ACR_REGISTRY}/${ACR_NAMESPACE}/docs-rs-mcp:${SHORT_SHA}
```

Do not use `latest` as the manifest's deployment tag.

### Manifest Update

After push, update the image field in:

```text
deploy/argocd/manifests/mcp/docs-rs-mcp/deployment.yaml
```

The workflow should commit only if the manifest changes. Suggested commit message:

```text
ci(gitops): update docs-rs-mcp image to <short-sha> [skip ci]
```

The push must use `contents: write` permission.

## Deployment Flow

```text
Developer merges MCP code to main
  -> GitHub Actions builds docs-rs-mcp image
  -> GitHub Actions pushes image to ACR with short SHA tag
  -> GitHub Actions updates deploy/argocd/manifests/mcp/docs-rs-mcp/deployment.yaml
  -> GitHub Actions commits manifest update
  -> ArgoCD observes Git change
  -> docs-rs-mcp Application syncs new manifest
  -> Kubernetes rolls out docs-rs-mcp in vol-agent-system
```

## Error Handling and Operational Notes

- If image build fails, no manifest update should be committed.
- If image push fails, no manifest update should be committed.
- If manifest update commit has no diff, the workflow should exit successfully without committing.
- If ArgoCD cannot sync due to missing secrets, the application should show degraded or out-of-sync; secrets remain out of scope for this change except example manifests.
- If ACR credentials are unavailable, the workflow should fail early at login.

## Testing and Validation

Implementation should validate:

1. YAML parses successfully for all new manifests and workflow files.
2. ArgoCD Application manifests point only under `deploy/argocd/`, not `k8s/`.
3. All GitOps-managed Kubernetes resources use namespace `vol-agent-system`.
4. `docs-rs-mcp` manifests contain a concrete image, not `${MCP_NAME}` placeholders.
5. The MCP workflow does not trigger on manifest-only changes.
6. The MCP workflow uses immutable SHA tags for deployment manifests.

## Open Questions

None for the first implementation. Future work may add more MCP services, multi-arch MCP builds, sealed secret integration, or a separate image update workflow for `agent-server`.
