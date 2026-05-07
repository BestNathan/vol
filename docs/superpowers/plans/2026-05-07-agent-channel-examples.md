# Agent Channel WS + HTTP Examples Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add two example applications to `vol-llm-agent-channel/examples/` demonstrating how to build services with WebSocket and HTTP endpoints using the channel protocol primitives.

**Architecture:** Two self-contained example files — `single_agent.rs` (one agent, dual transport) and `multi_agent.rs` (multiple agents behind AgentRouter) — wired from existing `WsServer`, `HttpTransport`, `AgentDispatcher`, `ConnectionHolder`, and `AgentRouter` into runnable axum services.

**Tech Stack:** Rust, axum, tokio, tracing, vol-llm-agent-channel, vol-llm-provider, vol-llm-agent

---

### Task 1: Add dev-dependencies to Cargo.toml

**Files:**
- Modify: `crates/vol-llm-agent-channel/Cargo.toml`

Add `tracing-subscriber` and `vol-llm-provider` as dev-dependencies so the examples can compile.

- [ ] **Step 1: Read current Cargo.toml**

Read `crates/vol-llm-agent-channel/Cargo.toml` to find the existing `[dev-dependencies]` section.

- [ ] **Step 2: Add two dev-dependencies**

Append these lines to the `[dev-dependencies]` section:

```toml
tracing-subscriber = { workspace = true }
vol-llm-provider = { path = "../vol-llm-provider" }
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p vol-llm-agent-channel --all-targets`
Expected: compiles cleanly (no new errors)

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent-channel/Cargo.toml
git commit -m "feat: add tracing-subscriber and vol-llm-provider as channel dev-deps"
```

---

### Task 2: Create single_agent.rs example

**Files:**
- Create: `crates/vol-llm-agent-channel/examples/single_agent.rs`

A single `ReActAgent` with dual transport (WS + HTTP) behind one axum server on port 3000.

### Architecture for this file

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

### Imports

The file needs these imports:

```rust
use std::sync::Arc;

use axum::routing::get;
use axum::{Json, Router};
use tokio::net::TcpListener;
use tracing::info;
use vol_llm_agent::agent_def::AgentDef;
use vol_llm_agent::react::{AgentConfig, PluginRegistry, ReActAgent};
use vol_llm_agent_channel::{AgentDispatcher, ConnectionHolder, HttpTransport, WsServer};
use vol_llm_context::ContextBuilderBuilder;
use vol_llm_provider::create_provider;
use vol_llm_tool::ToolRegistry;
use vol_session::{InMemoryEntryStore, Session};
```

- [ ] **Step 1: Write the file**

Create `crates/vol-llm-agent-channel/examples/single_agent.rs` with this complete content:

```rust
//! Single-agent service with WebSocket and HTTP endpoints.
//!
//! Run with:
//! ```bash
//! ANTHROPIC_AUTH_TOKEN=your_key RUST_LOG=info \
//!   cargo run --example single_agent -p vol-llm-agent-channel
//! ```
//!
//! Endpoints:
//! - `GET /ws` — WebSocket upgrade for bidirectional chat
//! - `POST /api/chat` — HTTP POST with `{"input": "..."}`, returns JSON result
//! - `POST /api/chat?stream=true` — Same as POST but with SSE event streaming
//! - `GET /health` — Health check

use std::sync::Arc;

use axum::routing::get;
use axum::{Json, Router};
use tokio::net::TcpListener;
use tracing::info;
use vol_llm_agent::agent_def::AgentDef;
use vol_llm_agent::react::{AgentConfig, PluginRegistry, ReActAgent};
use vol_llm_agent_channel::{AgentDispatcher, ConnectionHolder, HttpTransport, WsServer};
use vol_llm_context::ContextBuilderBuilder;
use vol_llm_provider::create_provider;
use vol_llm_tool::ToolRegistry;
use vol_session::{InMemoryEntryStore, Session};

