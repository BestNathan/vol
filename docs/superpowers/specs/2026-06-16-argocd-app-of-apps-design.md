# Design: ArgoCD App-of-Apps Deployment, Runtime Config, and MCP Image Workflow

## Background

The project currently keeps Kubernetes manifests under `k8s/` for manual or script-driven deployment. The GitOps path introduces a separate `deploy/argocd/` tree in the same repository. This tree is self-contained for ArgoCD and must not point back to, copy from, or depend on `k8s/` at sync time.

The deployment is single-environment and focuses on the agent runtime and MCP services, not the existing volatility monitor deployment.

The runtime has existing file-discovery conventions:

- Agents are Markdown files loaded from `{working_dir}/.agents/agents/*.md`.
- Providers are TOML files loaded from `{working_dir}/.agents/providers/*.toml`.
- Skills are directory-based definitions loaded from `{working_dir}/.agents/skills/<skill-name>/SKILL.md`.

Therefore, GitOps should distinguish between:

1. **Runtime config**: `.agents/agents`, `.agents/providers`, `.agents/skills` content shared by one or more `agent-server` deployments.
2. **Workloads**: Kubernetes Deployments/Services such as `agent-server` and `docs-rs-mcp`.

## Goals

1. Add an ArgoCD App-of-Apps deployment entrypoint under `deploy/argocd/`.
2. Use `vol-agent-system` as the target Kubernetes namespace for all GitOps-managed resources.
3. Keep `deploy/argocd/` independent from `k8s/` by placing complete Kubernetes manifests under the deploy tree.
4. Manage `.agents` runtime config as shared Kubernetes ConfigMaps/Secret examples:
   - `agents` = agent Markdown definitions with frontmatter.
   - `providers` = provider TOML files.
   - `skills` = skill definition directories containing `SKILL.md`.
5. Mount shared runtime config into `agent-server` at `/app/.agents`, so multiple `agent-server` workloads can reuse the same agents/providers/skills without duplication.
6. Include initial workload manifests for:
   - `agent-server`
   - `docs-rs-mcp`
7. Add a GitHub Actions workflow for building and pushing MCP service images to ACR.
8. Make the MCP image workflow update the GitOps manifest image tag so ArgoCD performs rollout through Git state.

## Non-Goals

1. Do not migrate or remove existing `k8s/` manifests.
2. Do not include `vol-monitor` in the new ArgoCD deployment tree.
3. Do not introduce Helm or Kustomize in the first version.
4. Do not implement sealed secrets, external secrets, or secret encryption in this change.
5. Do not make GitHub Actions run `kubectl apply` or otherwise deploy directly to the cluster.
6. Do not build a general dynamic MCP templating system in the first version; start with `docs-rs-mcp` as a concrete service.
7. Do not treat `agents` and `skills` as workload categories. In this design they are runtime configuration loaded by `agent-server`.

## Directory Layout

```text
deploy/
  argocd/
    README.md
    root.yaml
    applications/
      runtime-config.yaml
      workloads.yaml
    manifests/
      runtime-config/
        namespace.yaml
        agents-configmap.yaml
        providers-configmap.yaml
        skills-configmap.yaml
        provider-secrets.example.yaml
      workloads/
        agent-server/
          configmap.yaml
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

| Application | Source path | Purpose |
|---|---|---|
| `runtime-config` | `deploy/argocd/manifests/runtime-config` | Namespace, shared `.agents` runtime config, provider secret example |
| `workloads` | `deploy/argocd/manifests/workloads` | Agent server and MCP workload manifests |

Each child application should enable automated sync, prune, and self-heal. Both target `vol-agent-system`, but `runtime-config` owns the namespace manifest and shared config primitives.

## Runtime Config Design

### Namespace

`deploy/argocd/manifests/runtime-config/namespace.yaml` defines:

```yaml
apiVersion: v1
kind: Namespace
metadata:
  name: vol-agent-system
```

### Shared Agents ConfigMap

`deploy/argocd/manifests/runtime-config/agents-configmap.yaml` provides Markdown agent definitions. Each ConfigMap key maps to a file under `/app/.agents/agents/`.

Example key mapping:

```yaml
data:
  coding.md: |
    ---
    name: coding
    description: General coding agent
    model: qwen3.6-plus
    ---

    You are a coding agent for this project.
```

Mounted path in `agent-server`:

```text
/app/.agents/agents/coding.md
```

### Shared Providers ConfigMap

`deploy/argocd/manifests/runtime-config/providers-configmap.yaml` provides provider TOML files. Each ConfigMap key maps to a file under `/app/.agents/providers/`.

Example key mapping:

```yaml
data:
  anthropic-dashscope.toml: |
    provider = "anthropic"
    model = "qwen3.6-plus"
    api_key = "${ANTHROPIC_AUTH_TOKEN}"
    base_url = "http://192.168.2.162:31693"
```

Mounted path in `agent-server`:

```text
/app/.agents/providers/anthropic-dashscope.toml
```

### Shared Skills ConfigMap

`deploy/argocd/manifests/runtime-config/skills-configmap.yaml` provides skill definitions. Each ConfigMap key maps to a `SKILL.md` file under `/app/.agents/skills/<skill-name>/`.

Example key mapping:

```yaml
data:
  gitops/SKILL.md: |
    ---
    name: gitops
    description: Use when working with GitOps deployment manifests
    ---

    Follow the repository GitOps conventions.
