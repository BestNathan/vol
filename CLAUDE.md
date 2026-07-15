# CLAUDE.md

Volatility-monitor + LLM-agent Rust workspace. Deeper context: [[docs/wiki/index]]

## Project Structure

```
crates/
├── vol-monitor/          # Deribit pipeline binary
├── vol-agent-server/     # Agent server binary (data-plane + control-plane)
├── vol-llm-runtime/      # AgentRuntime — single source of truth for tools/skills/MCP/providers
├── vol-llm-agent-protocol/  # JSON-RPC protocol, transport, handler abstractions
├── vol-llm-ui/           # Dioxus WASM web frontend (use make web-* commands)
├── vol-llm-tui/          # Terminal UI
├── vol-llm-agent/        # ReAct agent orchestration
├── vol-llm-mcp/          # MCP client
├── vol-mcp-servers/      # MCP server implementations
├── vol-llm-tool/         # ToolRegistry
├── vol-llm-skill/        # Skill system
├── vol-llm-task/         # Task management
├── vol-llm-provider/     # Anthropic / OpenAI providers
├── vol-session/          # Session persistence
└── vol-*/                # Volatility pipeline crates
configs/                  # Example configs (one per server)
dockers/                  # Dockerfiles (one per service)
k8s/                      # Kubernetes manifests (vol-monitor/ agent-server/ mcp/)
scripts/                  # Build / deploy helpers
```

[[docs/wiki/index]] — full entity/concept/source index.

## Conventions

- **Task done → `wiki-ingest`**: always ingest implementation results to `docs/wiki`.
- **`docs/superpowers/*` → Lark**: upload new/updated superpowers docs to the corresponding Lark wiki node.
- **Coverage ≥ 80%**: `make coverage-threshold PKG=<crate>` before claiming done. Exception: `main.rs`, `app.rs`, `health.rs`.
- **Every new `pub fn` / handler → at least one test**.
- **Tool registration**: `AgentRuntimeBuilder::build()` is the primary place. `DataPlaneServerCoreBuilder` inherits from it; do not duplicate.
- **`vol-llm-agent-protocol` owns wire types**: `Operation`, `Payload`, `control.*`, JSON-RPC codec. No wire type definitions in `vol-agent-server`.
- **`vol-llm-runtime` knows nothing about control-plane**. No `NodeRegistry` / `ControlRouter` imports there.
- **Docker builds use `rsproxy.cn`** mirror — copy `.cargo/config.toml` into builder stage.
- **Web frontend**: use `make web-*` commands; never `cargo build/run` directly for vol-llm-ui.

## Guardrails

- **No `vol-agent-control-plane` crate** — control-plane lives in `vol-agent-server::control_plane`.
- **`vol-llm-agent-protocol` must not depend on `vol-agent-server`** (verify: `./scripts/check-agent-boundaries.sh`).
- **`vol-llm-runtime` must not depend on `vol-agent-server`**.
- **JSON-RPC params/results are flat** — `ControlPayload` must not use `#[serde(tag/ content=...)]`.
- **Route collision**: `control_plane.client_ws_path` must ≠ `node_ws_path` and ≠ `/health` (config validation rejects).
- **Combined mode** (`control_plane=true, data_plane=true`): `/ws` goes to control-plane; local data-plane registers in-process.

## Commands

### Build & Check

```bash
cargo check -p vol-agent-server -p vol-llm-agent-protocol
cargo build -p vol-agent-server --release
```

### Test & Coverage

```bash
cargo test -p vol-agent-server -p vol-llm-agent-protocol
make coverage PKG=vol-agent-server                        # summary
make coverage-threshold PKG=vol-agent-server PCT=80      # gate check
make coverage-html PKG=vol-llm-agent-protocol             # browser report
```

### Web Dev (3 terminals)

```bash
make web-css         # Tailwind watch
make web-dev         # Dioxus WASM on :8080
make web-backend     # cargo-watch agent server on :3001
```

Pre-flight: `which dx && npm ci --prefix crates/vol-llm-ui`

### Docker

```bash
docker build -f dockers/vol-agent-server.Dockerfile -t vol-agent-server .
docker build -f dockers/vol-monitor.cross.Dockerfile -t vol-monitor .
```

### Lark Docs

```bash
# Upload
lark-cli docs +create --api-version v2 --doc-format markdown \
  --content @path/to/doc.md --wiki-node "<node-id>" --as user

# Update
lark-cli docs +update --api-version v2 --doc "<url-or-token>" \
  --command overwrite --doc-format markdown \
  --content @path/to/doc.md --as user
```

| Superpowers dir | Lark node id |
|---|---|
| `docs/superpowers/plans/*` | `TEkkw1W6niuBxQkcvswchOo5nhb` |
| `docs/superpowers/requirement/*` | `PPDZw7LFqiFjMTkAXFocFoO6nce` |
| `docs/superpowers/specs/*` | `Og7twpiPoi0Vbjk2EzvcqX92nsb` |

### K8s

```bash
# ArgoCD GitOps (primary)
kubectl apply -f deploy/argocd/root.yaml

# Kustomize (alternative, less duplication)
kubectl apply -k deploy/kustomize/overlays/control-plane
kubectl apply -k deploy/kustomize/overlays/data-plane

# Legacy (deprecated — prefer ArgoCD)
kubectl apply -f k8s/namespace.yaml
./k8s/vol-monitor/deploy.sh latest
kubectl apply -f k8s/agent-server/deployment.yaml
```

### Post-deploy Verification

```bash
./scripts/smoke-test.sh --all                    # test all components
./scripts/smoke-test.sh -H localhost:3001        # direct endpoint
```

### Runtime Config Sync

```bash
python3 scripts/sync-configmaps.py               # regenerate ConfigMap manifests
```

[[docs/deployment/k8s-deployment]] — full deployment guide.
[[docs/wiki/concepts/argocd-app-of-apps-gitops]] — GitOps architecture.

## Model Service

```
Base: http://192.168.2.162:31693
Models: gpt5.5, coding, qwen3.6-plus, glm5.1
```
