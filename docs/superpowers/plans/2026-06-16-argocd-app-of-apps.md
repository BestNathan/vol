# ArgoCD App-of-Apps Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a self-contained `deploy/argocd/` GitOps tree for `agent-server` and `docs-rs-mcp`, plus a GitHub Actions workflow that builds MCP images and updates the GitOps manifest image tag.

**Architecture:** `deploy/argocd/root.yaml` bootstraps an App-of-Apps that syncs child ArgoCD `Application` resources from `deploy/argocd/applications/`. Each child application points only to complete manifests under `deploy/argocd/manifests/`, never to `k8s/`. The MCP image workflow builds `docs-rs-mcp`, pushes it to ACR with a short-SHA tag, commits that tag into the MCP deployment manifest, and lets ArgoCD deploy from Git.

**Tech Stack:** Kubernetes YAML, ArgoCD `argoproj.io/v1alpha1` Application CRD, Docker multi-stage Rust builds, GitHub Actions, ACR, Rust/Cargo workspace.

---

## File Structure

Create these files:

- `deploy/argocd/README.md` — operator-facing bootstrap and workflow documentation.
- `deploy/argocd/root.yaml` — manually applied root ArgoCD Application.
- `deploy/argocd/applications/agent-server.yaml` — child Application for agent server manifests.
- `deploy/argocd/applications/docs-rs-mcp.yaml` — child Application for docs.rs MCP manifests.
- `deploy/argocd/manifests/agent-server/namespace.yaml` — namespace manifest included with the first agent sync.
- `deploy/argocd/manifests/agent-server/configmap.yaml` — non-secret agent server config.
- `deploy/argocd/manifests/agent-server/secret.example.yaml` — example secret manifest, intentionally not applied by ArgoCD.
- `deploy/argocd/manifests/agent-server/deployment.yaml` — agent server deployment.
- `deploy/argocd/manifests/agent-server/service.yaml` — agent server service.
- `deploy/argocd/manifests/mcp/docs-rs-mcp/deployment.yaml` — concrete docs.rs MCP deployment.
- `deploy/argocd/manifests/mcp/docs-rs-mcp/service.yaml` — docs.rs MCP service.
- `dockers/vol-mcp-servers.Dockerfile` — Dockerfile for building a selected MCP binary.
- `.github/workflows/build-mcp-images.yml` — workflow that builds/pushes MCP images and updates GitOps manifests.

Modify these files:

- `docs/wiki/` files via `wiki-ingest` after implementation is complete.

Do not modify these files:

- `k8s/**` — existing manual deployment manifests remain independent.
- Existing `build-vol-agent-server.yml` — agent-server image workflow is not part of this implementation.

---

### Task 1: Add ArgoCD Application Bootstrap Files

**Files:**
- Create: `deploy/argocd/root.yaml`
- Create: `deploy/argocd/applications/agent-server.yaml`
- Create: `deploy/argocd/applications/docs-rs-mcp.yaml`

- [ ] **Step 1: Create the root Application manifest**

Create `deploy/argocd/root.yaml` with exactly this content:

```yaml
apiVersion: argoproj.io/v1alpha1
kind: Application
metadata:
  name: vol-agent-root
  namespace: argocd
  labels:
    app.kubernetes.io/name: vol-agent-root
    app.kubernetes.io/part-of: vol-agent
spec:
  project: default
  source:
    repoURL: git@github.com:BestNathan/vol.git
    targetRevision: main
    path: deploy/argocd/applications
    directory:
      recurse: true
  destination:
    server: https://kubernetes.default.svc
    namespace: argocd
  syncPolicy:
    automated:
      prune: true
      selfHeal: true
    syncOptions:
      - CreateNamespace=false
```

- [ ] **Step 2: Create the agent-server child Application**

Create `deploy/argocd/applications/agent-server.yaml` with exactly this content:

