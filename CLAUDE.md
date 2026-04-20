# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Quick Start

```bash
# Build
cargo build --release

# Run (after setting up .env)
source .env && ./target/release/vol-monitor --config config.dev.toml

# Deploy
./k8s/deploy.sh latest
```

## Project Structure

```
nq-deribit/
├── crates/                          # 24 workspace crates
│   │
│   ├── === Monitoring System ===
│   ├── vol-core/                    # Shared traits & data models
│   ├── vol-config/                  # TOML configuration loading
│   ├── vol-tracing/                 # Tracing utilities & span helpers
│   ├── vol-deribit/                 # Deribit WebSocket client
│   ├── vol-datasource/              # Data providers (Deribit, CSV)
│   ├── vol-alert/                   # Alert evaluation logic
│   ├── vol-rules/                   # Rule processors
│   ├── vol-notification/            # Alert delivery (stdout, Feishu)
│   ├── vol-engine/                  # Monitoring engine orchestration
│   ├── vol-monitor/                 # Main binary
│   ├── vol-eventbus/                # Event bus for inter-component communication
│   ├── vol-tdengine/                # TDengine time-series database client
│   │
│   ├── === LLM Agent System ===
│   ├── vol-llm-core/                # LLM abstractions: client, message, conversation, tool, sandbox
│   ├── vol-llm-provider/            # LLM provider implementations (Anthropic via DashScope)
│   ├── vol-llm-tool/                # Tool system: registry, sensitivity, proxy config
│   ├── vol-llm-agent/               # ReAct Agent core: plugin system, HITL, observability, RAG, embeddings
│   ├── vol-llm-agents/              # Specialized agents: CodingAgent, AdviceAgent, QaAgent, PptAgent
│   ├── vol-llm-tools-builtin/       # Built-in tools: read, write, edit, bash, glob, grep, web_fetch, web_search
│   ├── vol-llm-tui/                 # Full-screen TUI with ratatui: tabs, conversation, workspace, HITL approval
│   ├── vol-llm-tdengine/            # TDengine tools for LLM agent queries
│   ├── vol-llm-observability/       # Agent observability: RunLogLogger, observability plugin
│   ├── vol-llm-memory/              # Cross-session memory: Store/Retriever traits, MemoryManager
│   ├── vol-session/                 # Session management & message persistence (JSONL)
│   │
│   └── ppt-agent/                   # PPT generation agent (uses lark-whiteboard)
│
├── k8s/                             # Kubernetes manifests
├── docs/                            # Documentation (see index below)
└── .cargo/config.toml               # Registry mirror (rsproxy.cn)
```

## Architecture Overview

### Monitoring System

| Crate | Purpose |
|-------|---------|
| `vol-core` | Shared traits (`DataSource`, `AlertHandler`, `NotificationHandler`) |
| `vol-config` | TOML-based configuration loading |
| `vol-tracing` | Tracing utilities, `TracedEvent<T>`, span helpers |
| `vol-deribit` | Deribit client: WebSocket, JSON-RPC, market data types |
| `vol-datasource` | DataSource trait implementation using vol-deribit |
| `vol-alert` | Alert evaluation logic |
| `vol-rules` | Rule processors |
| `vol-notification` | Alert delivery (stdout, Feishu webhook) |
| `vol-engine` | Monitoring engine orchestration |
| `vol-monitor` | Main binary - wires everything together |

**Data Flow:**
```
Deribit WebSocket → DataSource → mpsc → MonitoringEngine → Rules → Notifications
```

### LLM Agent System

| Crate | Purpose |
|-------|---------|
| `vol-llm-core` | Core abstractions: `LLMClient`, `Message`, `Conversation`, `Sandbox` |
| `vol-llm-provider` | Provider implementations (Anthropic via DashScope Qwen) |
| `vol-llm-tool` | Tool system: `ExecutableTool`, `ToolRegistry`, `ToolSensitivity`, proxy config |
| `vol-llm-agent` | ReAct Agent loop, plugin system, HITL approval, observability, session management |
| `vol-llm-agents` | Specialized agents: `CodingAgent`, `AdviceAgent`, `QaAgent`, `PptAgent` |
| `vol-llm-tools-builtin` | Built-in tools: read, write, edit, bash, glob, grep, web_fetch, web_search |
| `vol-llm-tui` | Full-screen ratatui TUI with Conversation/Workspace tabs, input area, status bar, HITL approval, unsafe mode |
| `vol-llm-memory` | Cross-session memory: `MemoryStore`/`MemoryRetriever` traits, `InMemoryStore`, `KeywordRetriever`, `MemoryManager` |
| `vol-llm-observability` | Agent observability: `RunLogLogger`, `ObservabilityPlugin` for structured run logs |
| `vol-session` | Session lifecycle, `SessionListener`, `FileMessageStore` (JSONL persistence) |

