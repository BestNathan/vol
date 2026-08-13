# Playwright MCP on K8s: Deployment & Agent Config

**Date:** 2026-08-13
**Status:** approved

## Overview

Make playwright MCP actually usable for agents in the k8s cluster. The `.mcp.json` already contains a playwright entry, but as stdio + `npx` it cannot work in any agent-server pod (Rust-only image, no node/npx, readOnlyRootFilesystem). Replace it with a standalone `playwright-mcp` Deployment + ClusterIP Service (official multi-arch image with bundled browsers), referenced via http URL in the shared `.mcp.json` — following the existing docs-rs-mcp / cli-tools-mcp pattern.

Zero Rust code changes. Pure k8s manifests + `.mcp.json` config.

## Decisions (confirmed with user)

| Topic | Decision |
|-------|----------|
| Scope | Shared `.mcp.json` — all workloads mounting `mcp-config` (dp / cp / dingtalk) get playwright |
| Browser egress | No `--proxy-server` initially; verify after deploy, add `--proxy-server http://192.168.2.98:8890` if external sites unreachable |
| Access restriction | No `--allowed-hosts` — unrestricted browsing; tighten later if misuse observed |
| Deployment form | Standalone Deployment + ClusterIP Service (rejected: bake into agent-server image — bloats Rust image, breaks readOnlyRootFilesystem hardening; rejected: pod sidecar — complex stdio-over-pipe plumbing) |

## Design

### 1. Workload: playwright-mcp

New directory `deploy/argocd/manifests/workloads/mcp/playwright-mcp/` — auto-synced by the ArgoCD `workloads` app (recursive, only `nginx-proxy/**` excluded).

`deployment.yaml`:

```yaml
image: mcr.microsoft.com/playwright/mcp:latest   # multi-arch amd64/arm64 (dp node is arm64), bundles browsers
args: ["--headless", "--host", "0.0.0.0", "--port", "8931", "--no-sandbox"]
readOnlyRootFilesystem: true                      # /tmp emptyDir for Chromium scratch
runAsNonRoot: true                                # image default user (pwuser); do NOT force runAsUser: 1000 — /ms-playwright owned by pwuser
capabilities: drop ALL
readinessProbe / livenessProbe: tcpSocket: 8931   # no HTTP path dependency
resources: requests 100m/500Mi, limits 1/2Gi
replicas: 1
env: HTTPS_PROXY / HTTP_PROXY / NO_PROXY identical to other MCP workloads (cluster convention; covers any outbound calls the MCP server process itself makes — Chromium browser traffic does NOT honor these env vars, which is why browser egress is verified separately in step 4)
```

Notes:
- `--no-sandbox` required: non-root Chromium under default k8s seccomp cannot bring up its own sandbox. Pod-level isolation (non-root, readOnlyRootFilesystem, dropped caps) still applies.
- Image pull risk: cluster is on a China network; `mcr.microsoft.com` may be unreachable. Fallbacks (do not change the design, only the image field): Docker Hub `mcp/playwright`, or pull + retag to `ghcr-bestnathan` with a digest pin.

`service.yaml`: ClusterIP, port 8931 → targetPort 8931, selector `app.kubernetes.io/name: playwright-mcp`, labels matching docs-rs-mcp convention (`app.kubernetes.io/component: mcp`).

### 2. Shared MCP config

`.mcp.json` (repo root, source of truth):

```json
"playwright": {
  "type": "http",
  "url": "http://playwright-mcp.vol-agent-system.svc.cluster.local:8931/mcp"
}
```

The old stdio entry (`npx @playwright/mcp@latest --headless`) is removed — it always fails at spawn in the agent-server pods and would surface as connection errors on every agent session.

Regenerate `deploy/argocd/manifests/runtime-config/mcp-configmap.yaml` via `python3 scripts/sync-configmaps.py`.

`vol-llm-mcp` transport compatibility: config type `http` maps to `McpTransport::Http` which speaks streamable HTTP — the `/mcp` endpoint served by `@playwright/mcp` (MCP SDK) is streamable HTTP. Same mechanism as the existing docs-rs-mcp entry.

### 3. Configuration flow & restart requirement

```
.mcp.json ──sync-configmaps.py──▶ mcp-configmap.yaml ──ArgoCD──▶ mcp-config ConfigMap
```

`vol-llm-mcp` loads MCP config once at agent-server startup — ConfigMap updates are NOT hot-reloaded. After the configmap syncs: `kubectl rollout restart deployment/agent-server-dp` (and agent-server / agent-server-dingtalk as appropriate).

### 4. Verification steps (post-deploy)

1. `kubectl get pods -l app.kubernetes.io/name=playwright-mcp` — Running, ready.
2. From the pod or any pod in-cluster: `curl http://playwright-mcp.vol-agent-system.svc.cluster.local:8931/mcp` with an MCP `initialize` request → expect `protocolVersion` in response.
3. Rollout restart dp, check agent-server-dp logs for successful playwright MCP connection.
4. Browser egress test: agent session opens an external site (e.g. example.com) via playwright tools. Success → done. Failure (network) → add `--proxy-server http://192.168.2.98:8890` to deployment args and retry.
5. Observe tool usage; if overreach into internal services is observed, introduce `--allowed-hosts` whitelist later.

### 5. Error handling

- MCP server down = soft failure: agent receives tool-call errors; agent-server itself unaffected.
- Single replica; crash → k8s restart; browser sessions (isolated contexts) are not persisted — acceptable.
- No `--allowed-hosts`: agent browsing is unrestricted; internal cluster services are reachable from the pod. Accepted risk per decision above.

## Out of scope

- `--proxy-server` (added only if verification step 4 fails)
- `--allowed-hosts` whitelist (added only if misuse observed)
- Headed mode / video / trace persistence
- k8s/mcp legacy deploy.sh (deprecated path)