```yaml
apiVersion: argoproj.io/v1alpha1
kind: Application
metadata:
  name: agent-server
  namespace: argocd
  labels:
    app.kubernetes.io/name: agent-server
    app.kubernetes.io/part-of: vol-agent
spec:
  project: default
  source:
    repoURL: git@github.com:BestNathan/vol.git
    targetRevision: main
    path: deploy/argocd/manifests/agent-server
    directory:
      recurse: true
      exclude: secret.example.yaml
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

- [ ] **Step 3: Create the docs-rs-mcp child Application**

Create `deploy/argocd/applications/docs-rs-mcp.yaml` with exactly this content:

```yaml
apiVersion: argoproj.io/v1alpha1
kind: Application
metadata:
  name: docs-rs-mcp
  namespace: argocd
  labels:
    app.kubernetes.io/name: docs-rs-mcp
    app.kubernetes.io/part-of: vol-agent
    app.kubernetes.io/component: mcp
spec:
  project: default
  source:
    repoURL: git@github.com:BestNathan/vol.git
    targetRevision: main
    path: deploy/argocd/manifests/mcp/docs-rs-mcp
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

- [ ] **Step 4: Validate no child Application points to `k8s/`**

Run:

```bash
rtk grep -R "path: k8s" deploy/argocd || true
```

Expected output: no matches.

- [ ] **Step 5: Commit bootstrap manifests**

Run:

```bash
git add deploy/argocd/root.yaml deploy/argocd/applications/agent-server.yaml deploy/argocd/applications/docs-rs-mcp.yaml
git commit -m "feat(gitops): add argocd app-of-apps bootstrap" \
  -m "Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 2: Add Agent Server GitOps Manifests

**Files:**
- Create: `deploy/argocd/manifests/agent-server/namespace.yaml`
- Create: `deploy/argocd/manifests/agent-server/configmap.yaml`
- Create: `deploy/argocd/manifests/agent-server/secret.example.yaml`
- Create: `deploy/argocd/manifests/agent-server/deployment.yaml`
- Create: `deploy/argocd/manifests/agent-server/service.yaml`

- [ ] **Step 1: Create namespace manifest**

Create `deploy/argocd/manifests/agent-server/namespace.yaml` with exactly this content:

```yaml
apiVersion: v1
kind: Namespace
metadata:
  name: vol-agent-system
  labels:
    app.kubernetes.io/name: vol-agent-system
    app.kubernetes.io/part-of: vol-agent
```

- [ ] **Step 2: Create agent-server ConfigMap**

Create `deploy/argocd/manifests/agent-server/configmap.yaml` with exactly this content:

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

- [ ] **Step 3: Create example secret manifest**

Create `deploy/argocd/manifests/agent-server/secret.example.yaml` with exactly this content:

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: agent-server-secrets
  namespace: vol-agent-system
  labels:
    app.kubernetes.io/name: agent-server
    app.kubernetes.io/part-of: vol-agent
type: Opaque
stringData:
  ANTHROPIC_AUTH_TOKEN: "sk-placeholder-replace-me"
  OPENAI_API_KEY: "placeholder-replace-me"
```

This file is excluded by `deploy/argocd/applications/agent-server.yaml` and documents the required real Secret. Do not put real credentials in it.

- [ ] **Step 4: Create agent-server Deployment**

Create `deploy/argocd/manifests/agent-server/deployment.yaml` with exactly this content:

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
            - name: providers
              mountPath: /app/.agents/providers
              readOnly: true
          env:
            - name: ANTHROPIC_AUTH_TOKEN
              valueFrom:
                secretKeyRef:
                  name: agent-server-secrets
                  key: ANTHROPIC_AUTH_TOKEN
            - name: OPENAI_API_KEY
              valueFrom:
                secretKeyRef:
                  name: agent-server-secrets
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
        - name: providers
          configMap:
            name: agent-server-config
            items:
              - key: anthropic-dashscope.toml
                path: anthropic-dashscope.toml
              - key: openai-example.toml
                path: openai-example.toml
            defaultMode: 0644
