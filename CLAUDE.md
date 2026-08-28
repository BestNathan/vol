# CLAUDE.md

Vol-agent-system Rust workspace. Deeper context: [[docs/wiki/index]]

## Project Structure

```
crates/
├── vol-agent-server/     # Agent server binary (data-plane + control-plane)
├── vol-llm-runtime/      # AgentRuntime — single source of truth for tools/skills/MCP/providers
├── vol-llm-agent-protocol/  # JSON-RPC protocol, transport, handler abstractions
├── vol-llm-ui/           # DEPRECATED (2026-08): Dioxus WASM web frontend — replaced by React frontend/. TUI + shared state still maintained.
├── vol-llm-tui/          # Terminal UI
├── vol-llm-agent/        # ReAct agent orchestration
├── vol-llm-agent-tool/   # AgentTool 派发工具 + AgentInjector（高层组合 crate）
├── vol-llm-mcp/          # MCP client
├── vol-mcp-servers/      # MCP server implementations
├── vol-llm-tool/         # ToolRegistry
├── vol-llm-skill/        # Skill system
├── vol-llm-task/         # Task management
├── vol-llm-provider/     # Anthropic / OpenAI providers
└── vol-session/          # Session persistence
configs/                  # Example configs (one per server)
dockers/                  # Dockerfiles (one per service)
k8s/                      # Kubernetes manifests (agent-server/ mcp/)
scripts/                  # Build / deploy helpers
```

[[docs/wiki/index]] — full entity/concept/source index.

## Conventions

- **Task done → `wiki-ingest`**: always ingest implementation results to `docs/wiki`.
- **Coverage ≥ 80%**: `just cover-gate <crate> 80` before claiming done. Exception: `main.rs`, `app.rs`, `health.rs`.
- **Every new `pub fn` / handler → at least one test**.
- **No doc tests**: write `#[cfg(test)]` unit tests or `tests/` integration tests instead. Doc comment code examples must use ` ```text` (not ` ```rust`). Check with `./scripts/check-no-doc-tests.sh`.
- **Tool registration**: `AgentRuntimeBuilder::build()` is the primary place. `DataPlaneServerCoreBuilder` inherits from it; do not duplicate.
- **`vol-llm-agent-protocol` owns wire types**: `Operation`, `Payload`, `control.*`, JSON-RPC codec. No wire type definitions in `vol-agent-server`.
- **`vol-llm-runtime` knows nothing about control-plane**. No `NodeRegistry` / `ControlRouter` imports there.
- **Docker builds use `rsproxy.cn`** mirror — copy `.cargo/config.toml` into builder stage.
- **Web frontend**: use `just web-*` commands; never `cargo build/run` directly for vol-llm-ui.

## Guardrails

- **No `vol-agent-control-plane` crate** — control-plane lives in `vol-agent-server::control_plane`.
- **`vol-llm-agent-protocol` must not depend on `vol-agent-server`** (verify: `./scripts/check-agent-boundaries.sh`).
- **`vol-llm-runtime` must not depend on `vol-agent-server`**.
- **No plaintext Kubernetes Secrets in git** — commit `kind: SealedSecret` (encrypted via sealed-secrets) only. Use `scripts/seal-secret.sh` to encrypt (auto-downloads the right kubeseal version from the cluster). Plain `kind: Secret` is blocked by `scripts/check-no-plaintext-secrets.sh` (pre-commit + CI). Documentation placeholders are OK only if named `*.example.yaml` / `*.template.yaml`.
- **JSON-RPC params/results are flat** — `ControlPayload` must not use `#[serde(tag/ content=...)]`.
- **Route collision**: `control_plane.client_ws_path` must ≠ `node_ws_path` and ≠ `/health` (config validation rejects).
- **Combined mode** (`control_plane=true, data_plane=true`): `/ws` goes to control-plane; local data-plane registers in-process.
- **New protocol operation → register in codec**: when adding a new variant to any `*Operation` enum (e.g. `SandboxOperation::ListSpecs`), you MUST:
  1. Add the `method_name()` match arm in `agent_server_protocol.rs`
  2. Add the reverse mapping in `operation_codec.rs` (`method_to_operation`)
  3. Add the payload decode branch in `Payload::from_operation`
  4. Verify with `./scripts/check-protocol-registration.sh` — ensures every operation variant is registered in the codec

