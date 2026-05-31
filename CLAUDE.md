# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with this repository.

## Docker + Rust Build Configuration

All Docker-based Rust builds must use rsproxy as the mirror source. The build environment
cannot access crates.io directly.

### Environment Variables (Dockerfile builder stage)

```dockerfile
ENV RUSTUP_DIST_SERVER=https://rsproxy.cn \
    RUSTUP_UPDATE_ROOT=https://rsproxy.cn/rustup \
    RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH
```

### Rust Installation

```dockerfile
RUN curl --proto '=https' --tlsv1.2 -sSf https://rsproxy.cn/rustup-init.sh | sh -s -- -y
```

### Cargo Mirror Config (`.cargo/config.toml`)

Must be copied into the builder stage. Contains:
```toml
[source.crates-io]
replace-with = 'rsproxy-sparse'
[source.rsproxy]
registry = "https://rsproxy.cn/crates.io-index"
[source.rsproxy-sparse]
registry = "sparse+https://rsproxy.cn/index/"
[registries.rsproxy]
index = "https://rsproxy.cn/crates.io-index"
[net]
git-fetch-with-cli = true
```

## Project Overview

This repository is a Rust Cargo workspace for Deribit volatility monitoring and LLM agent tooling. The original monitoring pipeline is event-driven: configuration feeds data sources, data sources publish through the event bus, alert handlers evaluate market conditions, and notification handlers deliver alerts.

### AgentRuntime — Single Source of Truth

`vol-llm-runtime` (`AgentRuntime`) is the **authoritative owner** of all shared agent resources. It is the single place where tools, skills, MCP, and providers are assembled. `AgentServerCore` wraps it for transport — it does NOT patch or extend the resource set.

**Tool registration rules:**

| Location | Role | Should register tools? |
|----------|------|------------------------|
| `AgentRuntimeBuilder::build()` | **Primary** — all tools registered here | YES — builtin, task, web, **skill**, and future tools |
| `AgentServerCoreBuilder::build()` | Transport wrapper — inherits runtime's registry as-is | NO — must not clone/patch the registry |
| `AgentConfigBuilder::build()` | Standalone path (TUI, tests) — mirrors the same set | YES — but only for callers that bypass Runtime |

When adding a new tool or resource that should be available to all agents, register it in `AgentRuntimeBuilder::build()` (`crates/vol-llm-runtime/src/lib.rs`). The `AgentServerCore` path will pick it up automatically.

### Main Directories

| Path | Purpose |
|------|---------|
| `crates/` | Workspace crates. The `vol-*` crates implement the Deribit volatility monitor, while `vol-llm-*` crates implement LLM providers, agents, tools, memory, skills, MCP, TUI, and web UI layers. |
| `crates/vol-core` | Core monitoring traits and data models shared by the volatility pipeline. |
| `crates/vol-config` | Configuration loading and typed settings used by services. |
| `crates/vol-datasource`, `crates/vol-deribit`, `crates/vol-tdengine` | Market-data ingestion and storage integrations. |
| `crates/vol-eventbus`, `crates/vol-engine`, `crates/vol-alert`, `crates/vol-notification`, `crates/vol-monitor` | Runtime pipeline: event distribution, monitoring engine, alert rules, notification delivery, and main monitor binary. |
| `crates/vol-llm-core`, `crates/vol-llm-provider`, `crates/vol-llm-tool`, `crates/vol-llm-agent`, `crates/vol-llm-agents` | LLM abstraction layer, provider implementations, tool registry, ReAct orchestration, and higher-level agent implementations. |
| `crates/vol-llm-agent-channel`, `crates/vol-llm-mcp`, `crates/vol-mcp-servers` | Agent communication, JSON-RPC/MCP integration, and MCP server implementations. |
| `crates/vol-llm-ui` | Dioxus 0.6 WASM web frontend. Use the web Makefile commands rather than generic Cargo commands for this crate. |
| `crates/vol-llm-tui` | Terminal UI frontend for the LLM agent experience. |
| `docs/` | Architecture, deployment, development notes, migrations, test results, superpowers docs, and the persistent project wiki at `docs/wiki`. |
| `openspec/` | OpenSpec change/spec artifacts. |
| `k8s/` | Kubernetes manifests. |
| `scripts/` | Repository automation scripts. |
| `.cargo/` | Cargo mirror configuration; Docker Rust builds must copy this config. |

## Development

### Web Frontend

**`vol-llm-ui`** (`crates/vol-llm-ui`) is the web frontend crate. It contains the Dioxus 0.6 WASM app with components, Tailwind CSS, and web-specific state management. All web-related code lives under this crate.