```

- [ ] **Step 5: Create agent-server Service**

Create `deploy/argocd/manifests/agent-server/service.yaml` with exactly this content:

```yaml
apiVersion: v1
kind: Service
metadata:
  name: agent-server
  namespace: vol-agent-system
  labels:
    app.kubernetes.io/name: agent-server
    app.kubernetes.io/part-of: vol-agent
spec:
  selector:
    app.kubernetes.io/name: agent-server
  ports:
    - name: ws
      port: 3001
      targetPort: 3001
      protocol: TCP
  type: ClusterIP
```

- [ ] **Step 6: Validate namespace consistency**

Run:

```bash
rtk grep -R "namespace: deribit\|namespace: mcp" deploy/argocd || true
```

Expected output: no matches.

- [ ] **Step 7: Commit agent-server manifests**

Run:

```bash
git add deploy/argocd/manifests/agent-server
git commit -m "feat(gitops): add agent-server manifests" \
  -m "Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 3: Add docs-rs-mcp GitOps Manifests

**Files:**
- Create: `deploy/argocd/manifests/mcp/docs-rs-mcp/deployment.yaml`
- Create: `deploy/argocd/manifests/mcp/docs-rs-mcp/service.yaml`

- [ ] **Step 1: Create docs-rs-mcp Deployment**

Create `deploy/argocd/manifests/mcp/docs-rs-mcp/deployment.yaml` with exactly this content:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: docs-rs-mcp
  namespace: vol-agent-system
  labels:
    app.kubernetes.io/name: docs-rs-mcp
    app.kubernetes.io/part-of: vol-agent
    app.kubernetes.io/component: mcp
spec:
  replicas: 1
  selector:
    matchLabels:
      app.kubernetes.io/name: docs-rs-mcp
  strategy:
    type: RollingUpdate
    rollingUpdate:
      maxSurge: 1
      maxUnavailable: 0
  template:
    metadata:
      labels:
        app.kubernetes.io/name: docs-rs-mcp
        app.kubernetes.io/part-of: vol-agent
        app.kubernetes.io/component: mcp
    spec:
      restartPolicy: Always
      nodeSelector:
        kubernetes.io/arch: amd64
      imagePullSecrets:
        - name: acr-registry-secret
      containers:
        - name: docs-rs-mcp
          image: crpi-ck06yio90i1ttwlz.cn-beijing.personal.cr.aliyuncs.com/n_common/docs-rs-mcp:bootstrap
          imagePullPolicy: Always
          args:
            - "--http"
            - "0.0.0.0:8080"
          ports:
            - containerPort: 8080
              name: http
              protocol: TCP
          readinessProbe:
            httpGet:
              path: /health
              port: 8080
            initialDelaySeconds: 5
            periodSeconds: 10
            timeoutSeconds: 3
          livenessProbe:
            httpGet:
              path: /health
              port: 8080
            initialDelaySeconds: 15
            periodSeconds: 30
            timeoutSeconds: 5
          env:
            - name: RUST_LOG
              value: "info"
            - name: HTTPS_PROXY
              value: "http://192.168.2.98:8890"
            - name: HTTP_PROXY
              value: "http://192.168.2.98:8890"
            - name: NO_PROXY
              value: "localhost,127.0.0.1,192.168.0.0/16,10.0.0.0/8,kubernetes.default.svc,.svc.cluster.local,docs.rs,crates.io"
          resources:
            requests:
              cpu: 100m
              memory: 128Mi
            limits:
              cpu: 500m
              memory: 256Mi
```

- [ ] **Step 2: Create docs-rs-mcp Service**

Create `deploy/argocd/manifests/mcp/docs-rs-mcp/service.yaml` with exactly this content:

```yaml
apiVersion: v1
kind: Service
metadata:
  name: docs-rs-mcp
  namespace: vol-agent-system
  labels:
    app.kubernetes.io/name: docs-rs-mcp
    app.kubernetes.io/part-of: vol-agent
    app.kubernetes.io/component: mcp