#[tokio::main]
async fn main() {
    // Init tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Create LLM provider from env
    let llm = create_provider(&vol_llm_provider::LLMConfig::with_env_key(
        vol_llm_core::LLMProvider::Anthropic,
        "claude-sonnet-4-6",
        "ANTHROPIC_AUTH_TOKEN",
        "https://coding.dashscope.aliyuncs.com/apps/anthropic",
    ))
    .expect("failed to create LLM provider — set ANTHROPIC_AUTH_TOKEN");

    info!(model = "claude-sonnet-4-6", "LLM provider created");

    // Build agent
    let def = AgentDef::new(
        "general-assistant",
        "You are a helpful AI assistant. Answer questions concisely.",
    )
    .with_type("general-assistant");

    let session = Arc::new(Session::new(Arc::new(InMemoryEntryStore::new())));
    let tools = Arc::new(ToolRegistry::new());
    let context_builder = ContextBuilderBuilder::new(128_000).build();
    let config = AgentConfig {
        def: Some(def),
        llm: Arc::from(llm),
        tools,
        session,
        sandbox: None,
        context_builder,
        plugin_registry: PluginRegistry::new(),
    };
    let agent = ReActAgent::new(config);

    // Shared primitives
    let dispatcher = Arc::new(AgentDispatcher::new(agent));
    let holder = Arc::new(ConnectionHolder::new("my-agent".to_string(), "client".to_string()));

    // Build routers
    let ws_router = WsServer::new(dispatcher.clone(), holder.clone(), "my-agent").into_axum_router();
    let http_router = HttpTransport::new(dispatcher, holder, "my-agent").into_axum_router();

    // Combine
    let app = Router::new()
        .route("/health", get(|| async { Json(serde_json::json!({"status": "ok"})) }))
        .merge(ws_router)
        .merge(http_router);

    info!("Starting server on 0.0.0.0:3000");
    info!("  WS:   ws://localhost:3000/ws");
    info!("  HTTP: POST http://localhost:3000/api/chat");
    info!("  SSE:  POST http://localhost:3000/api/chat?stream=true");

    let listener = TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("failed to bind to 0.0.0.0:3000");

    axum::serve(listener, app)
        .await
        .expect("server failed");
}
```

- [ ] **Step 2: Test compilation**

Run: `cargo build --example single_agent -p vol-llm-agent-channel`
Expected: compiles without errors

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent-channel/examples/single_agent.rs
git commit -m "feat: add single_agent example with WS + HTTP endpoints"
```

---

### Task 3: Create multi_agent.rs example

**Files:**
- Create: `crates/vol-llm-agent-channel/examples/multi_agent.rs`

Multiple `ReActAgent` instances (translator, summarizer, coder), each with its own dispatcher and holder, routed by `AgentRouter`. Custom WS and HTTP handlers extract `agent_id` from the URL path.

### Architecture for this file

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

| Agent ID | System Prompt |
|----------|---------------|
| `translator` | "You are a translation assistant. Translate the input to English." |
| `summarizer` | "You are a summarization assistant. Provide a brief summary." |
| `coder` | "You are a coding assistant. Help with programming questions." |

### Imports

```rust
use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::Path;
use axum::extract::ws::WebSocket;
use axum::extract::Query;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use axum::extract::WebSocketUpgrade;
use serde::Deserialize;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tracing::info;
use vol_llm_agent::agent_def::AgentDef;
use vol_llm_agent::react::{AgentConfig, PluginRegistry, ReActAgent};
use vol_llm_agent_channel::transport::ws::WsConnection;
use vol_llm_agent_channel::{AgentDispatcher, AgentRouter, ConnectionHolder};
use vol_llm_context::ContextBuilderBuilder;
use vol_llm_provider::create_provider;
use vol_llm_tool::ToolRegistry;
use vol_session::{InMemoryEntryStore, Session};
```

### AppState struct

```rust
struct AppState {
    router: AgentRouter,
    dispatchers: Arc<RwLock<HashMap<String, Arc<AgentDispatcher>>>>,
    holders: Arc<RwLock<HashMap<String, Arc<ConnectionHolder>>>>,
}
```

### Agent definition helper

