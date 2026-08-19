# Playwright MCP on K8s Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deploy `playwright-mcp` as a standalone HTTP MCP service in `vol-agent-system` and point the shared `.mcp.json` at it, replacing the broken stdio entry.

**Architecture:** Standalone Deployment + ClusterIP Service (official `mcr.microsoft.com/playwright/mcp` image, streamable HTTP on port 8931) follows the existing docs-rs-mcp / cli-tools-mcp pattern. The shared `.mcp.json` (synced to `mcp-config` ConfigMap via `scripts/sync-configmaps.py`) references it by internal URL. All workloads mounting `mcp-config` (dp / cp / dingtalk) gain playwright.

**Tech Stack:** Kubernetes manifests, ArgoCD GitOps (`deploy/argocd/manifests/`), `sync-configmaps.py`, MCP streamable HTTP.

**Spec:** `docs/superpowers/specs/2026-08-13-playwright-mcp-dp-design.md`

## Global Constraints

- Zero Rust code changes — manifests + `.mcp.json` only.
- Namespace: `vol-agent-system`. Port: 8931. Image: `mcr.microsoft.com/playwright/mcp:latest` (public registry — no `imagePullSecrets`).
- No `--proxy-server`, no `--allowed-hosts` in this iteration (add only if Task 3 verification fails / misuse observed).
- Follow docs-rs-mcp manifest conventions: labels `app.kubernetes.io/name|part-of|component`, `readOnlyRootFilesystem: true`, `allowPrivilegeEscalation: false`, `capabilities.drop: [ALL]`, tmp emptyDir.
- Do NOT force `runAsUser: 1000` on the playwright container — the image's default user (pwuser) owns `/ms-playwright`.
- `sync-configmaps.py` reads `.mcp.json` relative to CWD — always run from repo root.
- Commit messages per repo convention, ending with `Co-Authored-By: Claude <noreply@anthropic.com>`.

---

### Task 1: playwright-mcp Deployment + Service manifests

**Files:**
- Create: `deploy/argocd/manifests/workloads/mcp/playwright-mcp/deployment.yaml`
- Create: `deploy/argocd/manifests/workloads/mcp/playwright-mcp/service.yaml`

**Interfaces:**
- Consumes: nothing (self-contained; ArgoCD `workloads` app auto-syncs `manifests/workloads/` recursively — no app definition change needed).
- Produces: ClusterIP service `playwright-mcp.vol-agent-system.svc.cluster.local:8931`, streamable HTTP endpoint `/mcp` — consumed by Task 2's config entry and Task 3's smoke test.

- [ ] **Step 1: Create deployment.yaml**

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: playwright-mcp
  namespace: vol-agent-system
  labels:
    app.kubernetes.io/name: playwright-mcp
    app.kubernetes.io/part-of: vol-agent
    app.kubernetes.io/component: mcp
spec:
  replicas: 1
  selector:
    matchLabels:
      app.kubernetes.io/name: playwright-mcp
  strategy:
    type: RollingUpdate
    rollingUpdate:
      maxSurge: 1
      maxUnavailable: 0
  template:
    metadata:
      labels:
        app.kubernetes.io/name: playwright-mcp
        app.kubernetes.io/part-of: vol-agent
        app.kubernetes.io/component: mcp
    spec:
      restartPolicy: Always
      # No nodeSelector — mcr.microsoft.com/playwright/mcp is multi-arch (amd64/arm64)
      securityContext:
        runAsNonRoot: true
        # runAsUser omitted: image default user (pwuser) owns /ms-playwright browsers
      containers:
        - name: playwright-mcp
          image: mcr.microsoft.com/playwright/mcp:latest
          imagePullPolicy: Always
          args:
            - "--headless"
            - "--host"
            - "0.0.0.0"
            - "--port"
            - "8931"
            - "--no-sandbox"
          ports:
            - containerPort: 8931
              name: http
              protocol: TCP
          securityContext:
            readOnlyRootFilesystem: true
            allowPrivilegeEscalation: false
            capabilities:
              drop:
                - ALL
          readinessProbe:
            tcpSocket:
              port: 8931
            initialDelaySeconds: 10
            periodSeconds: 10
            timeoutSeconds: 3
          livenessProbe:
            tcpSocket:
              port: 8931
            initialDelaySeconds: 30
            periodSeconds: 30
            timeoutSeconds: 5
          env:
            # HOME → /tmp so node/chromium can write caches despite readOnlyRootFilesystem
            - name: HOME
              value: "/tmp"
            - name: HTTPS_PROXY
              value: "http://192.168.2.98:8890"
            - name: HTTP_PROXY
              value: "http://192.168.2.98:8890"
            - name: NO_PROXY
              value: "localhost,127.0.0.1,192.168.0.0/16,10.0.0.0/8,kubernetes.default.svc,.svc.cluster.local"
          resources:
            requests:
              cpu: 100m
              memory: 500Mi
            limits:
              cpu: "1"
              memory: 2Gi
          volumeMounts:
            - name: tmp
              mountPath: /tmp
            # Bigger /dev/shm — Chromium crashes with k8s default 64MB shm on complex pages
            - name: dshm
              mountPath: /dev/shm
      volumes:
        - name: tmp
          emptyDir: {}
        - name: dshm
          emptyDir:
            medium: Memory
            sizeLimit: 1Gi
