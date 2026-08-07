# CLAUDE.md

Volatility-monitor + LLM-agent Rust workspace. Deeper context: [[docs/wiki/index]]

## Project Structure

```
crates/
├── vol-monitor/          # Deribit pipeline binary
├── vol-agent-server/     # Agent server binary (data-plane + control-plane)
├── vol-llm-runtime/      # AgentRuntime — single source of truth for tools/skills/MCP/providers
├── vol-llm-agent-protocol/  # JSON-RPC protocol, transport, handler abstractions
├── vol-llm-ui/           # DEPRECATED (2026-08): Dioxus WASM web frontend — replaced by React frontend/. TUI + shared state still maintained.
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
- **No doc tests**: write `#[cfg(test)]` unit tests or `tests/` integration tests instead. Doc comment code examples must use ` ```text` (not ` ```rust`). Check with `./scripts/check-no-doc-tests.sh`.
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

### Web Dev

The active web frontend is the **React app at `frontend/`** (not the deprecated Dioxus WASM crate).

```bash
make web-dev         # React dev server on :5173 (WS proxy to :3001)
make web-backend     # cargo-watch agent server on :3001
make web-build       # Production React build
```

#### Web Frontend shadcn/ui Conventions

The frontend uses **shadcn/ui** (Radix base) with Tailwind CSS v4. All UI primitives live in `frontend/src/components/ui/`. See `frontend/components.json` for config.

**CRITICAL — these rules are enforced in code review:**

| Rule | ✅ Correct | ❌ Wrong |
|------|-----------|----------|
| **Spacing** | `flex flex-col gap-4` | `space-y-4` |
| **Icons in buttons** | `<SearchIcon data-icon="inline-start" />` | `<SearchIcon className="h-4 w-4" />` (no data-icon) |
| **Truncate** | `className="truncate"` | `className="overflow-hidden text-ellipsis whitespace-nowrap"` |
| **Conditional classes** | `cn("base", condition && "extra")` | `` className={`base ${condition && "extra"}`} `` |
| **Items in groups** | `SelectItem` inside `SelectGroup` | `SelectItem` directly in `SelectContent` |
| **Icon sizing in components** | Let component handle via `[&_svg]:size-4` | Add `h-4 w-4` on icons inside Buttons/Badges |
| **Semantic colors** | `bg-primary`, `text-muted-foreground` | `bg-blue-500`, `text-gray-400` |
| **Empty states** | `<Empty><EmptyHeader><EmptyTitle>...</EmptyTitle></EmptyHeader></Empty>` | Custom `<div>` with centered text |
| **Separators** | `<Separator />` | `<hr>` or `<div className="border-t">` |
| **Button variants** | Use built-in variants + semantic `success` | Raw color overrides like `bg-emerald-600` |
| **Dialog/Sheet** | Always include `DialogTitle` / `SheetTitle` | Missing title (accessibility violation) |
| **Loading** | `<Skeleton className="h-4 w-48" />` | Custom `animate-pulse` divs |
| **Status badges** | `<Badge variant="secondary">` | Custom styled `<span>` |

**Adding components:** `cd frontend && npx shadcn@latest add <name>` — move files from any stray `frontend/@/` path to `frontend/src/components/ui/`. Never create UI primitives manually.

**Tailwind v4 specifics:** `@theme` block for custom tokens, `@theme inline` for shadcn CSS variable mapping. No `tailwind.config.js` — all config is in `frontend/src/index.css`.

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
