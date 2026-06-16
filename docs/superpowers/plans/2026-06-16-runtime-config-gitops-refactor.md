# Runtime Config GitOps Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor the existing ArgoCD GitOps tree so `agents`, `providers`, and `skills` represent shared `.agents/` runtime configuration mounted by `agent-server`, while workload manifests live separately under `workloads`.

**Architecture:** Replace service-specific child Applications (`agent-server`, `docs-rs-mcp`) with two child Applications: `runtime-config` and `workloads`. `runtime-config` owns namespace plus ConfigMaps for `.agents/agents/*.md`, `.agents/providers/*.toml`, and `.agents/skills/*/SKILL.md`; `workloads` owns `agent-server` and `docs-rs-mcp` Deployments/Services. `agent-server` mounts shared runtime ConfigMaps into `/app/.agents` and references `agent-provider-secrets` for provider API keys.

**Tech Stack:** Kubernetes YAML, ArgoCD Application CRDs, ConfigMap projected volumes, Rust agent runtime `.agents` conventions, GitHub Actions.

---

## File Structure

Create:
- `deploy/argocd/applications/runtime-config.yaml` — ArgoCD child app for shared runtime config.
- `deploy/argocd/applications/workloads.yaml` — ArgoCD child app for workloads.
- `deploy/argocd/manifests/runtime-config/namespace.yaml` — `vol-agent-system` namespace.
- `deploy/argocd/manifests/runtime-config/agents-configmap.yaml` — agent Markdown definitions.
- `deploy/argocd/manifests/runtime-config/providers-configmap.yaml` — provider TOML definitions.
- `deploy/argocd/manifests/runtime-config/skills-configmap.yaml` — skill `SKILL.md` definitions.
- `deploy/argocd/manifests/runtime-config/provider-secrets.example.yaml` — provider secret example.
- `deploy/argocd/manifests/workloads/agent-server/configmap.yaml` — only `agent-server.toml`.
- `deploy/argocd/manifests/workloads/agent-server/deployment.yaml` — agent-server workload mounting `/app/.agents`.
- `deploy/argocd/manifests/workloads/agent-server/service.yaml` — agent-server service.
- `deploy/argocd/manifests/workloads/mcp/docs-rs-mcp/deployment.yaml` — moved MCP deployment.
- `deploy/argocd/manifests/workloads/mcp/docs-rs-mcp/service.yaml` — moved MCP service.

Remove:
- `deploy/argocd/applications/agent-server.yaml`
- `deploy/argocd/applications/docs-rs-mcp.yaml`
- `deploy/argocd/manifests/agent-server/**`
- `deploy/argocd/manifests/mcp/**`

Modify:
- `.github/workflows/build-mcp-images.yml` — update manifest path to `deploy/argocd/manifests/workloads/mcp/docs-rs-mcp/deployment.yaml`.
- `deploy/argocd/README.md` — document runtime-config/workloads split and shared `.agents` mounts.
- `docs/wiki/**` — ingest the refactor after validation.

---

### Task 1: Replace ArgoCD child Applications

**Files:**
- Create: `deploy/argocd/applications/runtime-config.yaml`
- Create: `deploy/argocd/applications/workloads.yaml`
- Delete: `deploy/argocd/applications/agent-server.yaml`
- Delete: `deploy/argocd/applications/docs-rs-mcp.yaml`

- [ ] **Step 1: Create runtime-config Application**

Create `deploy/argocd/applications/runtime-config.yaml`:

```yaml
apiVersion: argoproj.io/v1alpha1
kind: Application
metadata:
  name: runtime-config
  namespace: argocd
  labels:
    app.kubernetes.io/name: runtime-config
    app.kubernetes.io/part-of: vol-agent
spec:
  project: default
  source:
    repoURL: git@github.com:BestNathan/vol.git
    targetRevision: main
    path: deploy/argocd/manifests/runtime-config
    directory:
      recurse: true
      exclude: provider-secrets.example.yaml
  destination:
    server: https://kubernetes.default.svc
    namespace: vol-agent-system
  syncPolicy:
    automated:
      prune: true
      selfHeal: true
    syncOptions:
      - CreateNamespace=true
```