spec:
  selector:
    app.kubernetes.io/name: docs-rs-mcp
  ports:
    - name: http
      port: 8080
      targetPort: 8080
      protocol: TCP
  type: ClusterIP
```

- [ ] **Step 3: Verify no MCP template placeholders remain in GitOps manifests**

Run:

```bash
rtk grep -R '\${MCP_NAME}' deploy/argocd || true
```

Expected output: no matches.

- [ ] **Step 4: Commit docs-rs-mcp manifests**

Run:

```bash
git add deploy/argocd/manifests/mcp/docs-rs-mcp
git commit -m "feat(gitops): add docs-rs-mcp manifests" \
  -m "Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 4: Add MCP Dockerfile

**Files:**
- Create: `dockers/vol-mcp-servers.Dockerfile`

- [ ] **Step 1: Create MCP Dockerfile**

Create `dockers/vol-mcp-servers.Dockerfile` with exactly this content:

```dockerfile
# vol-mcp-servers Dockerfile (Debian slim runtime)
# =============================================================================
# Multi-stage build for binaries from the vol-mcp-servers crate.
#
# Build args:
#   BIN    — MCP binary to build and run (default: docs-rs-mcp)
#   REGION — cn (default) | global. cn uses aliyun apt mirror + rsproxy.cn
#            for rustup and crates.io. global uses Debian/rustup/crates.io
#            official sources for GitHub Actions runners.
#
# Build:
#   docker build --build-arg BIN=docs-rs-mcp --build-arg REGION=global \
#     -f dockers/vol-mcp-servers.Dockerfile -t docs-rs-mcp:local .
#
# Run:
#   docker run --rm -p 8080:8080 docs-rs-mcp:local --http 0.0.0.0:8080
# =============================================================================

FROM debian:bookworm-slim AS builder

ARG BIN=docs-rs-mcp
ARG REGION=cn

ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH

RUN set -eux; \
    if [ "$REGION" = "cn" ]; then \
        sed -i 's|deb.debian.org|mirrors.aliyun.com|g' /etc/apt/sources.list.d/debian.sources; \
    fi; \
    apt-get update; \
    apt-get install -y --no-install-recommends \
        curl gcc g++ make cmake perl libssl-dev pkg-config ca-certificates git; \
    rm -rf /var/lib/apt/lists/*; \
    if [ "$REGION" = "cn" ]; then \
        export RUSTUP_DIST_SERVER=https://rsproxy.cn; \
        export RUSTUP_UPDATE_ROOT=https://rsproxy.cn/rustup; \
        curl --proto '=https' --tlsv1.2 -sSf https://rsproxy.cn/rustup-init.sh \
            | sh -s -- -y --default-toolchain stable; \
        mkdir -p "$CARGO_HOME"; \
        printf '%s\n' \
            '[source.crates-io]' \
            'replace-with = "rsproxy-sparse"' \
            '[source.rsproxy-sparse]' \
            'registry = "sparse+https://rsproxy.cn/index/"' \
            '[net]' \
            'git-fetch-with-cli = true' \
            > "$CARGO_HOME/config.toml"; \
    else \
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
            | sh -s -- -y --default-toolchain stable; \
    fi; \
    cargo --version

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/

ENV CARGO_NET_RETRY=10 \
    CARGO_HTTP_TIMEOUT=120
RUN cargo build --release -p vol-mcp-servers --bin "${BIN}" && \
    strip "/app/target/release/${BIN}"

FROM debian:bookworm-slim

ARG BIN=docs-rs-mcp
ARG REGION=cn

RUN set -eux; \
    if [ "$REGION" = "cn" ]; then \
        sed -i 's|deb.debian.org|mirrors.aliyun.com|g' /etc/apt/sources.list.d/debian.sources; \
    fi; \
    apt-get update; \
    apt-get install -y --no-install-recommends ca-certificates; \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder "/app/target/release/${BIN}" /usr/local/bin/mcp-server

EXPOSE 8080

ENTRYPOINT ["/usr/local/bin/mcp-server"]
```

