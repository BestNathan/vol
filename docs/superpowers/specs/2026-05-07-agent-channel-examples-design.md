# Agent Channel Examples: WS + HTTP Service Design

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add example applications to `vol-llm-agent-channel/examples/` demonstrating how to build a service with WebSocket and HTTP endpoints using the channel protocol primitives.

**Architecture:** Two example files (`single_agent.rs`, `multi_agent.rs`) that wire `AgentDispatcher`, `ConnectionHolder`, `WsServer`, `HttpTransport`, and `AgentRouter` into runnable axum services backed by real LLM providers.

**Tech Stack:** Rust, axum, tokio, vol-llm-agent-channel, vol-llm-provider, tracing

---

## 1. Dev Dependencies

**Files:**
- Modify: `crates/vol-llm-agent-channel/Cargo.toml`

Add dev-dependencies needed by the examples:

```toml
[dev-dependencies]
# ... existing ...
tracing-subscriber = { workspace = true }
vol-llm-provider = { path = "../vol-llm-provider" }
```

- [ ] **Step: Add dev-dependencies to Cargo.toml**

Run: `cargo check -p vol-llm-agent-channel --all-targets`
Expected: compiles cleanly

---

## 2. Example 1: Single Agent Service (`single_agent.rs`)

**Files:**
- Create: `crates/vol-llm-agent-channel/examples/single_agent.rs`

A single `ReActAgent` with dual transport (WS + HTTP) behind one axum server.

### Architecture

```
┌─────────────────────────────────────────────────────┐
│                    axum Router                       │
│                                                      │
│  /ws           ──► WsServer                         │
│  /api/chat     ──► HttpTransport (blocking + SSE)   │
│  /health       ──► JSON {"status": "ok"}            │
│                                                      │
│  Both transports share:                              │
│    AgentDispatcher  (single ReActAgent)             │
│    ConnectionHolder ("my-agent" ↔ "client")         │
└─────────────────────────────────────────────────────┘
```

### Key Code Structure

```rust
use vol_llm_agent_channel::{AgentDispatcher, ConnectionHolder, WsServer, HttpTransport};
use vol_llm_agent::agent_def::AgentDef;
use vol_llm_agent::react::{AgentConfig, PluginRegistry, ReActAgent};
use vol_llm_provider::create_provider;
use vol_session::{InMemoryEntryStore, Session};
use vol_llm_tool::ToolRegistry;
use vol_llm_context::ContextBuilderBuilder;
```

1. **Init tracing** with `tracing_subscriber` and `RUST_LOG` env filter
2. **Create LLM provider** from env vars (DashScope Anthropic endpoint)
3. **Build ReActAgent** with `AgentDef`, session, empty tool registry, context builder
4. **Create shared primitives**: `AgentDispatcher::new(agent)`, `ConnectionHolder::new("my-agent", "client")`
5. **Build WS router**: `WsServer::new(dispatcher.clone(), holder.clone(), "my-agent").into_axum_router()`
6. **Build HTTP router**: `HttpTransport::new(dispatcher, holder, "my-agent").into_axum_router()`
7. **Combine routers** into a single `axum::Router` with `.merge()`
8. **Add `/health`** endpoint returning `{"status": "ok"}`
9. **Serve** on `0.0.0.0:3000` with `axum::serve()`

### Environment Variables

The example reads:
- `ANTHROPIC_AUTH_TOKEN` (required) — API key for DashScope/Claude
- `RUST_LOG` (optional) — tracing log level, default `info`

### LLM Config

```rust
let llm = create_provider(&vol_llm_provider::LLMConfig::with_env_key(
    vol_llm_core::LLMProvider::Anthropic,
    "claude-sonnet-4-6",
    "ANTHROPIC_AUTH_TOKEN",
    "https://coding.dashscope.aliyuncs.com/apps/anthropic",
)).expect("failed to create LLM provider");
```

### Agent Def

```rust
let def = AgentDef::new(
    "general-assistant",
    "You are a helpful AI assistant. Answer questions concisely.",
).with_type("general-assistant");
```

### How to Run

```bash
ANTHROPIC_AUTH_TOKEN=your_key RUST_LOG=info cargo run --example single_agent -p vol-llm-agent-channel
```

### Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Health check, returns `{"status":"ok"}` |
| GET (WS) | `/ws` | WebSocket connection. Client sends `Message::Submit` JSON, receives `Message::Result` or `Message::Error` |
| POST | `/api/chat` | HTTP chat endpoint. Body: `{"input": "hello"}`. Returns JSON result |
| POST | `/api/chat?stream=true` | Same as POST but with SSE streaming of intermediate events |

### Usage Examples

**WebSocket (via wscat or similar):**
```bash
# Connect
wscat -c ws://localhost:3000/ws

# Send a message (JSON)
{"type": "submit", "req_id": "req-1", "sender": "client", "receiver": "my-agent", "input": "What is 2+2?"}

# Receive result
{"type": "result", "req_id": "req-1", "sender": "my-agent", "receiver": "client", "result": {...}}
```

**HTTP:**
```bash
curl -X POST http://localhost:3000/api/chat \
  -H "Content-Type: application/json" \
  -d '{"input": "What is 2+2?"}'
```

