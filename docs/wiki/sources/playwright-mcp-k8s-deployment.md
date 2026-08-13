---
type: source
source_type: design
date: 2026-08-13
ingested: 2026-08-13
tags: [playwright, mcp, k8s, argocd, deployment, browser-automation]
---

# Playwright MCP on K8s Deployment

**Authors/Creators:** BestNathan
**Date:** 2026-08-13
**Link:** deploy/argocd/manifests/workloads/mcp/playwright-mcp/, .mcp.json, deploy/argocd/manifests/runtime-config/mcp-configmap.yaml

## TL;DR

The `.mcp.json` playwright entry (stdio + `npx`) was unrunnable in any agent-server pod: the Rust-only agent-server image has no node/npx and a read-only root filesystem, so the stdio spawn always failed. It was replaced with a standalone `playwright-mcp` Deployment + ClusterIP Service using the official multi-arch image `mcr.microsoft.com/playwright/mcp:latest` (bundled browsers), referenced via an `"type": "http"` URL in the shared mcp-config — following the existing docs-rs-mcp / cli-tools-mcp in-cluster pattern. Verified in-cluster: initialize 200, 24 `browser_*` tools listed, agent log `MCP server connected server="playwright"`.

## Key Takeaways

- **Stdio pitfall:** stdio MCP servers require the runtime image to contain the command — the Rust-only agent-server image (Debian slim, ro rootfs) cannot run `npx`-based servers, so stdio entries fail at spawn.
- **In-cluster pattern:** third-party MCP servers are deployed as standalone Deployment + ClusterIP Service + `"type": "http"` URL in mcp.json (docs-rs-mcp / cli-tools-mcp / playwright-mcp).
- **Field fix 1 — `runAsUser: 1000`:** the image declares `USER node` (a *named* user); kubelet cannot verify named users against `runAsNonRoot: true` (`CreateContainerConfigError`). The pod-level securityContext forces `runAsUser: 1000` (= the image's node user; `/ms-playwright` is owned by node). The design's assumption that the image uses `pwuser` was wrong.
- **Field fix 2 — `--allowed-hosts *`:** playwright-core's `installHttpTransport` normalizes the bound `0.0.0.0:8931` to `localhost:8931` and allowlists only that Host header — in-cluster clients using service DNS got **HTTP 403 "Access is only allowed at localhost:8931"**. Added `--allowed-hosts *` (playwright-core `--allowed-hosts <hosts...>`) to disable the host check.
- **Image entrypoint** already carries `--headless --browser chromium --no-sandbox`; deployment args partially duplicate it — harmless.
- **Egress test still PENDING** user verification (browse an external site from the agent UI); the `--proxy-server` contingency decision is open. The deployment sets `HTTPS_PROXY`/`HTTP_PROXY`/`NO_PROXY` env vars (cluster convention) — Chromium browser traffic does not honor these env vars, which is why egress is verified separately.
- **Transient node issue (not a code issue):** k8s-worker1's containerd image-pull path was extremely slow/wedged for ~45+ min (all registries, `serializeImagePulls: true`), delaying the rollout; it recovered on its own. The brief's MCR-unreachable contingency was NOT applied (cluster reaches MCR fine).

## Detailed Summary

### Design (docs/superpowers/specs/2026-08-13-playwright-mcp-dp-design.md)

Replace the unrunnable stdio playwright entry with a standalone workload: Deployment (replicas 1, `mcr.microsoft.com/playwright/mcp:latest`, args `--headless --host 0.0.0.0 --port 8931 --no-sandbox`, hardening: ro rootfs, non-root, dropped caps, 1Gi `/dev/shm`, `HOME=/tmp` emptyDir) + ClusterIP Service port 8931 → targetPort 8931. Shared `.mcp.json` playwright entry becomes `{"type": "http", "url": "http://playwright-mcp.vol-agent-system.svc.cluster.local:8931/mcp"}`, regenerated into `mcp-configmap.yaml` by `scripts/sync-configmaps.py`. `vol-llm-mcp` config type `http` maps to `McpTransport::Http` (streamable HTTP), same mechanism as docs-rs-mcp. ConfigMap updates are not hot-reloaded — agent-server deployments need a rollout restart.

### Implementation facts (deviations from the design)

1. **`runAsUser: 1000`** (pod-level securityContext): the image's `USER node` is a named user; kubelet cannot verify it against `runAsNonRoot: true`. Also fixed the manifest comment (`pwuser` → `node`).
2. **`--allowed-hosts *`** (args): playwright-core allows only `Host: localhost:8931` when bound to `0.0.0.0:8931`; in-cluster service-DNS clients got HTTP 403. `--allowed-hosts *` disables the check (accepted risk per design decision; tighten later if misuse observed).

### Verification (post node-recovery re-run)

- `deployment.apps/playwright-mcp` 1/1 Ready (scheduled on `rock-5b-plus`; no nodeSelector — image is multi-arch amd64/arm64). Rollout succeeded after the `runAsUser` fix.
- MCP `initialize` over the service DNS returned HTTP 200 with `protocolVersion` 2025-06-18 and serverInfo Playwright 1.63.0-alpha-2026-08-05 (SSE framing normal for streamable HTTP).
- `tools/list` (session-aware: initialize first, then pass `mcp-session-id` header) returned 24 `browser_*` tools including `browser_navigate`. A literal one-shot `tools/list` without a session returns `Bad Request: Server not initialized` — expected streamable-HTTP behavior.
- `agent-server-dp` log (after configmap apply + rollout restart of exactly the three briefed deployments): `INFO vol_llm_mcp::manager: ... MCP server connected server="playwright"`; handshake via `HTTPS with proxy http://192.168.2.98:8890` (deployment proxy env), protocolVersion 2025-11-25 (rmcp client default) — both protocol versions accepted by the server.
- ArgoCD `runtime-config` app self-healed the configmap back to the stdio entry twice while the commits were unpushed (local-only), causing a transient `MCP server binary not found`; resolved once commits were pushed to origin/main and ArgoCD converged.

## Entities Mentioned

- [[playwright-mcp-service]]: new standalone in-cluster MCP server exposing Playwright browser automation on port 8931
- [[vol-llm-mcp-crate]]: client side — parses the `http` config and connects via streamable HTTP

## Concepts Covered

- [[mcp-transport-pattern]]: updated with the stdio-pitfall and the in-cluster Deployment+Service+http-URL pattern
- [[mcp-client-integration]]: `.mcp.json` http entries for in-cluster servers (docs-rs-mcp / cli-tools-mcp / playwright-mcp)

## Notes

- Egress test (external-site browse via `browser_navigate`) is PENDING user verification; `--proxy-server http://192.168.2.98:8890` contingency open.
- The brief's ghcr.io image fallback contingency was NOT applied — the cluster can reach MCR; the failure was a node-level pull wedge that self-recovered.
- k8s-worker1 pull slowness was transient (extreme slowness, not permanent failure); not a code issue.
- Smoke test used protocolVersion 2025-06-18, agent handshake 2025-11-25 — server negotiates both; no action needed.
