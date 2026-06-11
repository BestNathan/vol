# vol

A Rust workspace combining a Deribit volatility monitoring pipeline with a full LLM agent framework — ReAct orchestration, MCP integration, built-in tools, skills, TUI and web frontends.

## Architecture

The repository has two major subsystems connected by an agent advice bridge:

### Volatility Monitoring Pipeline

```
Config ──▶ DataSource ──▶ EventBus ──▶ Alert Rules ──▶ Notifications
  │         (Deribit)    (broadcast)   (iv/rate/       (Feishu,
  │                                    term/skew)       Stdout)
  └────────────────────────────────────────────────────────────┘
```

- **Event-driven**: tokio channels (mpsc + broadcast) with `TracedEvent<T>` wrappers for distributed tracing
- **Plugin traits**: `DataSource`, `RuleProcessor`, `NotificationHandler` — drop in new implementations
- **TDengine**: time-series storage for alerts, IV curves, and market data
- **OpenTelemetry + Jaeger**: full span tracing across all pipeline stages

### LLM Agent Framework

```
┌──────────────────────────────────────────────────────────────────┐
│                         Frontends                                 │
│   TUI (ratatui)  │  Web (Dioxus WASM)  │  JSON-RPC WebSocket     │
└──────────────────────────────┬───────────────────────────────────┘
                               │
┌──────────────────────────────▼───────────────────────────────────┐
│              AgentServerCore (transport + handlers)               │
│              ── wraps AgentRuntime, does NOT patch it             │
└──────────────────────────────┬───────────────────────────────────┘
                               │
┌──────────────────────────────▼───────────────────────────────────┐
│              AgentRuntime (single source of truth)                │
│  ToolRegistry  │  SkillLoader  │  McpManager  │  ProviderLoader  │
│  ALL tools registered here: builtin + task + web + skill + mcp   │
└──────────────────────────────┬───────────────────────────────────┘
                               │
┌──────────────────────────────▼───────────────────────────────────┐
│                       LLM Providers                               │
│      Anthropic  │  OpenAI  │  Custom (DashScope, local)          │
└──────────────────────────────────────────────────────────────────┘
```

- **AgentRuntime** (`vol-llm-runtime`) is the single source of truth for all shared agent state — tools, skills, MCP, providers are all assembled here. `AgentServerCore` wraps it for transport and must not patch its registries.
- **ReAct agent**: tool-calling loop with structured `AgentInput` / `AgentOutput`
- **8 built-in tools**: read, write, edit, glob, grep, bash, web-search, web-fetch
- **Skills system**: markdown-frontmatter skill definitions loaded at runtime
- **YAML agents**: define agents declaratively via YAML config
- **MCP**: Model Context Protocol client (`rmcp`) + `docs-rs-mcp` server
- **3 frontends**: Terminal UI, Web (Dioxus 0.6 WASM + Tailwind CSS), JSON-RPC WebSocket

## Project Structure