```

- [ ] **Step 2: Create service.yaml**

```yaml
apiVersion: v1
kind: Service
metadata:
  name: playwright-mcp
  namespace: vol-agent-system
  labels:
    app.kubernetes.io/name: playwright-mcp
    app.kubernetes.io/part-of: vol-agent
    app.kubernetes.io/component: mcp
spec:
  selector:
    app.kubernetes.io/name: playwright-mcp
  ports:
    - name: http
      port: 8931
      targetPort: 8931
      protocol: TCP
  type: ClusterIP
```

- [ ] **Step 3: Validate YAML parses**

Run: `python3 -c "import yaml,glob; [yaml.safe_load(open(f)) for f in sorted(glob.glob('deploy/argocd/manifests/workloads/mcp/playwright-mcp/*.yaml'))]; print('YAML OK')"`
Expected: `YAML OK` (pyyaml is available — `sync-configmaps.py` depends on it).

- [ ] **Step 4: Client-side schema validation (if kubectl configured)**

Run: `kubectl apply --dry-run=client -f deploy/argocd/manifests/workloads/mcp/playwright-mcp/`
Expected: both resources listed, no schema errors. (Client dry-run does not contact the API server.)

- [ ] **Step 5: Commit**

```bash
git add deploy/argocd/manifests/workloads/mcp/playwright-mcp/
git commit -m "feat(k8s): add playwright-mcp deployment and service

Standalone MCP workload following the docs-rs-mcp pattern: official
multi-arch playwright/mcp image, streamable HTTP on :8931, hardened
(ro rootfs, non-root pwuser, dropped caps, 1Gi /dev/shm).

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 2: Shared .mcp.json — replace stdio entry with http URL

**Files:**
- Modify: `.mcp.json` (repo root)
- Modify: `deploy/argocd/manifests/runtime-config/mcp-configmap.yaml` (regenerated, not hand-edited)

**Interfaces:**
- Consumes: service DNS name from Task 1 (`playwright-mcp.vol-agent-system.svc.cluster.local:8931`).
- Produces: `mcp-config` ConfigMap with playwright as `"type": "http"` — consumed by agent-server workloads at startup (`vol-llm-mcp` parses http type as streamable HTTP; verified compatible with the docs-rs-mcp entry mechanism).

- [ ] **Step 1: Replace the playwright entry in .mcp.json**

The whole file becomes (the `钉钉文档` entry is not in the shared file — its permissioned URL lives only in the `agent-dingtalk-secrets` secret, mounted as `/app/.mcp.json` in the dingtalk deployment):

```json
{
  "mcpServers": {
    "docs-rs-mcp": {
      "type": "http",
      "url": "http://docs-rs-mcp.vol-agent-system.svc.cluster.local:8080/mcp"
    },
    "playwright": {
      "type": "http",
      "url": "http://playwright-mcp.vol-agent-system.svc.cluster.local:8931/mcp"
    }
  }
}
```

The old stdio entry (`"command": "npx", "args": ["@playwright/mcp@latest", "--headless"]`) must be gone — it fails at spawn in every agent-server pod (no node/npx in the Rust image).

- [ ] **Step 2: Regenerate the ConfigMap manifest**

Run from repo root: `python3 scripts/sync-configmaps.py`
Expected: stdout shows `mcp: .mcp.json` under a generated-manifests section.

- [ ] **Step 3: Verify the regenerated manifest**

Run: `grep -c "playwright-mcp.vol-agent-system" deploy/argocd/manifests/runtime-config/mcp-configmap.yaml && ! grep -q "npx" deploy/argocd/manifests/runtime-config/mcp-configmap.yaml && echo "CONFIGMAP OK"`
Expected: first grep prints `1`, and `CONFIGMAP OK`.

- [ ] **Step 4: Check for unexpected regen drift**

