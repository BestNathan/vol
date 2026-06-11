# vol

A Rust workspace containing a Deribit volatility monitoring pipeline and a full LLM agent framework —
ReAct orchestration, MCP integration, built-in tools, skills, TUI and web frontends.

---

## Architecture

### Control Plane / Data Plane

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

### Crate Boundaries

| Crate | Responsibility |
|-------|---------------|
| `vol-llm-agent-protocol` | JSON-RPC codec, `Operation`/`Payload`, `Connection`, `DomainHandler`, `HandlerRegistry`, `JsonRpcMessageService` |
| `vol-llm-runtime` | `AgentRuntime` — authoritative owner of tools, skills, MCP, providers, task/session stores |
| `vol-agent-server` | `DataPlaneServerCore`, `ControlPlaneServerCore`, role composition, config, routes |
| `vol-llm-agent` | ReAct orchestration, `AgentConfig`, plugin system |
| `vol-llm-tool` | `ToolRegistry`, `Tool` trait, `ToolContext` |
| `vol-llm-mcp` | MCP client (`McpManager`), server lifecycle, tool/resource/prompt discovery |
| `vol-llm-skill` | Skill loader, `SkillTool`, skill injection into agent context |
| `vol-llm-task` | Task models, file/database task stores |
| `vol-session` | Session persistence (file + SeaORM SQLite/Postgres) |
| `vol-llm-provider` | Anthropic, OpenAI, DashScope provider implementations |
| `vol-llm-ui` | Dioxus 0.6 WASM web frontend |
| `vol-llm-tui` | Terminal UI (ratatui) |
| `vol-mcp-servers` | MCP server implementations (docs-rs-mcp) |

**Dependency direction**: `vol-agent-server` → `vol-llm-agent-protocol` + `vol-llm-runtime`. Protocol must not depend on server. Runtime must not depend on server.

### Volatility Monitoring Pipeline

```
Config ──► DataSource (Deribit) ──► EventBus (broadcast) ──► Alert Rules ──► Notifications (Feishu/Stdout)
                                       │
                                  TDengine (time-series storage)
```

Event-driven with `TracedEvent<T>` wrappers, plugin traits (`DataSource`, `RuleProcessor`, `NotificationHandler`), and OpenTelemetry/Jaeger tracing.

### Full crate listing

```
crates/
├── vol-core/                  Core traits and data models
├── vol-config/                TOML configuration loading
├── vol-tracing/               TracedEvent<T> wrappers
├── vol-eventbus/              Event bus (tokio broadcast)
├── vol-deribit/               Deribit WebSocket client & types
├── vol-datasource/            Data source implementations
├── vol-alert/                 Alert rule implementations
├── vol-rules/                 Rule processors
├── vol-notification/          Feishu / Stdout notification handlers
├── vol-engine/                Monitoring engine orchestration
├── vol-monitor/               Main binary — vol-monitor
├── vol-tdengine/              TDengine REST API client
├── vol-observability/         Prometheus metrics HTTP server
│
├── vol-llm-core/              LLM abstractions, types, traits
├── vol-llm-provider/          Anthropic, OpenAI, DashScope
├── vol-llm-tool/              ToolRegistry framework
├── vol-llm-tools-builtin/     read/write/edit/grep/bash/web-search/web-fetch
├── vol-llm-agent/             ReAct agent orchestration
├── vol-llm-agents/            High-level agent implementations
├── vol-llm-context/           Context/memory management
├── vol-llm-memory/            Conversation persistence
├── vol-llm-skill/             Skill system (markdown-frontmatter)
├── vol-llm-task/              Task management
├── vol-llm-runtime/           AgentRuntime — single source of truth
├── vol-llm-agent-protocol/    JSON-RPC protocol + transport abstractions
├── vol-llm-mcp/               MCP client (rmcp)
├── vol-llm-wiki/              Wiki/knowledge-base tool
├── vol-llm-observability/     OTel agent logging
│
├── vol-agent-server/          Agent server binary (data + control plane)
├── vol-llm-ui/                Web frontend (Dioxus 0.6 WASM + Tailwind)
├── vol-llm-tui/               Terminal UI
├── vol-mcp-servers/           MCP server implementations
│
├── md-frontmatter/            Markdown frontmatter parser
└── ppt-agent/                 PowerPoint generation agent
```

---

## Development Tools

### Quick Start

```bash
# Prerequisites
rustup target add wasm32-unknown-unknown
cargo install dioxus-cli --version 0.6.3 --locked
cargo install cargo-watch --locked
npm ci --prefix crates/vol-llm-ui

# Agent server (standalone data-plane)
cp configs/vol-agent-server.env.example .env
source .env
cargo run -p vol-agent-server

# All web services (3 terminals)
make web-css          # Tailwind watch
make web-dev          # Dioxus WASM :8080
make web-backend      # Agent server :3001

# Volatility monitor
cp configs/vol-monitor.env.example .env
source .env
cargo run -p vol-monitor -- --config configs/vol-monitor.example.toml
```

### Test & Coverage

```bash
# Run tests
cargo test -p vol-agent-server -p vol-llm-agent-protocol

# Coverage (≥80% required for agent-server and protocol)
make coverage PKG=vol-agent-server                        # summary
make coverage-threshold PKG=vol-agent-server PCT=80      # gate check
make coverage-html PKG=vol-llm-agent-protocol             # browser report

# Dependency boundary check
./scripts/check-agent-boundaries.sh
```

