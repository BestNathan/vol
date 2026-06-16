---
type: source
source_type: code
date: 2026-06-16
ingested: 2026-06-16
tags: [gitops, argocd, kubernetes, deployment, mcp, ci]
---

# ArgoCD GitOps Deployment

**Authors/Creators:** Nathan + Claude Code  
**Date:** 2026-06-16  
**Link:** `deploy/argocd/`, `.github/workflows/build-mcp-images.yml`, `dockers/vol-mcp-servers.Dockerfile`

## TL;DR

The repository gained a self-contained ArgoCD GitOps deployment tree under `deploy/argocd/` for `agent-server` and `docs-rs-mcp`. It uses an App-of-Apps root application, targets the `vol-agent-system` namespace, keeps legacy `k8s/` manifests independent, and adds an MCP image workflow that builds `docs-rs-mcp`, pushes to ACR, updates the GitOps deployment manifest with an immutable short-SHA tag, and lets ArgoCD roll out from Git.

## Key Takeaways

- `deploy/argocd/root.yaml` bootstraps an App-of-Apps that syncs child `Application` manifests from `deploy/argocd/applications/`.
- Child applications sync complete manifests under `deploy/argocd/manifests/`; they do not reference `k8s/`.
- Initial GitOps-managed workloads are `agent-server` and `docs-rs-mcp`, both targeting `vol-agent-system`.
- `agent-server` and `docs-rs-mcp` both use the ACR pull secret `acr-registry-secret` for private image pulls.
- `dockers/vol-mcp-servers.Dockerfile` builds a selected MCP binary using `ARG BIN=docs-rs-mcp` and `ARG REGION=cn|global`.
- `.github/workflows/build-mcp-images.yml` builds/pushes `docs-rs-mcp` for `linux/amd64`, updates `deploy/argocd/manifests/mcp/docs-rs-mcp/deployment.yaml`, rebases before pushing, and uses `[skip ci]` plus push path filters to avoid manifest-update loops.
- Validation passed for manifest location, no `k8s/` path references, no legacy namespaces, no `${MCP_NAME}` placeholders, YAML parsing, kubectl client dry-run, and workflow hardening checks.
- Local Docker build validation could not complete because Docker Hub token fetch for `debian:bookworm-slim` timed out; this was recorded as an external network issue rather than a manifest/workflow failure.

## Detailed Summary

The GitOps deployment structure is self-contained:

```text
deploy/argocd/
  root.yaml
  applications/
    agent-server.yaml
    docs-rs-mcp.yaml
  manifests/
    agent-server/
      namespace.yaml
      configmap.yaml
      secret.example.yaml
      deployment.yaml
      service.yaml
    mcp/docs-rs-mcp/
      deployment.yaml
      service.yaml
```

`root.yaml` is the one-time bootstrap object applied to the `argocd` namespace. It points ArgoCD at `deploy/argocd/applications/`, where the child applications define independent sync roots for `agent-server` and `docs-rs-mcp`. This preserves a hard boundary between GitOps manifests and the existing manual/scripted `k8s/` deployment tree.

The `agent-server` manifests define `vol-agent-system`, non-secret runtime/provider configuration, an excluded `secret.example.yaml`, a control-plane `vol-agent-server:cp-latest` deployment, and a ClusterIP service on port `3001`. The deployment mounts config and provider files from `agent-server-config`, references `agent-server-secrets` for LLM provider credentials, and uses `acr-registry-secret` for ACR image pulls.

The `docs-rs-mcp` manifests define a concrete deployment and service rather than using the legacy shell template. The deployment uses image `crpi-ck06yio90i1ttwlz.cn-beijing.personal.cr.aliyuncs.com/n_common/docs-rs-mcp:bootstrap` until CI updates it, runs `--http 0.0.0.0:8080`, exposes port `8080`, includes `/health` readiness/liveness probes, proxy environment variables, resource requests/limits, and the same ACR pull secret.

The MCP Dockerfile is a Debian slim multi-stage build for binaries in the `vol-mcp-servers` crate. It follows the project’s region-aware Docker pattern: `REGION=cn` uses Aliyun/rsproxy mirrors, while `REGION=global` uses upstream Debian/rustup for GitHub Actions. It builds `cargo build --release -p vol-mcp-servers --bin "${BIN}"`, strips the selected binary, and installs it as `/usr/local/bin/mcp-server` in the runtime image.

The MCP workflow builds only when MCP source, Dockerfile, Cargo metadata, `.cargo/config.toml`, or the workflow itself changes. It intentionally does not trigger on `deploy/argocd/**`, so workflow-generated manifest commits do not rebuild images. The workflow uses a top-level `concurrency` group, pushes to ACR with a short-SHA image tag, updates the matching service image field in the GitOps manifest using a `SERVICE` environment variable, commits with `[skip ci]`, and runs `git pull --rebase origin main` before pushing to reduce non-fast-forward failures.

`deploy/argocd/README.md` documents prerequisites, repository access for the SSH `repoURL`, namespace/secret ordering, real secret creation, ACR pull secret creation, bootstrap, verification commands, application paths, and MCP image-update behavior.

## Entities Mentioned

- [[vol-agent-server-crate]]: Deployed as the initial `agent-server` workload in the GitOps tree.
- [[vol-mcp-servers-crate]]: Provides the `docs-rs-mcp` binary and Docker image built by the new workflow.
- [[vol-repository]]: Contains the new `deploy/argocd/` GitOps deployment tree and MCP image workflow.

## Concepts Covered

- [[argocd-app-of-apps-gitops]]: Self-contained App-of-Apps deployment structure and CI-driven image tag update pattern.
- [[mcp-transport-pattern]]: `docs-rs-mcp` runs through HTTP transport with `--http 0.0.0.0:8080`.
- [[docs-rs-tools]]: The deployed `docs-rs-mcp` service exposes docs.rs/crates.io tools.

## Notes

- The legacy `k8s/` directory remains available for manual or older deployment workflows and is not referenced by ArgoCD Applications.
- Real secrets are intentionally not committed; `secret.example.yaml` is excluded from ArgoCD sync.
- The local Docker build validation was blocked by external Docker Hub network timeout while fetching `debian:bookworm-slim` metadata.