```

Mounted path in `agent-server`:

```text
/app/.agents/skills/gitops/SKILL.md
```

### Provider Secrets

`deploy/argocd/manifests/runtime-config/provider-secrets.example.yaml` documents the provider API keys required by provider TOML files. It must be excluded from ArgoCD sync or left as an example-only manifest that is not applied with real placeholder values.

Real secrets should be created out-of-band or later replaced by an external secret solution. The expected Secret name is:

```text
agent-provider-secrets
```

`agent-server` workloads read provider API keys through environment variables sourced from that Secret.

## Workload Manifest Design

### agent-server

`deploy/argocd/manifests/workloads/agent-server/` contains only workload-specific resources:

- `configmap.yaml` for `agent-server.toml` server/runtime/control-plane config.
- `deployment.yaml` for the `agent-server` workload.
- `service.yaml` for the ClusterIP service.

Provider TOML files should not live in the agent-server workload ConfigMap. They belong in `runtime-config/providers-configmap.yaml` so multiple `agent-server` instances can reuse the same provider definitions.

The `agent-server` Deployment mounts:

```text
/etc/agent-server/agent-server.toml  # workload server config
/app/.agents/agents/*.md             # shared agent definitions
/app/.agents/providers/*.toml        # shared providers
/app/.agents/skills/*/SKILL.md       # shared skills
```

A second `agent-server` can be added later by creating another workload directory that mounts the same runtime-config ConfigMaps and Secret.

### docs-rs-mcp

`deploy/argocd/manifests/workloads/mcp/docs-rs-mcp/` contains concrete manifests, not shell templates:

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
4. Update the corresponding manifest under `deploy/argocd/manifests/workloads/mcp/<service>/deployment.yaml`.
5. Commit the manifest image tag update back to the repository.
6. Let ArgoCD perform the deployment from the committed manifest change.

### Initial Scope

The first version supports `docs-rs-mcp` only, but should use a structure that can be extended to additional MCP services.

Suggested service matrix:

```yaml
matrix:
  include:
    - service: docs-rs-mcp
      manifest: deploy/argocd/manifests/workloads/mcp/docs-rs-mcp/deployment.yaml
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

It should support selecting a specific MCP binary:

```dockerfile
ARG BIN=docs-rs-mcp
```

The Docker build should follow the project’s region-aware mirror pattern with `REGION=cn|global`.

### Image Tagging

Use immutable short SHA tags:

```text
${ACR_REGISTRY}/${ACR_NAMESPACE}/docs-rs-mcp:${SHORT_SHA}
```

Do not use `latest` as the manifest's deployment tag.

### Manifest Update

After push, update the image field in:

```text
deploy/argocd/manifests/workloads/mcp/docs-rs-mcp/deployment.yaml
```

The workflow should commit only if the manifest changes. Suggested commit message:

```text
ci(gitops): update docs-rs-mcp image to <short-sha> [skip ci]
```

The push must use `contents: write` permission and should avoid manifest-update races with a concurrency group and a rebase before push.

## Deployment Flow

```text
Operator applies root.yaml
  -> ArgoCD syncs runtime-config
     -> creates vol-agent-system
     -> creates shared .agents ConfigMaps
  -> ArgoCD syncs workloads
     -> agent-server mounts runtime config at /app/.agents
     -> docs-rs-mcp starts as MCP HTTP service

Developer merges MCP code to main
  -> GitHub Actions builds docs-rs-mcp image
  -> GitHub Actions pushes image to ACR with short SHA tag
  -> GitHub Actions updates deploy/argocd/manifests/workloads/mcp/docs-rs-mcp/deployment.yaml
  -> GitHub Actions commits manifest update
  -> ArgoCD observes Git change
  -> workloads Application syncs new manifest
  -> Kubernetes rolls out docs-rs-mcp in vol-agent-system
```

## Error Handling and Operational Notes

- If image build fails, no manifest update should be committed.
- If image push fails, no manifest update should be committed.
- If manifest update commit has no diff, the workflow should exit successfully without committing.
- If ArgoCD cannot sync due to missing provider secrets, `agent-server` should show degraded or pods should fail clearly; real secret management remains out of scope except example manifests.
- If ACR credentials are unavailable, the workflow should fail early at login.
- If runtime ConfigMaps grow beyond Kubernetes ConfigMap size limits, split by domain or by individual agent/skill in a follow-up design.

## Testing and Validation

Implementation should validate:

1. YAML parses successfully for all manifests and workflow files.
2. ArgoCD Application manifests point only under `deploy/argocd/`, not `k8s/`.
3. All GitOps-managed Kubernetes resources use namespace `vol-agent-system`.
4. Runtime ConfigMap items mount to the expected `.agents/agents`, `.agents/providers`, and `.agents/skills` paths.
5. `docs-rs-mcp` manifests contain a concrete image, not `${MCP_NAME}` placeholders.
6. The MCP workflow does not trigger on manifest-only changes.
7. The MCP workflow uses immutable SHA tags for deployment manifests.

## Open Questions

None for this refactor. Future work may add more agent definitions, more skills, multi-arch MCP builds, sealed secret integration, or a separate image update workflow for `agent-server`.