```rust
fn make_agent(llm: Arc<dyn vol_llm_core::LLMClient>, name: &str, prompt: &str) -> ReActAgent {
    let def = AgentDef::new(name, prompt).with_type(name);
    let session = Arc::new(Session::new(Arc::new(InMemoryEntryStore::new())));
    let tools = Arc::new(ToolRegistry::new());
    let context_builder = ContextBuilderBuilder::new(128_000).build();
    let config = AgentConfig {
        def: Some(def),
        llm,
        tools,
        session,
        sandbox: None,
        context_builder,
        plugin_registry: PluginRegistry::new(),
    };
    ReActAgent::new(config)
}
```

- [ ] **Step 1: Write the file**

Create `crates/vol-llm-agent-channel/examples/multi_agent.rs` with this complete content:

```rust
//! Multi-agent service with per-agent WebSocket and HTTP endpoints.
//!
//! Run with:
//! ```bash
//! ANTHROPIC_AUTH_TOKEN=your_key RUST_LOG=info \
//!   cargo run --example multi_agent -p vol-llm-agent-channel
//! ```
//!
//! Endpoints:
//! - `GET /ws/:agent_id` — WebSocket to a specific agent
//! - `POST /api/chat/:agent_id` — HTTP POST to a specific agent
//! - `GET /api/agents` — List registered agents
//! - `GET /health` — Health check

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::Path;
use axum::extract::Query;
use axum::extract::WebSocketUpgrade;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use axum::extract::ws::WebSocket;
use serde::Deserialize;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tracing::info;
use vol_llm_agent::agent_def::AgentDef;
use vol_llm_agent::react::{AgentConfig, PluginRegistry, ReActAgent};
use vol_llm_agent_channel::transport::ws::WsConnection;
use vol_llm_agent_channel::{AgentDispatcher, AgentRouter, ConnectionHolder};
use vol_llm_context::ContextBuilderBuilder;
use vol_llm_provider::create_provider;
use vol_llm_tool::ToolRegistry;
use vol_session::{InMemoryEntryStore, Session};

/// Shared application state.
struct AppState {
    router: AgentRouter,
    holders: Arc<RwLock<HashMap<String, Arc<ConnectionHolder>>>>,
    llm: Arc<dyn vol_llm_core::LLMClient>,
}

/// Build a ReActAgent with the given system prompt.
fn make_agent(llm: Arc<dyn vol_llm_core::LLMClient>, name: &str, prompt: &str) -> ReActAgent {
    let def = AgentDef::new(name, prompt).with_type(name);
    let session = Arc::new(Session::new(Arc::new(InMemoryEntryStore::new())));
    let tools = Arc::new(ToolRegistry::new());
    let context_builder = ContextBuilderBuilder::new(128_000).build();
    let config = AgentConfig {
        def: Some(def),
        llm,
        tools,
        session,
        sandbox: None,
        context_builder,
        plugin_registry: PluginRegistry::new(),
    };
    ReActAgent::new(config)
}

#[tokio::main]
async fn main() {
    // Init tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Create shared LLM provider
    let llm = create_provider(&vol_llm_provider::LLMConfig::with_env_key(
        vol_llm_core::LLMProvider::Anthropic,
        "claude-sonnet-4-6",
        "ANTHROPIC_AUTH_TOKEN",
        "https://coding.dashscope.aliyuncs.com/apps/anthropic",
    ))
    .expect("failed to create LLM provider — set ANTHROPIC_AUTH_TOKEN");

    let llm: Arc<dyn vol_llm_core::LLMClient> = Arc::from(llm);

    // Define agents
    let agents = [
        ("translator", "You are a translation assistant. Translate the input to English."),
        ("summarizer", "You are a summarization assistant. Provide a brief summary."),
        ("coder", "You are a coding assistant. Help with programming questions."),
    ];

    let router = AgentRouter::new();
    let mut holders = HashMap::new();

    for (id, prompt) in &agents {
        let agent = make_agent(llm.clone(), id, prompt);
        let dispatcher = Arc::new(AgentDispatcher::new(agent));
        let holder = Arc::new(ConnectionHolder::new(id.to_string(), "client".to_string()));

        router.register(id.to_string(), dispatcher).await;
        holders.insert(id.to_string(), holder);

        info!(agent_id = id, "Agent registered");
    }

    let state = AppState {
        router,
        holders: Arc::new(RwLock::new(holders)),
        llm,
    };

    // Build router
    let app = Router::new()
        .route("/health", get(|| async { Json(serde_json::json!({"status": "ok"})) }))
        .route("/api/agents", get(list_agents_handler))
        .route("/ws/:agent_id", get(ws_handler))
        .route("/api/chat/:agent_id", post(chat_handler))
        .with_state(state);

    info!("Starting multi-agent server on 0.0.0.0:3000");
    info!("  GET   /api/agents");
    info!("  WS    /ws/:agent_id  (e.g. /ws/translator)");
    info!("  POST  /api/chat/:agent_id");

    let listener = TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("failed to bind to 0.0.0.0:3000");

    axum::serve(listener, app)
        .await
        .expect("server failed");
}