- [ ] **Step 2: Build the docs-rs-mcp image locally without pushing**

Run:

```bash
docker build \
  --build-arg BIN=docs-rs-mcp \
  --build-arg REGION=global \
  -f dockers/vol-mcp-servers.Dockerfile \
  -t docs-rs-mcp:plan-check .
```

Expected: Docker build exits 0 and produces `docs-rs-mcp:plan-check`.

- [ ] **Step 3: Commit the MCP Dockerfile**

Run:

```bash
git add dockers/vol-mcp-servers.Dockerfile
git commit -m "feat(docker): add mcp server image build" \
  -m "Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 5: Add MCP Image Build Workflow

**Files:**
- Create: `.github/workflows/build-mcp-images.yml`

- [ ] **Step 1: Create workflow file**

Create `.github/workflows/build-mcp-images.yml` with exactly this content:

```yaml
# Build and push MCP server images to ACR, then update GitOps manifests.
#
# Initial scope: docs-rs-mcp on linux/amd64.
#
# Required secrets:
#   DOCKER_USERNAME — ACR login user
#   DOCKER_PASSWORD — ACR login password
#
# Optional repo/org variables:
#   vars.ACR_REGISTRY  (default: crpi-ck06yio90i1ttwlz.cn-beijing.personal.cr.aliyuncs.com)
#   vars.ACR_NAMESPACE (default: n_common)

name: build-mcp-images

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
        type: choice
        options:
          - docs-rs-mcp
      push_image:
        description: "Push image to ACR and update GitOps manifest"
        required: false
        default: "true"
        type: choice
        options: ["true", "false"]

env:
  ACR_REGISTRY: ${{ vars.ACR_REGISTRY || 'crpi-ck06yio90i1ttwlz.cn-beijing.personal.cr.aliyuncs.com' }}
  ACR_NAMESPACE: ${{ vars.ACR_NAMESPACE || 'n_common' }}