Run: `git diff --stat`
Expected: ONLY `deploy/argocd/manifests/runtime-config/mcp-configmap.yaml` and `.mcp.json` modified. If `sync-configmaps.py` regenerated any other configmap (agents-configmap.yaml, providers-configmap.yaml, …), there is pre-existing drift in the source configs — stop, report it, do NOT commit the unrelated regeneration in this task.

- [ ] **Step 5: Commit**

```bash
git add .mcp.json deploy/argocd/manifests/runtime-config/mcp-configmap.yaml
git commit -m "feat(mcp): point playwright at in-cluster HTTP server

Replace the stdio/npx entry (unrunnable in the Rust agent-server
image) with the playwright-mcp ClusterIP service URL.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 3: Deploy & verify on the cluster

**Files:**
- Modify: none (verification + contingency only; contingency edits `deploy/argocd/manifests/workloads/mcp/playwright-mcp/deployment.yaml`)

**Interfaces:**
- Consumes: Task 1 manifests, Task 2 configmap. Requires `kubectl` access to the cluster.
- Produces: running `playwright-mcp` workload; agents on dp/cp/dingtalk with `mcp__playwright__*` tools; recorded verification results for Task 4.

- [ ] **Step 1: Apply the workload manifests**

Run: `kubectl apply -f deploy/argocd/manifests/workloads/mcp/playwright-mcp/`
Expected: `deployment.apps/playwright-mcp created`, `service/playwright-mcp created`.
(ArgoCD `workloads` app self-heals afterward; applying manually gives immediate verification before merge.)

- [ ] **Step 2: Wait for readiness**

Run: `kubectl rollout status deployment/playwright-mcp -n vol-agent-system --timeout=180s`
Expected: `deployment "playwright-mcp" successfully rolled out`.

- [ ] **Step 3: Contingency — image pull failure only**

Run: `kubectl describe pod -n vol-agent-system -l app.kubernetes.io/name=playwright-mcp | grep -A5 Events`
If `ImagePullBackOff` / `ErrImagePull` on `mcr.microsoft.com`: the cluster cannot reach MCR (China network). Fallback (same design, different image field):
```bash
# On a machine that CAN pull MCR (e.g. via proxy):
docker pull mcr.microsoft.com/playwright/mcp:latest
docker tag mcr.microsoft.com/playwright/mcp:latest ghcr.io/bestnathan/playwright-mcp:latest
docker push ghcr.io/bestnathan/playwright-mcp:latest
```
Then edit `deployment.yaml`: `image: ghcr.io/bestnathan/playwright-mcp:latest`, add `imagePullSecrets: [{name: ghcr-bestnathan}]` under `spec.template.spec`, re-apply, re-run Step 2. Record which image was used for Task 4.

- [ ] **Step 4: Smoke test — MCP initialize over the service**

Run:
```bash
kubectl run mcp-smoke --rm -i --restart=Never --image=curlimages/curl -n vol-agent-system -- \
  curl -sS -i http://playwright-mcp.vol-agent-system.svc.cluster.local:8931/mcp \
  -H 'Content-Type: application/json' \
  -H 'Accept: application/json, text/event-stream' \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"smoke","version":"0.0.1"}}}'
```
Expected: HTTP 200 (or 202 for streamable HTTP negotiation); body contains `"protocolVersion"`. Body may be SSE-framed (`data: {...}` lines) — that is normal.

- [ ] **Step 5: Smoke test — tools/list returns browser tools**

Run:
```bash
kubectl run mcp-smoke2 --rm -i --restart=Never --image=curlimages/curl -n vol-agent-system -- \
  curl -sS http://playwright-mcp.vol-agent-system.svc.cluster.local:8931/mcp \
  -H 'Content-Type: application/json' \
  -H 'Accept: application/json, text/event-stream' \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}'
```
Expected: response contains `browser_navigate` (plus other `browser_*` tools).

- [ ] **Step 6: Apply the new mcp-config ConfigMap**

Run: `kubectl apply -f deploy/argocd/manifests/runtime-config/mcp-configmap.yaml`
Expected: `configmap/mcp-config configured`.

- [ ] **Step 7: Restart agent-server workloads (config is loaded once at startup)**

Run:
```bash
kubectl rollout restart deployment/agent-server-dp -n vol-agent-system
kubectl rollout restart deployment/agent-server -n vol-agent-system
kubectl rollout restart deployment/agent-server-dingtalk -n vol-agent-system
```
Expected: all three restarted. (`agent-server-ansible` uses `mcp-config-ansible`, a separate ConfigMap — it does NOT get playwright and needs no restart.)

- [ ] **Step 8: Verify agent-side MCP connection**

Run: `kubectl logs -n vol-agent-system deployment/agent-server-dp --tail=500 | grep -i playwright`
Expected: a JSON log line containing `"MCP server connected"` with `server` = `playwright` (source: `vol-llm-mcp/src/manager.rs:301`). MUST NOT see `"MCP server connection failed"` for playwright.

- [ ] **Step 9: Egress verification — agent browses an external site**

Start an agent session on a dp node through the usual UI (vol-llm-ui-cp) or CLI, prompt (Chinese UI wording):
`使用 playwright 工具打开 https://example.com 并告诉我页面标题`
Expected: session shows a successful `mcp__playwright__browser_navigate` call and the agent reports the title ("Example Domain").
- **If the browser tool fails with network/DNS errors** (external site unreachable): the cluster egress requires the proxy for browsers. Contingency: add `- "--proxy-server"` and `- "http://192.168.2.98:8890"` to `args` in `deployment.yaml`, `kubectl apply`, re-run this step. Record the outcome for Task 4.
- **If it succeeds**: done — no proxy needed, exactly as the spec's decision anticipated.

