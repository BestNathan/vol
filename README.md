# vol

A Rust workspace containing a full LLM agent system — ReAct orchestration, tools, skills,
MCP integration, sandboxes, sub-agent dispatch, sessions, TUI and web frontends.

---

## 1. The Agent System

- **ReAct orchestration** with pluggable providers (Anthropic, OpenAI, DashScope)
- **Tools** — built-in file/bash/web tools, CLI-style `fs` and `task` tools, skill tools, and MCP tools
- **Skills** — markdown-frontmatter skills injected into the agent context
- **MCP** — client for external MCP servers plus bundled servers (`docs-rs-mcp`, `cli-tools-mcp`, `playwright-mcp`)
- **Sandboxes** — Local / Tmp / SSH / Firecracker / Wasm execution environments
- **Sub-agent dispatch** — the built-in `agent` tool dispatches work to other agents declared in `.agents/agents/*.toml`
- **Sessions & tasks** — persisted to SQLite / Postgres
- **Frontends** — TUI and React web frontend
- **Server** — JSON-RPC over WebSocket with a control-plane / data-plane split

## 2. Architecture

### 2.1 Core Concepts

- **`AgentRuntime`** ([`vol-llm-runtime`](crates/vol-llm-runtime)) is the single source of
  truth for agent resources — tools, skills, MCP clients, providers, session/task stores.
  Tool registration happens in `AgentRuntimeBuilder::build()`. [[vol-llm-runtime-crate]]
- **`AgentDef`** — agents are declared in `.agents/agents/*.toml` and run a ReAct loop
  (`vol-llm-agent`): think → tool call → observe, until a final answer.
- **Context** — `ContextBuilder` composes the prompt from `ContextContributor`s
  (skills, available-agent list via `AgentInjector`, session history) under a token
  budget. [[vol-llm-context-crate]]
- **Protocol** — JSON-RPC 2.0 over WebSocket is the only application protocol; HTTP is
  reserved for `/health` and `/metrics`. Wire types live in
  [`vol-llm-agent-protocol`](crates/vol-llm-agent-protocol). [[vol-llm-agent-protocol-crate]]

### 2.2 Control Plane / Data Plane

The agent server (`vol-agent-server`) supports three deployment modes configured via TOML:

| Mode | `control_plane` | `data_plane` | Description |
|------|-----------------|-------------|-------------|
| Standalone data-plane | false | true | Single-node agent execution (legacy `/ws` behavior) |
| Standalone control-plane | true | false | Cluster coordinator — registry, routing, capability index |
| Combined | true | true | Both in one process, local node self-registers |

```
                    ┌──────────────────────────────────┐
 Client / UI / CLI  │     vol-agent-server             │
 ─── JSON-RPC /ws ─►│                                   │
                    │  ┌─────────────────────────────┐  │
                    │  │  ControlPlaneServerCore      │  │
                    │  │  NodeRegistry  CapabilityIndex│  │
                    │  │  ControlRouter  LeaseManager  │  │
                    │  └─────────────┬───────────────┘  │
                    │                │                   │
                    │  ┌─────────────▼───────────────┐  │
                    │  │  DataPlaneServerCore         │  │
                    │  │  AgentRuntime  AgentRouter   │  │
                    │  │  ToolRegistry  McpManager    │  │
                    │  └─────────────────────────────┘  │
                    └──────────────────────────────────┘
                                 │
            ┌────────────────────┼────────────────────┐
            ▼                    ▼                    ▼
   vol-llm-agent-protocol   vol-llm-runtime    vol-llm-tool
   (JSON-RPC + transport)   (execution owner)  (ToolRegistry)
```

- Clients connect at `/ws`; data-plane nodes link to the control plane at `/control/v1/ws`.
- Both planes live in `vol-agent-server` (no separate control-plane crate).
- **Dependency direction**: `vol-agent-server` → `vol-llm-agent-protocol` + `vol-llm-runtime`.
  Protocol and runtime must not depend on server (`./scripts/check-agent-boundaries.sh`).

[[agent-server-control-data-plane]]

### 2.3 Tools & Sandboxes

- **`ToolRegistry`** — every tool implements the `Tool` trait and receives a `ToolContext`;
  registered once in `AgentRuntimeBuilder::build()`. [[tool-registry]]
- **Built-in tools** — `read` / `write` / `edit` / `grep` / `bash` / `web-search` /
  `web-fetch` (`vol-llm-tools-builtin`).
- **CLI-as-tool** — the `fs` and `task` tools expose CLI-style subcommands
  (`fs read <path>`, `fs grep <pattern>`, `--json` envelope) over the built-ins,
  sharing the `vol-llm-cli-tool` abstraction. [[vol-llm-fs-crate]] [[fs-cli-tool]]