jobs:
  build:
    name: build (${{ matrix.service }})
    runs-on: ubuntu-24.04
    permissions:
      contents: write
    strategy:
      fail-fast: false
      matrix:
        include:
          - service: docs-rs-mcp
            manifest: deploy/argocd/manifests/mcp/docs-rs-mcp/deployment.yaml

    steps:
      - name: Checkout
        uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Skip non-selected manual services
        if: github.event_name == 'workflow_dispatch' && github.event.inputs.service != matrix.service
        run: echo "Skipping ${{ matrix.service }} because workflow_dispatch selected ${{ github.event.inputs.service }}"

      - name: Stop skipped service
        if: github.event_name == 'workflow_dispatch' && github.event.inputs.service != matrix.service
        run: exit 0

      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v3

      - name: Decide whether to push
        id: push
        run: |
          if [ "${{ github.event_name }}" = "workflow_dispatch" ]; then
            echo "value=${{ github.event.inputs.push_image }}" >> "$GITHUB_OUTPUT"
          else
            echo "value=true" >> "$GITHUB_OUTPUT"
          fi

      - name: Compute image tag
        id: image
        run: |
          short="$(echo "${GITHUB_SHA}" | cut -c1-7)"
          image="${ACR_REGISTRY}/${ACR_NAMESPACE}/${{ matrix.service }}:${short}"
          {
            echo "short=${short}"
            echo "image=${image}"
          } >> "$GITHUB_OUTPUT"

      - name: Log in to ACR
        if: steps.push.outputs.value == 'true'
        uses: docker/login-action@v3
        with:
          registry: ${{ env.ACR_REGISTRY }}
          username: ${{ secrets.DOCKER_USERNAME }}
          password: ${{ secrets.DOCKER_PASSWORD }}

      - name: Build and optionally push image
        uses: docker/build-push-action@v6
        with:
          context: .
          file: dockers/vol-mcp-servers.Dockerfile
          platforms: linux/amd64
          build-args: |
            BIN=${{ matrix.service }}
            REGION=global
          tags: ${{ steps.image.outputs.image }}
          push: ${{ steps.push.outputs.value == 'true' }}
          provenance: false
          cache-from: type=gha,scope=mcp-${{ matrix.service }}-amd64
          cache-to: type=gha,mode=max,scope=mcp-${{ matrix.service }}-amd64

      - name: Update GitOps manifest image
        if: steps.push.outputs.value == 'true'
        env:
          MANIFEST: ${{ matrix.manifest }}
          IMAGE: ${{ steps.image.outputs.image }}
        run: |
          python3 - <<'PY'
          import os
          from pathlib import Path

          manifest = Path(os.environ["MANIFEST"])
          image = os.environ["IMAGE"]
          lines = manifest.read_text().splitlines()
          updated = []
          replaced = False
          for line in lines:
              stripped = line.strip()
              if stripped.startswith("image: ") and "docs-rs-mcp" in stripped:
                  indent = line[: len(line) - len(line.lstrip())]
                  updated.append(f"{indent}image: {image}")
                  replaced = True
              else:
                  updated.append(line)
          if not replaced:
              raise SystemExit(f"no docs-rs-mcp image field found in {manifest}")
          manifest.write_text("\n".join(updated) + "\n")
          PY

      - name: Commit GitOps manifest update
        if: steps.push.outputs.value == 'true'
        run: |
          if git diff --quiet -- "${{ matrix.manifest }}"; then
            echo "No GitOps manifest change to commit."
            exit 0
          fi
          git config user.name "github-actions[bot]"
          git config user.email "41898282+github-actions[bot]@users.noreply.github.com"
          git add "${{ matrix.manifest }}"
          git commit -m "ci(gitops): update ${{ matrix.service }} image to ${{ steps.image.outputs.short }} [skip ci]"
          git push

      - name: Summary
        run: |
          {
            echo "### ${{ matrix.service }}"
            echo
            echo "- image: \`${{ steps.image.outputs.image }}\`"
            echo "- manifest: \`${{ matrix.manifest }}\`"
            echo "- pushed: \`${{ steps.push.outputs.value }}\`"
          } >> "$GITHUB_STEP_SUMMARY"