**When developing or running the web frontend**, you must use the web-related Makefile commands — do NOT run generic `cargo build` or `cargo run` commands as they will not compile the WASM binary or serve the frontend.

#### Required Web Development Tools

Install these before starting web development:

| Tool | Purpose | Install / verify |
|------|---------|------------------|
| Rust + Cargo | Build the Dioxus WASM app and backend service | `cargo --version` |
| `wasm32-unknown-unknown` target | Build the web frontend WASM binary | `rustup target add wasm32-unknown-unknown` |
| Dioxus CLI (`dx`) 0.6.x | Serve the Dioxus 0.6 web app on port 8080 | `cargo install dioxus-cli --version 0.6.3 --locked` |
| `cargo-watch` | Auto-rebuild and restart the JSON-RPC backend | `cargo install cargo-watch --locked` |
| Node.js + npm | Run Tailwind CLI and manage web CSS dependencies | `node --version && npm --version` |
| `crates/vol-llm-ui` npm dependencies | Provide `tailwindcss` for CSS compilation | `npm ci --prefix crates/vol-llm-ui` |

Pre-flight checks:

```bash
which dx
cargo watch --version
npm ci --prefix crates/vol-llm-ui
lsof -i :8080 2>/dev/null || true
lsof -i :3001 2>/dev/null || true
```

All web development commands use the Makefile. Run `make help` to see available commands:

| Command | Description |
|---------|-------------|
| `make web-css` | Build Tailwind CSS in watch mode |
| `make web-dev` | Start Dioxus dev server (port 8080) |
| `make web-backend` | Start backend JSON-RPC agent service from `vol-llm-agent-channel` (port 3001) |
| `make web-check` | cargo check (web only) |
| `make web-build` | Build WASM binary |
| `make web-clippy` | cargo clippy (web only) |

**Starting web development requires running 3 services in separate terminals:**

1. `make web-css` — compile Tailwind CSS in persistent watch mode.
2. `make web-dev` — start Dioxus dev server on port 8080.
3. `make web-backend` — start backend JSON-RPC agent service on port 3001.

The `make web-css` target runs:

```bash
npx --prefix crates/vol-llm-ui @tailwindcss/cli \
  -i crates/vol-llm-ui/assets/input.css \
  -o crates/vol-llm-ui/assets/tailwind.css \
  --watch=always
```

**Important:** Tailwind CSS must be compiled before `make web-dev`; otherwise new Tailwind utility classes (e.g., arbitrary values like `w-[600px]`, `h-[70vh]`) won't be present in `assets/tailwind.css` and won't take effect.

If `make web-dev` serves `Err 404 - dioxus is not currently serving a web app`, restart it explicitly with the web platform:

```bash
dx serve --platform web --package vol-llm-ui --bin vol-llm-ui-web \
  --no-default-features --features web --addr 0.0.0.0 --port 8080
```

### Model Service

The model service is available at:

- Base URL: `http://192.168.2.162:31693`
- Models: `gpt5.5`, `coding`, `qwen3.6-plus`, `glm5.1`

## Conventions

- When finished a development task, you **MUST** use skill `wiki-ingest` to add or update project wiki at `docs/wiki`

- When `docs/superpowers/*` add or update docs you **MUST** upload the doc to lark wiki space **7630485291026910436**
```bash
# create wiki doc
lark-cli docs +create \
    --title "{title}" \
    --markdown "$(cat path/to/markdown.md)" \
    --wiki-space "{wiki space id}" \
    --as user
```

## Feishu Docs

- When `superpowers` skill writing a doc into `docs/superpowers/*`, you **MUST** upload it to feishu docs with `lark-cli`
- `docs/superpowers/plans/*`: wiki node id is **TEkkw1W6niuBxQkcvswchOo5nhb**
- `docs/superpowers/requirement/*`: wiki node id is **PPDZw7LFqiFjMTkAXFocFoO6nce**
- `docs/superpowers/specs/*`: wiki node id is **Og7twpiPoi0Vbjk2EzvcqX92nsb**


```sh
# lark-cli to upload docs to feishu
lark-cli docs +create \
    --title "{title}" \
    --markdown "$(cat path/to/markdown.md)" \
    --wiki-node "{wiki node id}"

# lark-cli to update docs to feishu, the token is the last part of url
# e.g: https://my.feishu.cn/wiki/PPDZw7LFqiFjMTkAXFocFoO6nce => token=**PPDZw7LFqiFjMTkAXFocFoO6nce**
lark-cli docs +update \
    --new-title "{title}" \
    --mode overwrite \
    --markdown "$(cat path/to/markdown.md)" \
    --doc "{doc url or token}"
```