---
type: entity
category: infrastructure
tags: [mcp, playwright, browser-automation, k8s, argocd]
created: 2026-08-13
updated: 2026-08-13
source_count: 1
---

# playwright-mcp Service

**Category:** In-cluster MCP service (infrastructure)
**Related:** [[mcp-transport-pattern]], [[vol-llm-mcp-crate]], [[argocd-app-of-apps-gitops]], [[mcp-client-integration]]
**Source:** [[playwright-mcp-k8s-deployment]]

## Overview

Standalone in-cluster MCP server exposing Playwright browser automation (24 `browser_*` tools, including `browser_navigate`) to the vol-agent MCP client. Runs the official `mcr.microsoft.com/playwright/mcp:latest` image (multi-arch amd64/arm64, bundles Chromium) as a Deployment + ClusterIP Service in `vol-agent-system`, referenced from the shared mcp-config via an `"type": "http"` streamable-HTTP URL — replacing the old stdio/`npx` entry that could never spawn in the Rust-only agent-server image.

## Key Facts

- **Image:** `mcr.microsoft.com/playwright/mcp:latest` (multi-arch, bundles browsers; no nodeSelector needed)
- **Port:** 8931 (HTTP), streamable HTTP `/mcp` endpoint
- **Service:** ClusterIP, port 8931 → targetPort 8931, selector `app.kubernetes.io/name: playwright-mcp`
- **Replicas:** 1 (RollingUpdate maxSurge 1 / maxUnavailable 0)
- **Args:** `--headless --host 0.0.0.0 --port 8931 --no-sandbox --allowed-hosts *` (entrypoint already carries `--headless --browser chromium --no-sandbox`; partial duplication harmless)
- **Hardening:** pod securityContext `runAsNonRoot: true` + `runAsUser: 1000`; container securityContext readOnlyRootFilesystem, `allowPrivilegeEscalation: false`, drop ALL capabilities; `HOME=/tmp` emptyDir; 1Gi memory-backed `/dev/shm` (Chromium crashes with k8s default 64MB shm on complex pages)
- **Env:** `HTTPS_PROXY`/`HTTP_PROXY` = `http://192.168.2.98:8890`, `NO_PROXY` for cluster ranges (cluster convention; Chromium browser traffic does NOT honor these env vars)
- **Resources:** requests 100m/500Mi, limits 1 CPU/2Gi
- **Probes:** readiness/liveness tcpSocket on 8931 (no HTTP path dependency)
- **Client config:** `.mcp.json` → `"playwright": {"type": "http", "url": "http://playwright-mcp.vol-agent-system.svc.cluster.local:8931/mcp"}` (regenerated into `mcp-configmap.yaml` via `scripts/sync-configmaps.py`; not hot-reloaded — agent-server rollout restart required)

## In-field fixes (beyond the original design spec)

1. **`runAsUser: 1000`** — image declares `USER node` (named user); kubelet cannot verify named users against `runAsNonRoot: true` (`CreateContainerConfigError`). 1000 is the image's node user uid; `/ms-playwright` owned by node.
2. **`--allowed-hosts *`** — playwright-core normalizes the bound `0.0.0.0:8931` to `localhost:8931` and allowlists only that Host header; in-cluster service-DNS clients got HTTP 403 without it.

## Verification

- Pod 1/1 Ready; rollout succeeded after the `runAsUser` fix
- MCP `initialize` via service DNS: HTTP 200 (protocolVersion 2025-06-18; serverInfo Playwright 1.63.0-alpha-2026-08-05)
- `tools/list` (session-aware, `mcp-session-id` header): 24 `browser_*` tools
- `agent-server-dp` log: `MCP server connected server="playwright"` (connected via `HTTPS with proxy http://192.168.2.98:8890`)

## Pending

- Egress test (external-site browse) PENDING user verification; `--proxy-server` contingency decision open
- `--allowed-hosts` whitelist to be introduced if misuse/overreach observed (per design)

## Timeline

- **2026-08-13**: Deployed via ArgoCD `workloads` app; verified in-cluster (initialize/tools/list/agent log) [[playwright-mcp-k8s-deployment]]