```

- [ ] **Step 2: Fix manual-service skip if adding more matrix entries later**

No code change is needed for the first implementation because the matrix contains only `docs-rs-mcp`, which is also the only valid `workflow_dispatch` choice. If another service is added later, replace the two skip steps with a job-level matrix filter or split service selection before matrix expansion.

- [ ] **Step 3: Validate workflow does not trigger on manifest-only changes**

Run:

```bash
rtk grep -n "deploy/argocd" .github/workflows/build-mcp-images.yml || true
```

Expected output: only the `manifest: deploy/argocd/manifests/mcp/docs-rs-mcp/deployment.yaml` matrix value. There should be no `deploy/argocd/**` under `on.push.paths`.

- [ ] **Step 4: Commit workflow**

Run:

```bash
git add .github/workflows/build-mcp-images.yml
git commit -m "ci(mcp): build and publish mcp images" \
  -m "Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 6: Add ArgoCD Deployment Documentation

**Files:**
- Create: `deploy/argocd/README.md`

- [ ] **Step 1: Create README**

Create `deploy/argocd/README.md` with exactly this content:

```markdown
# ArgoCD GitOps Deployment

This directory is the self-contained ArgoCD deployment entrypoint for vol agent services.

It does not reference `k8s/`. The existing `k8s/` directory remains available for manual or legacy deployment workflows.

## Scope

Managed here:

- `agent-server`
- `docs-rs-mcp`

Not managed here:

- `vol-monitor`
- legacy `k8s/` deployment scripts
- secret encryption or external secret operators

## Namespace

All GitOps-managed workloads target:

```text
vol-agent-system
```

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

## Applications

| Application | Manifest path |
|---|---|
| `agent-server` | `deploy/argocd/manifests/agent-server` |
| `docs-rs-mcp` | `deploy/argocd/manifests/mcp/docs-rs-mcp` |

## Secrets

`deploy/argocd/manifests/agent-server/secret.example.yaml` documents required keys for `agent-server`, but it is excluded from ArgoCD sync.

Create the real secret in the cluster before syncing `agent-server`:

```bash
kubectl -n vol-agent-system create secret generic agent-server-secrets \
  --from-literal=ANTHROPIC_AUTH_TOKEN='<token>' \
  --from-literal=OPENAI_API_KEY='<key>'
```

`docs-rs-mcp` expects the image pull secret `acr-registry-secret` in `vol-agent-system` if the ACR repository requires authentication.

## MCP Image Updates

The `.github/workflows/build-mcp-images.yml` workflow builds `docs-rs-mcp`, pushes it to ACR, and updates:

```text
deploy/argocd/manifests/mcp/docs-rs-mcp/deployment.yaml
```

The workflow uses immutable git short SHA tags. ArgoCD deploys the new image by syncing the committed manifest change.
```

- [ ] **Step 2: Commit README**

Run:

```bash
git add deploy/argocd/README.md
git commit -m "docs(gitops): document argocd deployment" \
  -m "Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 7: Validate Manifests, Workflow, and Boundaries

**Files:**
- Read-only validation across created files.

- [ ] **Step 1: Verify all GitOps manifests are under `deploy/argocd`**

Run:

```bash
rtk find deploy/argocd -type f | sort
```

Expected output includes exactly these files:

```text
deploy/argocd/README.md
deploy/argocd/applications/agent-server.yaml
deploy/argocd/applications/docs-rs-mcp.yaml
deploy/argocd/manifests/agent-server/configmap.yaml
deploy/argocd/manifests/agent-server/deployment.yaml
deploy/argocd/manifests/agent-server/namespace.yaml
deploy/argocd/manifests/agent-server/secret.example.yaml
deploy/argocd/manifests/agent-server/service.yaml
deploy/argocd/manifests/mcp/docs-rs-mcp/deployment.yaml
deploy/argocd/manifests/mcp/docs-rs-mcp/service.yaml
deploy/argocd/root.yaml
```

- [ ] **Step 2: Verify no GitOps Application points to `k8s/`**

Run:

```bash
rtk grep -R "path: k8s" deploy/argocd || true
```

Expected output: no matches.

- [ ] **Step 3: Verify no legacy namespaces are used in GitOps manifests**

Run:

```bash
rtk grep -R "namespace: deribit\|namespace: mcp" deploy/argocd || true
```

Expected output: no matches.

- [ ] **Step 4: Verify no MCP template placeholder appears in GitOps manifests**

Run:

```bash
rtk grep -R '\${MCP_NAME}' deploy/argocd || true
```

Expected output: no matches.

- [ ] **Step 5: Validate YAML syntax using Ruby Psych**

Run:

```bash
ruby -e 'require "yaml"; ARGV.each { |f| YAML.load_stream(File.read(f)); puts "ok #{f}" }' \
  deploy/argocd/root.yaml \
  deploy/argocd/applications/agent-server.yaml \
  deploy/argocd/applications/docs-rs-mcp.yaml \
  deploy/argocd/manifests/agent-server/namespace.yaml \
  deploy/argocd/manifests/agent-server/configmap.yaml \
  deploy/argocd/manifests/agent-server/secret.example.yaml \
  deploy/argocd/manifests/agent-server/deployment.yaml \
  deploy/argocd/manifests/agent-server/service.yaml \
  deploy/argocd/manifests/mcp/docs-rs-mcp/deployment.yaml \
  deploy/argocd/manifests/mcp/docs-rs-mcp/service.yaml \
  .github/workflows/build-mcp-images.yml
```

Expected: one `ok <file>` line for each YAML file and exit code 0.

- [ ] **Step 6: Validate Kubernetes manifests with kubectl client dry-run**

Run:

```bash
kubectl apply --dry-run=client -f deploy/argocd/manifests/agent-server/namespace.yaml
kubectl apply --dry-run=client -f deploy/argocd/manifests/agent-server/configmap.yaml
kubectl apply --dry-run=client -f deploy/argocd/manifests/agent-server/secret.example.yaml
kubectl apply --dry-run=client -f deploy/argocd/manifests/agent-server/deployment.yaml
kubectl apply --dry-run=client -f deploy/argocd/manifests/agent-server/service.yaml
kubectl apply --dry-run=client -f deploy/argocd/manifests/mcp/docs-rs-mcp/deployment.yaml
kubectl apply --dry-run=client -f deploy/argocd/manifests/mcp/docs-rs-mcp/service.yaml
```

Expected: each command exits 0. If `kubectl` is not installed or no local OpenAPI schema is available, record the exact failure and rely on Step 5 syntax validation plus ArgoCD CRD validation in a configured cluster.

- [ ] **Step 7: Validate Dockerfile build**

Run:

```bash
docker build \
  --build-arg BIN=docs-rs-mcp \
  --build-arg REGION=global \
  -f dockers/vol-mcp-servers.Dockerfile \
  -t docs-rs-mcp:validation .
```

Expected: build exits 0.

- [ ] **Step 8: Commit validation-only fixes if needed**

If any validation step required fixing created files, run:

```bash
git add deploy/argocd dockers/vol-mcp-servers.Dockerfile .github/workflows/build-mcp-images.yml
git commit -m "fix(gitops): address validation issues" \
  -m "Co-Authored-By: Claude <noreply@anthropic.com>"
```

If no fixes were needed, do not create an empty commit.

---

### Task 8: Ingest Implementation Results into Wiki

**Files:**
- Modify: `docs/wiki/**` via `wiki-ingest` skill output.

- [ ] **Step 1: Invoke wiki-ingest**

After all implementation tasks and validations are complete, invoke the `wiki-ingest` skill with a summary like:

```text
Ingest the ArgoCD App-of-Apps GitOps deployment implementation: deploy/argocd self-contained manifests for agent-server and docs-rs-mcp, vol-agent-system namespace, MCP Dockerfile, and build-mcp-images GitHub Actions workflow that updates the docs-rs-mcp image tag in GitOps manifests.
```

- [ ] **Step 2: Review wiki changes**

Run:

```bash
git diff -- docs/wiki
```

Expected: wiki pages record the new GitOps deployment structure and workflow.

- [ ] **Step 3: Commit wiki updates**

Run:

```bash
git add docs/wiki
git commit -m "docs(wiki): ingest argocd gitops deployment" \
  -m "Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Self-Review

### Spec coverage

- App-of-Apps root and child Applications: Task 1.
- `vol-agent-system` namespace: Tasks 2 and 3 validations.
- `deploy/argocd` self-contained manifests independent of `k8s/`: Tasks 1-3 and Task 7 boundary checks.
- Initial services `agent-server` and `docs-rs-mcp`: Tasks 2 and 3.
- MCP Dockerfile and image workflow: Tasks 4 and 5.
- Workflow image tag update and no manifest-only trigger loop: Task 5 and Task 7.
- Wiki ingestion required by project instructions: Task 8.

### Placeholder scan

The plan contains no `TBD`, no incomplete file paths, and no shell-template `${MCP_NAME}` in target manifests. The only placeholder-like values are explicit example secret values in `secret.example.yaml`, which are required documentation and excluded from ArgoCD sync.

### Type and path consistency

All ArgoCD source paths point to `deploy/argocd/...`. All GitOps-managed Kubernetes manifests use `vol-agent-system`. The MCP service name, image repository, manifest path, Docker build `BIN`, and workflow matrix value are consistently `docs-rs-mcp`.