```
crates/
├── vol-core/                    Core traits and data models
├── vol-config/                  Configuration loading (TOML)
├── vol-tracing/                 Tracing utilities, TracedEvent<T>
├── vol-eventbus/                Event bus (tokio broadcast)
├── vol-deribit/                 Deribit WebSocket client & types
├── vol-datasource/              Data source implementations
├── vol-alert/                   Alert rule implementations
├── vol-rules/                   Rule processors
├── vol-notification/            Notification handlers (Feishu, Stdout)
├── vol-engine/                  Monitoring engine orchestration
├── vol-monitor/                 Main binary (vol-monitor, upload-doc, upload-ppt)
├── vol-session/                 Session management for LLM agents
│
├── vol-tdengine/                TDengine REST API client
├── vol-llm-tdengine/            TDengine tool for LLM agents
│
├── vol-llm-core/                LLM abstractions, types, traits
├── vol-llm-provider/            Provider implementations (Anthropic, OpenAI)
├── vol-llm-tool/                Tool registry framework
├── vol-llm-agent/               ReAct agent orchestration
├── vol-llm-agents/              High-level agent implementations
├── vol-llm-context/             Context/memory management
├── vol-llm-memory/              Conversation persistence
├── vol-llm-skill/               Skill system (markdown-frontmatter)
├── vol-llm-task/                Task management
├── vol-llm-runtime/             Agent runtime combining all layers
├── vol-llm-tools-builtin/       Built-in tools (read/write/edit/grep/bash/...)
├── vol-llm-yaml-agent/          YAML-defined agent config
├── vol-llm-agent-protocol/       JSON-RPC agent communication
├── vol-llm-mcp/                 MCP client (rmcp)
├── vol-llm-wiki/                Wiki/knowledge-base tool
├── vol-llm-observability/       OpenTelemetry logging for agent sessions
│
├── vol-llm-ui/                  Dual UI: TUI (ratatui) + Web (Dioxus 0.6 WASM)
├── vol-llm-tui/                 Terminal UI binary
├── vol-mcp-servers/             MCP server implementations (docs-rs-mcp)
├── vol-observability/           Observability metrics HTTP server
│
├── md-frontmatter/              Markdown frontmatter parsing library
└── ppt-agent/                   PowerPoint generation agent

docs/                            Architecture, deployment, test results, wiki
k8s/                             Kubernetes manifests and deploy scripts
openspec/                        OpenSpec change/spec artifacts
scripts/                         Dev startup, agent test, TDengine schema
```

## Quick Start

### Prerequisites

- Rust toolchain (stable)
- `wasm32-unknown-unknown` target (for web frontend)
- Node.js + npm (for Tailwind CSS)

### Run the volatility monitor

```bash
cp config.toml.example config.toml   # edit with your settings
cp .env.example .env                 # set Deribit + Feishu credentials
cargo run --release -p vol-monitor
```

### Run the web frontend

```bash
# Terminal 1: Tailwind CSS
make web-css

# Terminal 2: Dioxus dev server (port 8080)
make web-dev

# Terminal 3: JSON-RPC backend (port 3001)
make web-backend
```

### Run the TUI

```bash
cargo run --release -p vol-llm-tui
```

### Run the MCP server

```bash
cargo run --release -p vol-mcp-servers --bin docs-rs-mcp
```

## Configuration

Multiple config presets are provided:

| File | Purpose |
|------|---------|
| `config.toml` | Default (deployed via K8s ConfigMap) |
| `config.dev.toml` | Local development (short cooldowns, human-readable logs) |
| `config.prod.toml` | Production (strict thresholds, JSON logs, OTEL enabled) |
| `config.agent-test.toml` | Agent advice testing (low thresholds) |
| `config.feishu-test.toml` | Feishu notification testing |

See [docs/CONFIGURATION.md](docs/CONFIGURATION.md) for detailed configuration guide.

## Docker & Kubernetes

```bash
# Build multi-arch image (amd64 + arm64)
docker build -f Dockerfile.cross-compile -t vol-monitor .

# Deploy to K8s
cd k8s && bash deploy.sh
```

The Dockerfile uses `rsproxy.cn` as the cargo mirror. Builder stage must copy `.cargo/config.toml`.

See [k8s/README.md](k8s/README.md) for deployment instructions.

## Model Service

Default model endpoint: `http://192.168.2.162:31693`

Available models: `gpt5.5`, `coding`, `qwen3.6-plus`, `glm5.1`

Provider configuration is in `config.toml` under `[[llm_providers]]`.

## Documentation

| Path | Topic |
|------|-------|
| `docs/architecture/overview.md` | System architecture and data flow |
| `docs/architecture/crates.md` | Crate organization |
| `docs/CONFIGURATION.md` | Configuration guide |
| `docs/tracing.md` | Tracing setup |
| `docs/deployment/` | Docker build, K8s deployment, multi-arch |
| `docs/ai-agent/` | LLM agent architecture and design docs |
| `docs/wiki/` | Persistent project wiki |

## License

MIT