- [ ] **Step 2: Create workloads Application**

Create `deploy/argocd/applications/workloads.yaml`:

```yaml
apiVersion: argoproj.io/v1alpha1
kind: Application
metadata:
  name: workloads
  namespace: argocd
  labels:
    app.kubernetes.io/name: workloads
    app.kubernetes.io/part-of: vol-agent
spec:
  project: default
  source:
    repoURL: git@github.com:BestNathan/vol.git
    targetRevision: main
    path: deploy/argocd/manifests/workloads
    directory:
      recurse: true
  destination:
    server: https://kubernetes.default.svc
    namespace: vol-agent-system
  syncPolicy:
    automated:
      prune: true
      selfHeal: true
    syncOptions:
      - CreateNamespace=true
```

- [ ] **Step 3: Remove old service-specific Applications**

Run:

```bash
rm deploy/argocd/applications/agent-server.yaml deploy/argocd/applications/docs-rs-mcp.yaml
```

- [ ] **Step 4: Validate Application paths**

Run:

```bash
rtk grep -R "path: deploy/argocd/manifests" deploy/argocd/applications
rtk grep -R "agent-server.yaml\|docs-rs-mcp.yaml" deploy/argocd/applications || true
```

Expected: paths point only to `runtime-config` and `workloads`; no old app files remain.

- [ ] **Step 5: Commit**

```bash
git add deploy/argocd/applications
git commit -m "refactor(gitops): split runtime config and workloads apps" \
  -m "Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 2: Create shared runtime-config manifests

**Files:**
- Create: `deploy/argocd/manifests/runtime-config/namespace.yaml`
- Create: `deploy/argocd/manifests/runtime-config/agents-configmap.yaml`
- Create: `deploy/argocd/manifests/runtime-config/providers-configmap.yaml`
- Create: `deploy/argocd/manifests/runtime-config/skills-configmap.yaml`
- Create: `deploy/argocd/manifests/runtime-config/provider-secrets.example.yaml`

- [ ] **Step 1: Create namespace**

Create `namespace.yaml`:

```yaml
apiVersion: v1
kind: Namespace
metadata:
  name: vol-agent-system
  labels:
    app.kubernetes.io/name: vol-agent-system
    app.kubernetes.io/part-of: vol-agent
```

- [ ] **Step 2: Create agents ConfigMap**

Create `agents-configmap.yaml`:

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: agent-definitions
  namespace: vol-agent-system
  labels:
    app.kubernetes.io/name: agent-definitions
    app.kubernetes.io/part-of: vol-agent
    app.kubernetes.io/component: runtime-config
data:
  coding.md: |
    ---
    name: coding
    type: coding
    description: General coding agent for repository maintenance and implementation tasks
    model: qwen3.6-plus
    max_iterations: 20
    ---

    You are a coding agent for this repository. Follow the project conventions, keep changes focused, and verify behavior before reporting completion.
```

- [ ] **Step 3: Create providers ConfigMap**

Create `providers-configmap.yaml`:

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: agent-providers
  namespace: vol-agent-system
  labels:
    app.kubernetes.io/name: agent-providers
    app.kubernetes.io/part-of: vol-agent
    app.kubernetes.io/component: runtime-config
data:
  anthropic-dashscope.toml: |
    provider = "anthropic"
    model = "qwen3.6-plus"
    api_key = "${ANTHROPIC_AUTH_TOKEN}"
    base_url = "http://192.168.2.162:31693"

    [body]
    max_tokens = 8192
    temperature = 0.7

    [headers]
    "anthropic-version" = "2023-06-01"

  openai-example.toml: |
    provider = "openai"
    model = "glm5.1"
    api_key = "${OPENAI_API_KEY}"
    base_url = "http://k8s.nhome.local:31693"

    [body]
    max_tokens = 2048
    temperature = 0.7