- **Sandboxes** — tools execute inside sandboxes: Local / Tmp / SSH / Firecracker / Wasm.
  `SandboxManager` provides unified lifecycle management with explicit instance identity,
  state tracking, and provider-based backend abstraction. Configuration lives in
  `.agents/sandboxes/*.toml`. [[sandbox-lifecycle]] [[vol-llm-sandbox-crate]]

### 2.4 Agent–Sub-agent Collaboration

The built-in `agent` tool lets an agent autonomously dispatch sub-tasks to other agents
declared in `.agents/agents/`:

- **Dispatch by `AgentDef.id`** — the sub-agent runs its own full ReAct loop and returns
  its final result synchronously. Dispatch is equivalent to a data-plane task submission;
  the decision maker switches from human to agent. [[agenttool-subagent-dispatch]]
- **Depth guard** — the only nesting control: `tool_config.agent.max_depth` (default 1 =
  root may dispatch one layer; deeper dispatch is rejected).
- **Sessions persist by name** — sub-agent sessions are keyed by agent name and remain
  observable by other agents and the UI.
- **`AgentInjector`** — contributes the list of available agents to the context so the
  model knows it can dispatch. [[vol-llm-agent-tool-crate]]

### 2.5 Deployment Architecture

- **ArgoCD GitOps (primary)** — `deploy/argocd/root.yaml` (app-of-apps) deploys:
  control-plane (`agent-server`), data-plane nodes (`agent-server-dp`), specialized
  agent workers (`agent-server-ansible`, `agent-server-dingtalk`), MCP servers,
  `nginx-proxy` + React frontend (`vol-llm-ui`). [[argocd-app-of-apps-gitops]]
- **Runtime config as ConfigMaps** — agents, providers, sandboxes, MCP endpoints and
  secrets live under `deploy/argocd/manifests/runtime-config/`, regenerated with
  `python3 scripts/sync-configmaps.py`.
- **Kustomize alternative** — `deploy/kustomize/overlays/{control-plane,data-plane}`.
- **Legacy `k8s/` tree is deprecated** — prefer ArgoCD.
- MCP servers run as standalone Deployments + ClusterIP Services. [[mcp-transport-pattern]]

See [[docs/wiki/concepts/argocd-app-of-apps-gitops]] for the full guide.

## 3. Project Structure

### Agent Crates

| Crate | Responsibility |
|-------|---------------|
| `vol-llm-core` | LLM abstractions, types, traits |
| `vol-llm-provider` | Anthropic, OpenAI, DashScope provider implementations |
| `vol-llm-tool` | `ToolRegistry`, `Tool` trait, `ToolContext` |
| `vol-llm-tools-builtin` | `read`/`write`/`edit`/`grep`/`bash`/`web-search`/`web-fetch` |
| `vol-llm-cli-tool` | Core abstraction for "CLI-as-Tool" (shared by `fs`/`task`) |
| `vol-llm-fs` | CLI-style `fs` tool over the file-op built-ins |
| `vol-llm-task` | Task models and stores (SeaORM SQLite/Postgres) |
| `vol-llm-sandbox` | Sandbox lifecycle management: `SandboxManager`, `SandboxProvider`, `SandboxStore` + implementations (Local/Tmp/SSH/Firecracker/Wasm) |
| `vol-llm-skill` | Skill system (markdown-frontmatter) |
| `vol-llm-agent` | ReAct orchestration, `AgentConfig`, plugin system |
| `vol-llm-agents` | High-level agent implementations |
| `vol-llm-yaml-agent` | Declarative agent definitions via YAML |
| `vol-llm-agent-tool` | `AgentTool` (sub-agent dispatch) + `AgentInjector` |
| `vol-llm-context` | `ContextBuilder` / `ContextContributor` prompt construction |
| `vol-llm-memory` | Layered memory abstractions for cross-session agent memory |
| `vol-llm-wiki` | Wiki compression and management tool |
| `vol-llm-mcp` | MCP client, server lifecycle, tool/resource/prompt discovery |
| `vol-llm-runtime` | `AgentRuntime` — single source of truth for runtime resources |
| `vol-llm-agent-protocol` | JSON-RPC protocol (`Operation`/`Payload`/`control.*`) + transport |
| `vol-session` | Session persistence (file + SeaORM SQLite/Postgres) |
| `vol-agent-server` | Agent server binary — `DataPlaneServerCore` + `ControlPlaneServerCore` |
| `vol-llm-tui` | Terminal UI (ratatui) |
| `vol-llm-ui` | **DEPRECATED** — Dioxus WASM web frontend; replaced by React `frontend/` |
| `vol-mcp-servers` | MCP server implementations (`docs-rs-mcp`, `cli-tools-mcp`, `playwright-mcp`) |
| `md-frontmatter` | Markdown frontmatter parser |
| `ppt-agent` | PowerPoint generation agent |