## Commands

### Build & Check

**Always use `just`, never raw `cargo`** — the recipes wrap nextest, feature flags, and fallbacks that raw cargo misses.

```bash
just check          # cargo check --workspace
just fmt            # format all Rust code
just fmt-check      # CI formatting gate
just clippy         # workspace clippy (warnings allowed)
just clippy-strict  # -D warnings (CI gate)
just test-compile   # compile all tests without running
```

### Test & Coverage

```bash
# Tiers
just test-unit                    # unit tests only (src/ inline #[cfg(test)])
just test-integration             # integration tests only (tests/, kind(test) filter)
just test                         # unit + integration (all non-e2e)
just test-crate <crate>           # single crate, all targets
just test-unit-crates <crate...>  # unit tests for specific crates (pre-push tier)
just test-slow                    # slow profile: 120s slow-timeout, terminate-after 5
just test-e2e                     # #[ignore = "e2e"] tests (need external services)
just test-sandbox                 # sandbox crate only
just test-sandbox-ssh             # sandbox with ssh feature
just test-tools                   # tool + sandbox + builtins

# Coverage
just cover <crate>                # summary
just cover-gate <crate> 80        # gate check (≥80%)
just cover-gate-multi 80 <c1> <c2>
just cover-html <crate>           # browser report

# Guards
just no-doc-tests            just boundaries
just no-clippy-allow         just no-plaintext-secrets
just audit
```

#### Test-running gotchas (learned the hard way)

- **Compilation dominates.** `vol-agent-server` test binaries take **~9 minutes** to compile cold; the 199 tests then run in **2.5 seconds**. A "hanging" test run is almost always still compiling. Compile first with `cargo nextest run -p <crate> --no-run` to see progress, then run.
- **`just test-*` recipes redirect stderr to `/dev/null`** (for the nextest→cargo fallback), which also swallows all nextest progress output. When you need to watch progress, call `cargo nextest run` directly — but only for diagnosis, not as the normal path.
- **The nextest `default` profile has no `slow-timeout`** — tests can hang forever. Only `--profile slow` (`just test-slow`) has `terminate-after = 5`. Use it when you suspect a hang.
- **`vol-agent-server` integration tests need `--features vol-agent-server/test-utils`** — `just test-integration` passes this already; `just test-crate` does not.
- Never `pkill -f "cargo test"` while other cargo work is in flight — it kills background test tasks too.

### Web Dev

The active web frontend is the **React app at `frontend/`** (not the deprecated Dioxus WASM crate).

```bash
just web-dev         # React dev server on :5173 (WS proxy to :3001)
just web-backend     # cargo-watch agent server on :3001
just web-build       # Production React build
```

Frontend test tiers (vitest projects + Playwright):

```bash
just fe-test-unit          # vitest unit project (tests/unit/, node)
just fe-test-integration   # vitest integration project (tests/integration/, jsdom + testing-library)
just fe-test               # both projects with coverage
just fe-e2e                # Playwright e2e (tests/e2e/, self-contained mock backend)
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
```

### K8s

```bash
# ArgoCD GitOps (primary)
kubectl apply -f deploy/argocd/root.yaml

# Kustomize (alternative, less duplication)
kubectl apply -k deploy/kustomize/overlays/control-plane
kubectl apply -k deploy/kustomize/overlays/data-plane

# Legacy (deprecated — prefer ArgoCD)
kubectl create namespace deribit  # legacy path prerequisite (namespace.yaml removed 2026-08-21)
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

[[docs/wiki/concepts/argocd-app-of-apps-gitops]] — GitOps architecture.

## Model Service

```
Base: http://192.168.2.162:31693
Models: gpt5.5, coding, qwen3.6-plus, glm5.1
```