```

- [ ] **Step 4: Create skills ConfigMap**

Create `skills-configmap.yaml`:

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: agent-skills
  namespace: vol-agent-system
  labels:
    app.kubernetes.io/name: agent-skills
    app.kubernetes.io/part-of: vol-agent
    app.kubernetes.io/component: runtime-config
data:
  gitops/SKILL.md: |
    ---
    name: gitops
    version: 1.0.0
    description: Use when working with this repository's ArgoCD GitOps manifests
    triggers:
      - gitops
      - argocd
      - kubernetes
    ---

    Keep `deploy/argocd/` self-contained and do not point ArgoCD Applications at `k8s/`. Runtime config belongs under `.agents/agents`, `.agents/providers`, and `.agents/skills`; workload manifests belong under `workloads`.
```

- [ ] **Step 5: Create provider secret example**

Create `provider-secrets.example.yaml`:

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: agent-provider-secrets
  namespace: vol-agent-system
  labels:
    app.kubernetes.io/name: agent-provider-secrets
    app.kubernetes.io/part-of: vol-agent
    app.kubernetes.io/component: runtime-config
type: Opaque
stringData:
  ANTHROPIC_AUTH_TOKEN: "sk-placeholder-replace-me"
  OPENAI_API_KEY: "placeholder-replace-me"