**Agent Data Flow:**
```
User Input → ReActAgent.run() → LLM → Tool Call → Sandbox → Tool Execute → LLM → ... → Final Answer
                                    │
                              PluginRegistry (intercept/listen hooks)
                                    │
                              SessionListener → FileMessageStore (session JSONL)
                              ObservabilityPlugin → RunLogLogger (run events)
```

**Key Patterns:**
- Trait-based plugin architecture (vol-core traits)
- Async-first (tokio, no blocking I/O)
- Channel-based communication (mpsc, broadcast)
- Trace context propagation (`TracedEvent<T>`)
- ReAct loop with tool registry and sandbox isolation
- Plugin flow intervention: `intercept()` before tool execution, `listen()` after events
- HITL approval via dedicated approval channel in `RunContext`
- Session persistence in JSONL format via `FileMessageStore`
- Layered memory architecture: `MemoryStore` (CRUD trait) and `MemoryRetriever` (relevance search trait) are separate, allowing swappable implementations

See [docs/architecture/](docs/architecture/) for monitoring architecture and [docs/ai-agent/](docs/ai-agent/) for LLM agent documentation.

## Common Commands

```bash
# Build & Check
cargo build --release
cargo check --workspace

# Run
./target/release/vol-monitor --config config.dev.toml
RUST_LOG=info ./target/release/vol-monitor

# Test
cargo test --workspace

# Docker
docker build -t crpi-ck06yio90i1ttwlz.cn-beijing.personal.cr.aliyuncs.com/n_common/vol-monitor:latest .
docker push crpi-ck06yio90i1ttwlz.cn-beijing.personal.cr.aliyuncs.com/n_common/vol-monitor:latest

# Kubernetes
kubectl -n deribit logs -f deployment/vol-monitor
kubectl -n deribit rollout restart deployment/vol-monitor
kubectl -n deribit rollout undo deployment/vol-monitor
```

## Documentation Index

### Monitoring System
| Category | File | Description |
|----------|------|-------------|
| **Architecture** | [docs/architecture/overview.md](docs/architecture/overview.md) | System architecture, data flow diagrams |
| | [docs/architecture/crates.md](docs/architecture/crates.md) | Detailed crate documentation |
| **Deployment** | [docs/deployment/docker-build.md](docs/deployment/docker-build.md) | Docker build, multi-arch, ACR registry |
| | [docs/deployment/k8s-deployment.md](docs/deployment/k8s-deployment.md) | Kubernetes deployment, secrets, troubleshooting |
| **Integration** | [docs/integration/deribit.md](docs/integration/deribit.md) | Deribit API, WebSocket, proxy support |
| | [docs/integration/tdengine.md](docs/integration/tdengine.md) | TDengine integration guide |
| **Development** | [docs/development/common-modifications.md](docs/development/common-modifications.md) | Adding alerts, datasources, notifications |
| **Configuration** | [docs/CONFIGURATION.md](docs/CONFIGURATION.md) | Config file structure, environment variables |
| **Tracing** | [docs/tracing.md](docs/tracing.md) | Logging, Jaeger, trace context |

### LLM Agent System
| Category | File | Description |
|----------|------|-------------|
| **Architecture** | [docs/ai-agent/01-llm-client-architecture.md](docs/ai-agent/01-llm-client-architecture.md) | LLM client design, provider abstraction |
| | [docs/ai-agent/02-protocol-design.md](docs/ai-agent/02-protocol-design.md) | Protocol details |
| | [docs/ai-agent/03-agent-tool-design.md](docs/ai-agent/03-agent-tool-design.md) | Tool system design |
| | [docs/ai-agent/04-memory-rag-design.md](docs/ai-agent/04-memory-rag-design.md) | Memory & RAG |
| | [docs/ai-agent/05-implementation-plan.md](docs/ai-agent/05-implementation-plan.md) | Implementation roadmap |
| **Specialized Agents** | [docs/ai-agent/rag-agent-design.md](docs/ai-agent/rag-agent-design.md) | RAG agent design |
| | [docs/ai-agent/react-plugin-system.md](docs/ai-agent/react-plugin-system.md) | ReAct Agent plugin system |
| | [docs/ai-agent/06-observability-plugin.md](docs/ai-agent/06-observability-plugin.md) | Observability logging |

## Configuration

Credentials via environment variables (never commit secrets):

| Variable | Purpose |
|----------|---------|
| `DERIBIT_CLIENT_ID` | Deribit API client ID |
| `DERIBIT_CLIENT_SECRET` | Deribit API client secret |
| `FEISHU_APP_ID` | Feishu app ID |
| `FEISHU_APP_SECRET` | Feishu app secret |
| `FEISHU_RECEIVE_ID` | Feishu recipient ID |
| `HTTPS_PROXY` | HTTP proxy (restricted dev environment) |
| `ANTHROPIC_AUTH_TOKEN` | LLM API key (Alibaba Cloud DashScope) |