### Docker

```bash
docker build -f dockers/vol-agent-server.Dockerfile -t vol-agent-server .
docker build -f dockers/vol-agent-server.alpine.Dockerfile -t vol-agent-server:alpine .
docker build -f dockers/vol-monitor.cross.Dockerfile -t vol-monitor .  # amd64 + arm64
```

### Config & Env

```
configs/
├── vol-monitor.example.toml            # Pipeline config
├── vol-agent-server.example.toml       # Agent server config (all sections)
├── vol-monitor.env.example             # Pipeline env
└── vol-agent-server.env.example        # Agent server env
```

### Model Service

| Endpoint | `http://192.168.2.162:31693` |
|----------|------------------------------|
| Models | `gpt5.5`, `coding`, `qwen3.6-plus`, `glm5.1` |

Provider config lives in `.agents/providers/*.toml` and is auto-discovered.

---

## AI Workflow

This project uses Superpowers skills for structured development. Key workflows:

### Design → Plan → Implement

```
clarifying-requirements ──► brainstorming ──► writing-architecture
      (需求澄清)               (方案脑暴)          (架构设计)

writing-architecture ──► writing-plans ──► subagent-driven-development
      (架构设计)              (实现计划)           (按 task 派发 subagent)
```

### Artifacts

| Phase | Output | Location |
|-------|--------|----------|
| Architecture | Design doc | `docs/superpowers/architectures/` |
| Spec | Addendum / detailed spec | `docs/superpowers/specs/` |
| Plan | Task-level implementation plan | `docs/superpowers/plans/` |
| Wiki | Compiled knowledge base | `docs/wiki/` |

### Task Completion Checklist

1. `cargo test -p <affected-crate>` — all tests pass
2. `make coverage-threshold PKG=<affected-crate> PCT=80` — coverage gate
3. `./scripts/check-agent-boundaries.sh` — dependency direction
4. `cargo fmt --all --check` — formatting
5. `wiki-ingest` — ingest changes into `docs/wiki`
6. Upload changed `docs/superpowers/*` to Lark
7. (If UI affected) Playwright verification — `make web-backend` + `make web-dev`, navigate tabs

---

## Documentation

| Path | Topic |
|------|-------|
| `CLAUDE.md` | AI agent quick reference (conventions, guardrails, commands) |
| `docs/CONFIGURATION.md` | Full configuration guide (TOML sections, env vars, K8s) |
| `docs/architecture/overview.md` | System architecture and data flow |
| `docs/architecture/crates.md` | Crate organization |
| `docs/deployment/docker-build.md` | Docker multi-stage builds, ACR push |
| `docs/deployment/k8s-deployment.md` | K8s deployment, secrets, troubleshooting |
| `docs/wiki/index.md` | Wiki index — entities, concepts, sources, full search |
| `docs/superpowers/architectures/` | Architecture design documents |
| `docs/superpowers/specs/` | Design specifications |
| `docs/superpowers/plans/` | Implementation plans |

---

## Service Deployment

### Agent Server

```bash
# Local
cargo run -p vol-agent-server

# Docker
docker build -f dockers/vol-agent-server.Dockerfile -t vol-agent-server .
docker run -d -p 3001:3001 \
  -v $(pwd)/.agents:/app/.agents:ro \
  -e ANTHROPIC_AUTH_TOKEN=sk-xxx \
  vol-agent-server

# K8s
kubectl apply -f k8s/agent-server/configmap.yaml
kubectl apply -f k8s/agent-server/secret.yaml
kubectl apply -f k8s/agent-server/deployment.yaml
```

### Vol Monitor

```bash
# Local
cargo run -p vol-monitor -- --config configs/vol-monitor.example.toml

# Docker
docker build -f dockers/vol-monitor.Dockerfile -t vol-monitor .

# K8s
kubectl apply -f k8s/vol-monitor/configmap.yaml
kubectl create secret generic vol-monitor-secrets \
  --from-literal=DERIBIT_CLIENT_ID=<id> \
  --from-literal=DERIBIT_CLIENT_SECRET=<secret> \
  -n deribit
kubectl apply -f k8s/vol-monitor/deployment.yaml
```

### MCP Server

```bash
cargo run -p vol-mcp-servers --bin docs-rs-mcp
./k8s/mcp/deploy.sh docs-rs-mcp v0.1.0
```

### Web Frontend

```bash
# Dev (3 terminals)
make web-css && make web-dev && make web-backend

# Release serve
make web-serve
```

[[docs/deployment/k8s-deployment]] — full K8s deployment guide with troubleshooting.

---

## Key Wiki Links

- [[agent-server-control-data-plane]] — Control/data-plane architecture
- [[vol-llm-runtime-crate]] — AgentRuntime resource ownership
- [[vol-agent-server-crate]] — Server implementation
- [[vol-llm-agent-protocol-crate]] — Protocol layer
- [[agent-router]] — Local multi-agent routing
- [[tool-registry]] — Tool registration framework
- [[mcp-manager-lifecycle]] — MCP lifecycle
- [[runtime-task-store-configuration]] — Task store config
- [[runtime-session-store-configuration]] — Session store config