- [ ] **Step 10: Commit any contingency edits**

```bash
git add -A && git commit -m "fix(k8s): <image fallback | playwright browser proxy>"
```
(Skip this step if no contingency was needed.)

---

### Task 4: wiki-ingest the implementation result

**Files:**
- Create: `docs/wiki/sources/playwright-mcp-k8s-deployment.md`
- Create: `docs/wiki/entities/playwright-mcp-service.md` (frontmatter `category: infrastructure`)
- Modify: `docs/wiki/concepts/mcp-transport-pattern.md` (add the in-cluster deployment pattern + stdio pitfall below)
- Modify: `docs/wiki/index.md` (Entities/Sources sections + one-line summaries)
- Modify: `docs/wiki/log.md` (ingest entry)

**Interfaces:**
- Consumes: Task 3 verification results (image used, egress outcome, probe results).

- [ ] **Step 1: Run the wiki-ingest skill**

Invoke the `wiki-ingest` skill with source material: the Task 1 manifests, the `.mcp.json` diff, and the Task 3 verification results (which image is actually used; whether `--proxy-server` was needed; smoke/agent-log outcomes). Required content the resulting pages must cover:
- Source page `sources/playwright-mcp-k8s-deployment.md` — type `design`; TL;DR: playwright stdio/npx entry was unrunnable in the Rust agent-server image, replaced by a standalone Deployment+Service referenced via http URL in the shared mcp-config.
- Entity page `entities/playwright-mcp-service.md` — what it is, image, port 8931, streamable HTTP `/mcp`, hardening (ro rootfs, non-root pwuser, 1Gi /dev/shm, dropped caps), replicas 1, no `--allowed-hosts` / no `--proxy-server` (or record the contingency outcome if applied).
- Concept update `concepts/mcp-transport-pattern.md` — add: (1) stdio MCP servers require the runtime image to contain the command — the agent-server image (Rust-only Debian slim, ro rootfs) cannot run `npx`-based servers, so stdio entries fail at spawn; (2) the in-cluster pattern for third-party MCP servers = standalone Deployment + ClusterIP Service + `"type": "http"` URL in mcp.json, as done for docs-rs-mcp / cli-tools-mcp / playwright-mcp.
- Cross-link: `[[playwright-mcp-service]]` from the source page and concept page; add forward links from `[[mcp-client-integration]]` if it enumerates MCP servers.

- [ ] **Step 2: Increment source_count on updated pages**

Per the wiki-ingest skill, bump `source_count` in frontmatter of every updated entity/concept page.

- [ ] **Step 3: Commit**

```bash
git add docs/wiki/
git commit -m "docs(wiki): ingest playwright-mcp k8s deployment

Co-Authored-By: Claude <noreply@anthropic.com>"
```

- [ ] **Step 4: Upload to Lark (repo convention: docs/superpowers/* → Lark)**

The design spec `docs/superpowers/specs/2026-08-13-playwright-mcp-dp-design.md` is new — upload to the specs Lark node `Og7twpiPoi0Vbjk2EzvcqX92nsb`:
```bash
lark-cli docs +create --api-version v2 --doc-format markdown \
  --content @docs/superpowers/specs/2026-08-13-playwright-mcp-dp-design.md \
  --wiki-node "Og7twpiPoi0Vbjk2EzvcqX92nsb" --as user
```

---

## Post-plan checklist (executor)

- [ ] `kubectl get deploy playwright-mcp -n vol-agent-system` healthy, 1/1 ready
- [ ] `git status` clean; all commits pushed to main (ArgoCD then self-heals to identical state)
- [ ] Coverage gate N/A (no Rust code); `./scripts/check-no-doc-tests.sh` N/A (no Rust code)
- [ ] Wiki log.md has the ingest entry