```

- [ ] **Step 6: Validate runtime-config YAML**

```bash
ruby -e 'require "yaml"; ARGV.each { |f| YAML.load_stream(File.read(f)); puts "ok #{f}" }' deploy/argocd/manifests/runtime-config/*.yaml
kubectl apply --dry-run=client -f deploy/argocd/manifests/runtime-config/namespace.yaml
kubectl apply --dry-run=client -f deploy/argocd/manifests/runtime-config/agents-configmap.yaml
kubectl apply --dry-run=client -f deploy/argocd/manifests/runtime-config/providers-configmap.yaml
kubectl apply --dry-run=client -f deploy/argocd/manifests/runtime-config/skills-configmap.yaml
kubectl apply --dry-run=client -f deploy/argocd/manifests/runtime-config/provider-secrets.example.yaml
```

Expected: all parse and dry-run successfully.

- [ ] **Step 7: Commit**

```bash
git add deploy/argocd/manifests/runtime-config
git commit -m "feat(gitops): add shared agent runtime config" \
  -m "Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 3: Move workload manifests and mount runtime config

**Files:**
- Create: `deploy/argocd/manifests/workloads/agent-server/configmap.yaml`
- Create: `deploy/argocd/manifests/workloads/agent-server/deployment.yaml`
- Create: `deploy/argocd/manifests/workloads/agent-server/service.yaml`
- Create: `deploy/argocd/manifests/workloads/mcp/docs-rs-mcp/deployment.yaml`
- Create: `deploy/argocd/manifests/workloads/mcp/docs-rs-mcp/service.yaml`
- Delete: `deploy/argocd/manifests/agent-server/**`
- Delete: `deploy/argocd/manifests/mcp/**`

- [ ] **Step 1: Create workload agent-server ConfigMap**

Create `deploy/argocd/manifests/workloads/agent-server/configmap.yaml` with only server config:

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: agent-server-config
  namespace: vol-agent-system
  labels:
    app.kubernetes.io/name: agent-server
    app.kubernetes.io/part-of: vol-agent
data:
  agent-server.toml: |
    [server]
    host = "0.0.0.0"
    port = 3001

    [server.roles]
    control_plane = true
    data_plane = false

    [control_plane]
    client_ws_path = "/ws"
    node_ws_path = "/control/v1/ws"
    lease_timeout_secs = 90
    lease_scan_secs = 15

    [runtime]
    working_dir = "/app"
    store_dir = "/app/data"

    [tracing]
    level = "info"
    format = "json"
```

- [ ] **Step 2: Create workload agent-server Deployment**

Create `deploy/argocd/manifests/workloads/agent-server/deployment.yaml`:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: agent-server
  namespace: vol-agent-system
  labels:
    app.kubernetes.io/name: agent-server
    app.kubernetes.io/part-of: vol-agent
spec:
  replicas: 1
  selector:
    matchLabels:
      app.kubernetes.io/name: agent-server
  strategy:
    type: RollingUpdate
    rollingUpdate:
      maxSurge: 1
      maxUnavailable: 0
  template:
    metadata:
      labels:
        app.kubernetes.io/name: agent-server
        app.kubernetes.io/part-of: vol-agent
    spec:
      restartPolicy: Always
      nodeSelector:
        kubernetes.io/arch: arm64
      imagePullSecrets:
        - name: acr-registry-secret
      containers:
        - name: agent-server
          image: crpi-ck06yio90i1ttwlz.cn-beijing.personal.cr.aliyuncs.com/n_common/vol-agent-server:cp-latest
          imagePullPolicy: Always
          workingDir: /app
          args:
            - "--config"
            - "/etc/agent-server/agent-server.toml"
          ports:
            - containerPort: 3001
              name: ws
              protocol: TCP
          volumeMounts:
            - name: config
              mountPath: /etc/agent-server
              readOnly: true
            - name: agent-definitions
              mountPath: /app/.agents/agents
              readOnly: true
            - name: agent-providers
              mountPath: /app/.agents/providers
              readOnly: true
            - name: agent-skills
              mountPath: /app/.agents/skills
              readOnly: true
          env:
            - name: ANTHROPIC_AUTH_TOKEN
              valueFrom:
                secretKeyRef:
                  name: agent-provider-secrets
                  key: ANTHROPIC_AUTH_TOKEN
            - name: OPENAI_API_KEY
              valueFrom:
                secretKeyRef:
                  name: agent-provider-secrets
                  key: OPENAI_API_KEY
            - name: HTTPS_PROXY
              value: "http://192.168.2.98:8890"
            - name: HTTP_PROXY
              value: "http://192.168.2.98:8890"
            - name: NO_PROXY
              value: "localhost,127.0.0.1,192.168.0.0/16,10.0.0.0/8,kubernetes.default.svc,.svc.cluster.local"
            - name: RUST_LOG
              value: "info"
      volumes:
        - name: config
          configMap:
            name: agent-server-config
            items:
              - key: agent-server.toml
                path: agent-server.toml
            defaultMode: 0644
        - name: agent-definitions
          configMap:
            name: agent-definitions
            items:
              - key: coding.md
                path: coding.md
            defaultMode: 0644
        - name: agent-providers
          configMap:
            name: agent-providers
            items:
              - key: anthropic-dashscope.toml
                path: anthropic-dashscope.toml
              - key: openai-example.toml
                path: openai-example.toml
            defaultMode: 0644
        - name: agent-skills
          configMap:
            name: agent-skills
            items:
              - key: gitops/SKILL.md
                path: gitops/SKILL.md
            defaultMode: 0644
```

- [ ] **Step 3: Move service and MCP manifests**

Copy current service and MCP manifests into new paths:

```bash
cp deploy/argocd/manifests/agent-server/service.yaml deploy/argocd/manifests/workloads/agent-server/service.yaml
mkdir -p deploy/argocd/manifests/workloads/mcp/docs-rs-mcp
cp deploy/argocd/manifests/mcp/docs-rs-mcp/deployment.yaml deploy/argocd/manifests/workloads/mcp/docs-rs-mcp/deployment.yaml
cp deploy/argocd/manifests/mcp/docs-rs-mcp/service.yaml deploy/argocd/manifests/workloads/mcp/docs-rs-mcp/service.yaml
```

- [ ] **Step 4: Remove old manifest paths**

```bash
rm -rf deploy/argocd/manifests/agent-server deploy/argocd/manifests/mcp
```

- [ ] **Step 5: Validate runtime mount paths and old paths are gone**

```bash
rtk grep -R "agent-server-secrets\|mountPath: /app/.agents/providers\|path: deploy/argocd/manifests/mcp\|path: deploy/argocd/manifests/agent-server" deploy/argocd || true
rtk grep -R "agent-provider-secrets\|mountPath: /app/.agents/agents\|mountPath: /app/.agents/providers\|mountPath: /app/.agents/skills" deploy/argocd/manifests/workloads/agent-server/deployment.yaml
```

Expected: first command has no matches; second command shows the new secret and mount paths.

- [ ] **Step 6: Validate Kubernetes YAML**

```bash
ruby -e 'require "yaml"; ARGV.each { |f| YAML.load_stream(File.read(f)); puts "ok #{f}" }' deploy/argocd/manifests/workloads/agent-server/*.yaml deploy/argocd/manifests/workloads/mcp/docs-rs-mcp/*.yaml
kubectl apply --dry-run=client -f deploy/argocd/manifests/workloads/agent-server/configmap.yaml
kubectl apply --dry-run=client -f deploy/argocd/manifests/workloads/agent-server/deployment.yaml
kubectl apply --dry-run=client -f deploy/argocd/manifests/workloads/agent-server/service.yaml
kubectl apply --dry-run=client -f deploy/argocd/manifests/workloads/mcp/docs-rs-mcp/deployment.yaml
kubectl apply --dry-run=client -f deploy/argocd/manifests/workloads/mcp/docs-rs-mcp/service.yaml
```

Expected: all parse and dry-run successfully.

- [ ] **Step 7: Commit**

```bash
git add deploy/argocd/manifests
git commit -m "refactor(gitops): mount shared runtime config into agent server" \
  -m "Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 4: Update MCP workflow manifest path

**Files:**
- Modify: `.github/workflows/build-mcp-images.yml`

- [ ] **Step 1: Update matrix manifest path**

Change:

```yaml
manifest: deploy/argocd/manifests/mcp/docs-rs-mcp/deployment.yaml
```

to:

```yaml
manifest: deploy/argocd/manifests/workloads/mcp/docs-rs-mcp/deployment.yaml
```

- [ ] **Step 2: Validate trigger loop safety**

```bash
ruby -e 'require "yaml"; YAML.load_stream(File.read(".github/workflows/build-mcp-images.yml")); puts "ok"'
rtk grep -n "deploy/argocd" .github/workflows/build-mcp-images.yml || true
rtk grep -n "concurrency:\|SERVICE:\|git pull --rebase origin main" .github/workflows/build-mcp-images.yml
```

Expected: YAML parses; `deploy/argocd` appears only in the matrix manifest path; hardening lines remain.

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/build-mcp-images.yml
git commit -m "ci(mcp): update gitops manifest path" \
  -m "Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 5: Update operator docs and wiki

**Files:**
- Modify: `deploy/argocd/README.md`
- Modify: `docs/wiki/**` via wiki-ingest

- [ ] **Step 1: Update README structure docs**

Update `deploy/argocd/README.md` so it documents:

```text
applications/runtime-config.yaml -> manifests/runtime-config
applications/workloads.yaml      -> manifests/workloads
```

and explains:

- `agents` means `.agents/agents/*.md` agent definitions.
- `providers` means `.agents/providers/*.toml` provider definitions.
- `skills` means `.agents/skills/<skill>/SKILL.md` skill definitions.
- `agent-server` mounts those ConfigMaps into `/app/.agents`.
- real provider keys are in `agent-provider-secrets`, not `agent-server-secrets`.

- [ ] **Step 2: Validate README mentions new terms**

```bash
rtk grep -n "runtime-config\|workloads\|agent-provider-secrets\|/app/.agents\|.agents/agents\|.agents/providers\|.agents/skills" deploy/argocd/README.md
```

Expected: matches for each required concept.

- [ ] **Step 3: Invoke wiki-ingest**

Use `wiki-ingest` with this summary:

```text
Ingest the GitOps runtime-config refactor: ArgoCD child Applications are now runtime-config and workloads; runtime-config owns namespace plus shared .agents agents/providers/skills ConfigMaps and provider secret example; agent-server mounts /app/.agents from shared ConfigMaps; workloads own agent-server and docs-rs-mcp manifests; MCP workflow manifest path moved under workloads.
```

- [ ] **Step 4: Commit docs/wiki updates**

```bash
git add deploy/argocd/README.md docs/wiki
git commit -m "docs(gitops): document runtime config layout" \
  -m "Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 6: Final validation and review

**Files:**
- Read-only validation.

- [ ] **Step 1: Run final boundary checks**

```bash
if rtk grep -R "path: k8s" deploy/argocd; then exit 1; else printf 'no path: k8s matches\n'; fi
if rtk grep -R "namespace: deribit\|namespace: mcp" deploy/argocd; then exit 1; else printf 'no legacy namespace matches\n'; fi
if rtk grep -R '\${MCP_NAME}' deploy/argocd; then exit 1; else printf 'no MCP_NAME placeholders\n'; fi
if rtk grep -R "agent-server-secrets" deploy/argocd; then exit 1; else printf 'no old agent-server secret refs\n'; fi
```

Expected: all checks print the no-match messages.

- [ ] **Step 2: Parse all YAML**

```bash
ruby -e 'require "yaml"; ARGV.each { |f| YAML.load_stream(File.read(f)); puts "ok #{f}" }' $(find deploy/argocd .github/workflows -name '*.yaml' -o -name '*.yml')
```

Expected: all YAML files parse.

- [ ] **Step 3: Kubectl dry-run all workload/runtime manifests**

```bash
kubectl apply --dry-run=client -f deploy/argocd/manifests/runtime-config/namespace.yaml
kubectl apply --dry-run=client -f deploy/argocd/manifests/runtime-config/agents-configmap.yaml
kubectl apply --dry-run=client -f deploy/argocd/manifests/runtime-config/providers-configmap.yaml
kubectl apply --dry-run=client -f deploy/argocd/manifests/runtime-config/skills-configmap.yaml
kubectl apply --dry-run=client -f deploy/argocd/manifests/runtime-config/provider-secrets.example.yaml
kubectl apply --dry-run=client -f deploy/argocd/manifests/workloads/agent-server/configmap.yaml
kubectl apply --dry-run=client -f deploy/argocd/manifests/workloads/agent-server/deployment.yaml
kubectl apply --dry-run=client -f deploy/argocd/manifests/workloads/agent-server/service.yaml
kubectl apply --dry-run=client -f deploy/argocd/manifests/workloads/mcp/docs-rs-mcp/deployment.yaml
kubectl apply --dry-run=client -f deploy/argocd/manifests/workloads/mcp/docs-rs-mcp/service.yaml
```

Expected: all dry-run successfully.

- [ ] **Step 4: Request final code review**

Dispatch a final reviewer over the refactor commit range and fix any blocking issues before completion.

## Self-Review

### Spec coverage

- Runtime config Applications and paths: Tasks 1-2.
- Shared `.agents/agents`, `.agents/providers`, `.agents/skills` ConfigMaps: Task 2.
- `agent-server` workload mounts `/app/.agents` and no longer contains provider TOML in its own ConfigMap: Task 3.
- MCP workflow path update: Task 4.
- Docs/wiki update: Task 5.
- Validation: Task 6.

### Placeholder scan

The plan contains no TBD/TODO/fill-later placeholders. The only placeholder values are intentional secret examples in `provider-secrets.example.yaml`.

### Type and path consistency

The plan consistently uses `runtime-config`, `workloads`, `agent-provider-secrets`, `agent-definitions`, `agent-providers`, and `agent-skills`. Old service-specific manifest paths are explicitly removed.