```bash
# Quick start
cp .env.example .env && vim .env
./scripts/run-dev.sh dev
```

### Important Configuration Notes

**1. Deribit WebSocket URL**

Only use the production Deribit environment:

```toml
[clients.deribit]
ws_url = "wss://www.deribit.com/ws/api/v2"
```

Do NOT use the test environment (`test.deribit.com`).

**2. LLM Provider Configuration**

Use Anthropic provider with Alibaba Cloud DashScope Qwen model:

```toml
[[llm_providers]]
id = "anthropic-main"
provider = "anthropic"
model = "qwen3.5-plus"
api_key = "${ANTHROPIC_AUTH_TOKEN}"
base_url = "https://coding.dashscope.aliyuncs.com/apps/anthropic"
```

- **Provider**: `anthropic` (do NOT create a separate `qwen` provider)
- **Model**: `qwen3.5-plus`
- **base_url**: Keep as `https://coding.dashscope.aliyuncs.com/apps/anthropic`

**3. User-Agent Configuration**

The DashScope coding endpoint (`https://coding.dashscope.aliyuncs.com/apps/anthropic`) requires requests to come from a "Coding Agent". The Anthropic provider in `crates/vol-llm-provider/src/anthropic.rs` sends a Claude Code User-Agent header:

```rust
.header("User-Agent", "claude-code/1.0.0")
```

This mimics the Claude Code CLI client pattern, which is accepted by DashScope's coding endpoint.

**4. Proxy Configuration**

When using HTTP proxy, add DashScope domains to NO_PROXY to avoid connection issues:

```bash
NO_PROXY="localhost,127.0.0.1,192.168.0.0/16,10.0.0.0/8,kubernetes.default.svc,*.aliyuncs.com,dashscope.aliyuncs.com"
```

**5. LLM Agent TUI**

The `vol-llm-tui` crate provides a full-screen ratatui-based interface for coding agent sessions:

```bash
cargo build -p vol-llm-tui
source .env && ./target/debug/vol-llm-tui
```

Features:
- **Conversation tab**: Scrollable message view with color-coded events (user input, thinking, tool calls, results, agent answers)
- **Workspace tab**: File browser for exploring project files
- **Input area**: Multi-line text input with Ctrl+Enter to submit
- **HITL approval**: Dangerous tool calls (e.g., bash commands) trigger inline approval prompts
- **Unsafe mode** (Ctrl+U): Toggle to auto-approve all tool calls
- **Status bar**: Shows agent state (Idle/Running/Waiting for approval), time, crate count

Commands: `/quit`, `/exit`, `/help`, `/clear`, `/unsafe`. Requires `ANTHROPIC_AUTH_TOKEN`.

**6. Agent Memory System**

The `vol-llm-memory` crate provides cross-session memory for agents. `MemoryManager` is optional in `AgentConfig`:

```rust
pub struct AgentConfig {
    // ... existing fields ...
    pub memory: Option<Arc<MemoryManager>>,
}
```

When present, the agent can inject relevant memories at session startup (e.g., user preferences, project facts, past experiences) into the system prompt. Memory extraction from sessions (`summarize_session`) is currently a stub — LLM-based extraction is not yet implemented.

See [docs/CONFIGURATION.md](docs/CONFIGURATION.md) for full configuration guide.

## Kubernetes Deployment

| Resource | Name | Namespace |
|----------|------|-----------|
| Namespace | `deribit` | - |
| Deployment | `vol-monitor` | `deribit` |
| ConfigMap | `vol-monitor-config` | `deribit` |
| Secret | `vol-monitor-secrets` | `deribit` |

**Create secrets:**
```bash
kubectl create secret generic vol-monitor-secrets \
  --from-literal=DERIBIT_CLIENT_ID=<id> \
  --from-literal=DERIBIT_CLIENT_SECRET=<secret> \
  --from-literal=FEISHU_APP_ID=<app-id> \
  --from-literal=FEISHU_APP_SECRET=<app-secret> \
  --from-literal=FEISHU_RECEIVE_ID=<receive-id> \
  -n deribit
```

See [docs/deployment/k8s-deployment.md](docs/deployment/k8s-deployment.md) for complete deployment guide.

## Convension

### Feishu(lark)

- **Wiki SpaceID** for this project is **7630485291026910436**
- after *spec* or *plan* create, upload the doc to project wiki

#### Commands

```bash

# create wiki doc
lark-cli docs +create \
    --title "{title}" \
    --markdown "$(cat path/to/markdown.md)" \
    --wiki-space "{wiki space id}" \
    --as user
```