## 4. Installation & Deployment

### Prerequisites

Rust toolchain (see `rust-toolchain.toml`), [`just`](https://github.com/casey/just) for
recipes, Node.js for the web frontend.

### Local

```bash
# Agent server (standalone data-plane)
cp configs/vol-agent-server.env.example .env
source .env
cargo run -p vol-agent-server
# Per-mode configs: configs/vol-agent-server.{data-plane,control-plane}.toml

# Web frontend (React, 2 terminals)
just web-backend          # agent server on :3001 (cargo-watch)
just web-dev              # Vite dev server on :5173 (WS proxy to :3001)
```

### Docker

```bash
just docker-agent                                   # or:
docker build -f dockers/vol-agent-server.Dockerfile -t vol-agent-server .
docker build -f dockers/vol-agent-server.alpine.Dockerfile -t vol-agent-server:alpine .
```

### Kubernetes

```bash
# ArgoCD GitOps (primary)
kubectl apply -f deploy/argocd/root.yaml

# Kustomize (alternative)
kubectl apply -k deploy/kustomize/overlays/control-plane
kubectl apply -k deploy/kustomize/overlays/data-plane

# Post-deploy verification
./scripts/smoke-test.sh --all
```

Runtime config changes are synced to ConfigMaps with `python3 scripts/sync-configmaps.py`.
See [[docs/wiki/concepts/argocd-app-of-apps-gitops]].

## 5. AI-Driven Development Workflow

This project uses Superpowers skills for structured development:

```
clarifying-requirements ──► brainstorming ──► writing-architecture
      (需求澄清)               (方案脑暴)          (架构设计)

writing-architecture ──► writing-plans ──► subagent-driven-development
      (架构设计)              (实现计划)           (按 task 派发 subagent)
```

| Phase | Output | Location |
|-------|--------|----------|
| Requirement | Requirement doc | `docs/superpowers/requirement/` |
| Architecture | Design doc | `docs/superpowers/architectures/` |
| Spec | Addendum / detailed spec | `docs/superpowers/specs/` |
| Plan | Task-level implementation plan | `docs/superpowers/plans/` |
| Wiki | Compiled knowledge base | `docs/wiki/` |

### Task Completion Checklist

1. `just test-crate <affected-crate>` — all tests pass
2. `just cover-gate <affected-crate> 80` — coverage gate
3. `./scripts/check-agent-boundaries.sh` — dependency direction
4. `just fmt-check && just clippy-strict` — formatting & lint
5. `wiki-ingest` — ingest changes into `docs/wiki`
6. (If UI affected) `just fe-test` + `just fe-e2e` — frontend test tiers

## 6. Core Tools & Commands

All recipes run via `just` (`just help` for the full list).

| Area | Commands |
|------|----------|
| Build & check | `just check`, `just clippy`, `just clippy-strict`, `just fmt`, `just fmt-check` |
| Tests | `just test-unit`, `just test-integration`, `just test-crate <crate>`, `just test-e2e`, `just test-tools`, `just test-sandbox` |
| Coverage | `just cover <crate>`, `just cover-gate <crate> 80`, `just cover-html <crate>`, `just cover-tools` |
| Guards | `just boundaries`, `just no-doc-tests`, `just no-clippy-allow`, `just audit` |
| Web | `just web-dev`, `just web-backend`, `just web-build`, `just web-serve` |
| Frontend tests | `just fe-test` (`fe-test-unit` / `fe-test-integration`), `just fe-e2e`, `just fe-lint`, `just fe-type` |
| Docker | `just docker-agent` |

### Model Service

| Endpoint | `http://192.168.2.162:31693` |
|----------|------------------------------|
| Models | `gpt5.5`, `coding`, `qwen3.6-plus`, `glm5.1` |

Provider config lives in `.agents/providers/*.toml` and is auto-discovered.

---

## Documentation

| Path | Topic |
|------|-------|
| `CLAUDE.md` | AI agent quick reference (conventions, guardrails, commands) |
| `docs/CONFIGURATION.md` | Full configuration guide (TOML sections, env vars, K8s) |
| `docs/wiki/index.md` | Wiki index — entities, concepts, sources, full search |
| `docs/superpowers/` | Requirement / architecture / spec / plan documents |