**HTTP SSE:**
```bash
curl -N -X POST 'http://localhost:3000/api/chat?stream=true' \
  -H "Content-Type: application/json" \
  -d '{"input": "What is 2+2?"}'
```

- [ ] **Step: Create single_agent.rs with the above structure**
- [ ] **Step: Test compilation** — `cargo build --example single_agent -p vol-llm-agent-channel`
- [ ] **Step: Verify it runs** — `cargo run --example single_agent -p vol-llm-agent-channel` (manual WS/HTTP test)

---

## 3. Example 2: Multi-Agent Router (`multi_agent.rs`)

**Files:**
- Create: `crates/vol-llm-agent-channel/examples/multi_agent.rs`

Multiple `ReActAgent` instances registered with `AgentRouter`, each accessible via path parameter.

### Architecture

```
┌───────────────────────────────────────────────────────┐
│                    axum Router                         │
│                                                         │
│  /ws/:agent_id      ──► connect to specific agent     │
│  /api/chat/:agent_id ──► POST to specific agent       │
│  /api/agents        ──► GET list of registered agents │
│  /health            ──► JSON {"status": "ok"}         │
│                                                         │
│  Each agent has its own:                               │
│    ReActAgent  (different system prompts)             │
│    AgentDispatcher                                    │
│    ConnectionHolder                                   │
│                                                         │
│  AgentRouter maps agent_id → Dispatcher               │
└───────────────────────────────────────────────────────┘
```

### Registered Agents

| Agent ID | Type | System Prompt |
|----------|------|---------------|
| `translator` | translator | "You are a translation assistant. Translate the input to English." |
| `summarizer` | summarizer | "You are a summarization assistant. Provide a brief summary." |
| `coder` | coder | "You are a coding assistant. Help with programming questions." |

### Key Code Structure

```rust
use vol_llm_agent_channel::{AgentDispatcher, AgentRouter, ConnectionHolder};
```

1. **Init tracing**
2. **Create shared LLM provider** (one provider, three agents)
3. **Build three ReActAgents** with different `AgentDef` system prompts
4. **Each agent gets** its own `AgentDispatcher` and `ConnectionHolder`
5. **Register all in `AgentRouter`**: `router.register("translator", dispatcher1)` etc.
6. **Custom WS handler** extracts `agent_id` from path, looks up dispatcher from router
7. **Custom HTTP handler** same pattern — path param → router → dispatcher
8. **GET `/api/agents`** returns JSON list of registered agent types
9. **Serve** on `0.0.0.0:3000`

### WS Handler for Multi-Agent

```rust
async fn handle_agent_ws(
    Path(agent_id): Path<String>,
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    match state.router.get(&agent_id) {
        Some((dispatcher, holder)) => {
            ws.on_upgrade(move |socket| {
                // Build WsConnection with the specific agent's dispatcher/holder
                let conn = WsConnection::new(socket, dispatcher, holder, agent_id.clone());
                conn.run().await
            })
        }
        None => (StatusCode::NOT_FOUND, "agent not found").into_response(),
    }
}
```

### How to Run

```bash
ANTHROPIC_AUTH_TOKEN=your_key RUST_LOG=info cargo run --example multi_agent -p vol-llm-agent-channel
```

### Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Health check |
| GET | `/api/agents` | List registered agents: `{"agents": ["translator", "summarizer", "coder"]}` |
| GET (WS) | `/ws/:agent_id` | WebSocket to specific agent |
| POST | `/api/chat/:agent_id` | HTTP POST to specific agent |

### Usage Examples

```bash
# List agents
curl http://localhost:3000/api/agents

# Talk to translator
curl -X POST http://localhost:3000/api/chat/translator \
  -H "Content-Type: application/json" \
  -d '{"input": "你好世界"}'

# Connect to coder via WebSocket
wscat -c ws://localhost:3000/ws/coder
```

- [ ] **Step: Create multi_agent.rs with the above structure**
- [ ] **Step: Test compilation** — `cargo build --example multi_agent -p vol-llm-agent-channel`
- [ ] **Step: Verify it runs** — `cargo run --example multi_agent -p vol-llm-agent-channel`

---

## 4. Error Handling

Both examples follow these error handling patterns:

- **LLM creation failure**: `expect()` with descriptive message — service cannot start without LLM
- **Agent not found (multi-agent)**: HTTP 404 with `"agent not found"` message
- **Agent execution error**: Returned as `Message::Error` (WS) or HTTP 500 JSON `{"error": "..."}`
- **Client disconnect**: WS connection loop breaks on `None`/`Err`, `ConnectionHolder.detach()` called
- **Concurrent SSE (HTTP)**: `ConnectionHolder.is_connected()` check → 409 Conflict
- **Panic recovery**: No special handling — let tokio task panic propagate, service continues for other connections

---

## 5. Testing Strategy

The examples are integration demos, not unit-tested code. Verification:

1. **Compilation**: `cargo build --example <name> -p vol-llm-agent-channel` must succeed
2. **Manual testing**: Run with real API key, test each endpoint as documented
3. **No CI dependency**: Examples don't run in CI (require real API key), but must compile in CI

---

## Notes

- No authentication middleware — examples are intentionally open for simplicity
- No configuration files — all config via environment variables and inline constants
- No metrics or health beyond basic `/health` endpoint
- Each example is self-contained in a single file