/// GET /api/agents — list registered agents.
async fn list_agents_handler(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl IntoResponse {
    let agents = state.router.list_agents().await;
    Json(serde_json::json!({ "agents": agents }))
}

/// GET /ws/:agent_id — WebSocket upgrade to a specific agent.
async fn ws_handler(
    Path(agent_id): Path<String>,
    ws: WebSocketUpgrade,
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl IntoResponse {
    let holders = state.holders.read().await;
    let dispatchers = state.router.clone();

    // We need to get the dispatcher from the router's internal map.
    // Since AgentRouter doesn't expose a `get` method, we look up
    // via has_agent + we store dispatchers in holders map too.
    // Actually, AgentRouter::send() uses internal dispatchers map.
    // For WS we need the Arc<AgentDispatcher> directly.
    //
    // Solution: store dispatchers alongside holders in AppState.
    // We'll fix this in the state struct below.
    ws.on_upgrade(|_socket| async {
        warn!("ws_handler: dispatchers map not yet wired");
    }).into_response()
}

/// POST /api/chat/:agent_id — HTTP chat with a specific agent.
#[derive(Deserialize)]
struct ChatInput {
    input: String,
}

async fn chat_handler(
    Path(agent_id): Path<String>,
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(body): Json<ChatInput>,
) -> impl IntoResponse {
    let request = vol_llm_agent_channel::AgentRequest::new(&agent_id, &body.input);

    match state.router.send(&agent_id, request).await {
        Ok(rx) => match rx.await {
            Ok(run_result) => match run_result.response {
                Ok(resp) => (
                    StatusCode::OK,
                    Json(serde_json::json!({
                        "req_id": run_result.req_id,
                        "success": true,
                        "response": serde_json::to_value(resp).unwrap_or(serde_json::Value::Null),
                    })),
                ).into_response(),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "success": false, "error": e.to_string() })),
                ).into_response(),
            },
            Err(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "success": false, "error": "dispatcher dropped" })),
            ).into_response(),
        },
        Err(e) => {
            let status = match e {
                vol_llm_agent_channel::ChannelError::AgentNotFound(_) => StatusCode::NOT_FOUND,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            (status, Json(serde_json::json!({ "error": e.to_string() }))).into_response()
        }
    }
}
```

**Wait** — the above has a problem. `AgentRouter` doesn't expose a `get()` method to retrieve the `Arc<AgentDispatcher>` by agent_id. The WS handler needs direct access to the dispatcher. Let me fix the state struct to also store dispatchers.

The corrected `AppState` should be:

```rust
struct AppState {
    router: AgentRouter,
    dispatchers: Arc<RwLock<HashMap<String, Arc<AgentDispatcher>>>>,
    holders: Arc<RwLock<HashMap<String, Arc<ConnectionHolder>>>>,
}
```

And the `ws_handler` becomes:

```rust
async fn ws_handler(
    Path(agent_id): Path<String>,
    ws: WebSocketUpgrade,
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl IntoResponse {
    let dispatchers = state.dispatchers.read().await;
    let holders = state.holders.read().await;

    match (dispatchers.get(&agent_id), holders.get(&agent_id)) {
        (Some(dispatcher), Some(holder)) => {
            let dispatcher = dispatcher.clone();
            let holder = holder.clone();
            let aid = agent_id.clone();
            ws.on_upgrade(move |socket| {
                let conn = WsConnection::new(socket, dispatcher, holder, aid);
                conn.run()
            })
            .into_response()
        }
        _ => (StatusCode::NOT_FOUND, "agent not found").into_response(),
    }
}
```

And in `main()`, after creating each dispatcher, also insert it into the dispatchers map:

```rust
let mut dispatchers = HashMap::new();
// inside the loop:
dispatchers.insert(id.to_string(), dispatcher.clone());
// in AppState:
let state = AppState {
    router,
    dispatchers: Arc::new(RwLock::new(dispatchers)),
    holders: Arc::new(RwLock::new(holders)),
};
```

- [ ] **Step 2: Write the complete corrected file**

Create `crates/vol-llm-agent-channel/examples/multi_agent.rs` with this full content (the ws_handler and AppState are corrected):

```rust
//! Multi-agent service with per-agent WebSocket and HTTP endpoints.
//!
//! Run with:
//! ```bash
//! ANTHROPIC_AUTH_TOKEN=your_key RUST_LOG=info \
//!   cargo run --example multi_agent -p vol-llm-agent-channel
//! ```
//!
//! Endpoints:
//! - `GET /ws/:agent_id` — WebSocket to a specific agent
//! - `POST /api/chat/:agent_id` — HTTP POST to a specific agent
//! - `GET /api/agents` — List registered agents
//! - `GET /health` — Health check

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::Path;
use axum::extract::WebSocketUpgrade;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use axum::extract::ws::WebSocket;
use serde::Deserialize;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tracing::info;
use vol_llm_agent::agent_def::AgentDef;
use vol_llm_agent::react::{AgentConfig, PluginRegistry, ReActAgent};
use vol_llm_agent_channel::transport::ws::WsConnection;
use vol_llm_agent_channel::{AgentDispatcher, AgentRouter, ConnectionHolder};
use vol_llm_context::ContextBuilderBuilder;
use vol_llm_provider::create_provider;
use vol_llm_tool::ToolRegistry;
use vol_session::{InMemoryEntryStore, Session};

/// Shared application state.
struct AppState {
    router: AgentRouter,
    dispatchers: Arc<RwLock<HashMap<String, Arc<AgentDispatcher>>>>,
    holders: Arc<RwLock<HashMap<String, Arc<ConnectionHolder>>>>,
}

/// Build a ReActAgent with the given system prompt.
fn make_agent(llm: Arc<dyn vol_llm_core::LLMClient>, name: &str, prompt: &str) -> ReActAgent {
    let def = AgentDef::new(name, prompt).with_type(name);
    let session = Arc::new(Session::new(Arc::new(InMemoryEntryStore::new())));
    let tools = Arc::new(ToolRegistry::new());
    let context_builder = ContextBuilderBuilder::new(128_000).build();
    let config = AgentConfig {
        def: Some(def),
        llm,
        tools,
        session,
        sandbox: None,
        context_builder,
        plugin_registry: PluginRegistry::new(),
    };
    ReActAgent::new(config)
}

#[tokio::main]
async fn main() {
    // Init tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Create shared LLM provider
    let llm = create_provider(&vol_llm_provider::LLMConfig::with_env_key(
        vol_llm_core::LLMProvider::Anthropic,
        "claude-sonnet-4-6",
        "ANTHROPIC_AUTH_TOKEN",
        "https://coding.dashscope.aliyuncs.com/apps/anthropic",
    ))
    .expect("failed to create LLM provider — set ANTHROPIC_AUTH_TOKEN");

    let llm: Arc<dyn vol_llm_core::LLMClient> = Arc::from(llm);

    // Define agents
    let agents = [
        ("translator", "You are a translation assistant. Translate the input to English."),
        ("summarizer", "You are a summarization assistant. Provide a brief summary."),
        ("coder", "You are a coding assistant. Help with programming questions."),
    ];

    let router = AgentRouter::new();
    let mut dispatchers = HashMap::new();
    let mut holders = HashMap::new();

    for (id, prompt) in &agents {
        let agent = make_agent(llm.clone(), id, prompt);
        let dispatcher = Arc::new(AgentDispatcher::new(agent));
        let holder = Arc::new(ConnectionHolder::new(id.to_string(), "client".to_string()));

        router.register(id.to_string(), dispatcher.clone()).await;
        dispatchers.insert(id.to_string(), dispatcher);
        holders.insert(id.to_string(), holder);

        info!(agent_id = id, "Agent registered");
    }

    let state = AppState {
        router,
        dispatchers: Arc::new(RwLock::new(dispatchers)),
        holders: Arc::new(RwLock::new(holders)),
    };

    // Build router
    let app = Router::new()
        .route("/health", get(|| async { Json(serde_json::json!({"status": "ok"})) }))
        .route("/api/agents", get(list_agents_handler))
        .route("/ws/:agent_id", get(ws_handler))
        .route("/api/chat/:agent_id", post(chat_handler))
        .with_state(state);

    info!("Starting multi-agent server on 0.0.0.0:3000");
    info!("  GET   /api/agents");
    info!("  WS    /ws/:agent_id  (e.g. /ws/translator)");
    info!("  POST  /api/chat/:agent_id");

    let listener = TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("failed to bind to 0.0.0.0:3000");

    axum::serve(listener, app)
        .await
        .expect("server failed");
}

/// GET /api/agents — list registered agents.
async fn list_agents_handler(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl IntoResponse {
    let agents = state.router.list_agents().await;
    Json(serde_json::json!({ "agents": agents }))
}

/// GET /ws/:agent_id — WebSocket upgrade to a specific agent.
async fn ws_handler(
    Path(agent_id): Path<String>,
    ws: WebSocketUpgrade,
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl IntoResponse {
    let dispatchers = state.dispatchers.read().await;
    let holders = state.holders.read().await;

    match (dispatchers.get(&agent_id), holders.get(&agent_id)) {
        (Some(dispatcher), Some(holder)) => {
            let dispatcher = dispatcher.clone();
            let holder = holder.clone();
            let aid = agent_id.clone();
            ws.on_upgrade(move |socket| {
                let conn = WsConnection::new(socket, dispatcher, holder, aid);
                conn.run()
            })
            .into_response()
        }
        _ => (StatusCode::NOT_FOUND, "agent not found").into_response(),
    }
}

/// POST /api/chat/:agent_id — HTTP chat with a specific agent.
#[derive(Deserialize)]
struct ChatInput {
    input: String,
}

async fn chat_handler(
    Path(agent_id): Path<String>,
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(body): Json<ChatInput>,
) -> impl IntoResponse {
    let request = vol_llm_agent_channel::AgentRequest::new(&agent_id, &body.input);

    match state.router.send(&agent_id, request).await {
        Ok(rx) => match rx.await {
            Ok(run_result) => match run_result.response {
                Ok(resp) => (
                    StatusCode::OK,
                    Json(serde_json::json!({
                        "req_id": run_result.req_id,
                        "success": true,
                        "response": serde_json::to_value(resp).unwrap_or(serde_json::Value::Null),
                    })),
                ).into_response(),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "success": false, "error": e.to_string() })),
                ).into_response(),
            },
            Err(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "success": false, "error": "dispatcher dropped" })),
            ).into_response(),
        },
        Err(e) => {
            let status = match e {
                vol_llm_agent_channel::ChannelError::AgentNotFound(_) => StatusCode::NOT_FOUND,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            (status, Json(serde_json::json!({ "error": e.to_string() }))).into_response()
        }
    }
}
```

- [ ] **Step 3: Test compilation**

Run: `cargo build --example multi_agent -p vol-llm-agent-channel`
Expected: compiles without errors

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent-channel/examples/multi_agent.rs
git commit -m "feat: add multi_agent example with per-agent WS and HTTP routing"
```

---

### Task 4: Verify both examples compile and existing tests pass

- [ ] **Step 1: Build all examples**

Run: `cargo build --examples -p vol-llm-agent-channel`
Expected: all examples compile without errors

- [ ] **Step 2: Run existing tests**

Run: `cargo test -p vol-llm-agent-channel`
Expected: all 11 tests pass

- [ ] **Step 3: Commit if needed**

If any fixes were applied in prior steps, commit them. Otherwise skip.
