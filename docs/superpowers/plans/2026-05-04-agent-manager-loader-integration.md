# Agent Manager Loader Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add file-based agent discovery to vol-agent-manager and route WS clients to dynamically instantiated agent instances.

**Architecture:** Three layers — (1) AgentLoader discovers agent definitions from `.agents/agents/*.md`, (2) InstanceRegistry manages running `(agent_type, session_id)` instances, (3) WS Router parses `/ws/agents/:type/session/:id` paths and dispatches to instances.

**Tech Stack:** Rust, axum (WebSocket), tokio (broadcast), vol-llm-agent (AgentLoader, ReActAgent), vol-session (FileSessionEntryStore).

---

### Task 1: FileSessionEntryStore agent_type subdirectory support

**Files:**
- Modify: `crates/vol-session/src/file_store.rs`
- Test: `crates/vol-session/src/file_store.rs` (inline tests)

- [ ] **Step 1: Add agent_type field and new constructor**

Add `agent_type: Option<String>` to `FileSessionEntryStore` and a new constructor:

```rust
// In FileSessionEntryStore struct, add field:
pub struct FileSessionEntryStore {
    entry_dir: PathBuf,
    agent_type: Option<String>,
}

// Modify existing constructor:
impl FileSessionEntryStore {
    /// Create a new file entry store (no agent_type scoping).
    pub fn new<P: AsRef<Path>>(entry_dir: P) -> Self {
        Self {
            entry_dir: entry_dir.as_ref().to_path_buf(),
            agent_type: None,
        }
    }

    /// Create a new file entry store with optional agent_type scoping.
    /// When agent_type is Some, files are stored under {entry_dir}/{agent_type}/{session_id}.jsonl.
    /// When None, uses original {entry_dir}/{session_id}.jsonl (backward compatible).
    pub fn with_agent_type<P: AsRef<Path>>(entry_dir: P, agent_type: Option<String>) -> Self {
        Self {
            entry_dir: entry_dir.as_ref().to_path_buf(),
            agent_type,
        }
    }

    /// Resolve file path for a session, respecting agent_type.
    fn file_path(&self, session_id: &str) -> PathBuf {
        match &self.agent_type {
            Some(agent_type) => self.entry_dir.join(agent_type).join(format!("{}.jsonl", session_id)),
            None => self.entry_dir.join(format!("{}.jsonl", session_id)),
        }
    }

    // ... rest of existing methods (ensure_dir, append_line, to_json, from_json, etc.)
    // ensure_dir already creates all directories including the agent_type subdirectory
```

- [ ] **Step 2: Add test for agent_type subdirectory**

```rust
#[tokio::test]
async fn test_file_entry_store_with_agent_type() {
    let temp_dir = tempdir().unwrap();
    let store = FileSessionEntryStore::with_agent_type(
        temp_dir.path(),
        Some("qa".to_string()),
    );

    let entry = SessionEntry::from_message(
        SessionMessage::new("test-session".to_string(), Message::user("Hello")),
    );
    store.save(entry).await.unwrap();

    // File should be at qa/test-session.jsonl
    let expected_path = temp_dir.path().join("qa").join("test-session.jsonl");
    assert!(expected_path.exists(), "Expected file at {}", expected_path.display());

    // Verify content
    let entries = store.get_entries("test-session").await.unwrap();
    assert_eq!(entries.len(), 1);
}

#[tokio::test]
async fn test_file_entry_store_agent_type_isolation() {
    let temp_dir = tempdir().unwrap();

    let qa_store = FileSessionEntryStore::with_agent_type(temp_dir.path(), Some("qa".to_string()));
    let code_store = FileSessionEntryStore::with_agent_type(temp_dir.path(), Some("code".to_string()));

    // Save to qa
    qa_store.save(SessionEntry::from_message(
        SessionMessage::new("shared-id".to_string(), Message::user("qa data")),
    )).await.unwrap();

    // code store should not see qa's data
    let code_entries = code_store.get_entries("shared-id").await.unwrap();
    assert_eq!(code_entries.len(), 0);

    // qa store should see its own data
    let qa_entries = qa_store.get_entries("shared-id").await.unwrap();
    assert_eq!(qa_entries.len(), 1);
}

#[tokio::test]
async fn test_file_entry_store_no_agent_type_backward_compat() {
    let temp_dir = tempdir().unwrap();
    let store = FileSessionEntryStore::new(temp_dir.path());

    let entry = SessionEntry::from_message(
        SessionMessage::new("old-session".to_string(), Message::user("legacy")),
    );
    store.save(entry).await.unwrap();

    // File should be at original location (no subdirectory)
    let expected_path = temp_dir.path().join("old-session.jsonl");
    assert!(expected_path.exists());
}
```

- [ ] **Step 3: Run tests to verify**

```bash
cargo test -p vol-session file_store -- --test-threads=1
```

Expected: All file_store tests pass (existing + 3 new).

- [ ] **Step 4: Commit**

```bash
git add crates/vol-session/src/file_store.rs
git commit -m "feat(vol-session): add agent_type subdirectory support to FileSessionEntryStore

Backward compatible: when agent_type is None, files stored at
{entry_dir}/{session_id}.jsonl. When Some(type), stored at
{entry_dir}/{type}/{session_id}.jsonl."
```

---

### Task 2: Add vol-llm-agent dependency to vol-agent-manager

**Files:**
- Modify: `crates/vol-agent-manager/Cargo.toml`

- [ ] **Step 1: Add vol-llm-agent dependency**

```toml
# In [dependencies], add:
vol-llm-agent = { path = "../vol-llm-agent" }
vol-session = { path = "../vol-session" }
vol-llm-provider = { path = "../vol-llm-provider" }
vol-llm-core = { path = "../vol-llm-core" }
vol-llm-tools-builtin = { path = "../vol-llm-tools-builtin" }
```

- [ ] **Step 2: Verify workspace resolves dependencies**

```bash
cargo check -p vol-agent-manager 2>&1 | head -30
```

Expected: Compiles successfully (no new code yet, just dependency resolution).

- [ ] **Step 3: Commit**

```bash
git add crates/vol-agent-manager/Cargo.toml
git commit -m "chore: add vol-llm-agent and related dependencies to vol-agent-manager"
```

---

### Task 3: AgentInstance and InstanceRegistry

**Files:**
- Create: `crates/vol-agent-manager/src/instance.rs`
- Modify: `crates/vol-agent-manager/src/lib.rs` (export instance module, add to AppRouterState)

- [ ] **Step 1: Create instance.rs**

```rust
use std::collections::HashSet;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use tokio::sync::{broadcast, RwLock};

use vol_session::{FileSessionEntryStore, Session};

/// Status of a running agent instance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InstanceStatus {
    Running,
    Completed,
    Failed,
}

/// Summary of a running instance for API responses.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AgentInstanceSummary {
    pub agent_type: String,
    pub session_id: String,
    pub parent_session_id: Option<String>,
    pub status: InstanceStatus,
    pub connection_count: usize,
    pub created_at: i64,
}

/// A running agent instance.
pub struct AgentInstance {
    pub agent_type: String,
    pub session_id: String,
    pub parent_session_id: Option<String>,
    pub session: Arc<Session>,
    pub status: InstanceStatus,
    pub created_at: DateTime<Utc>,
    pub ws_connections: HashSet<String>,
    pub broadcast_tx: broadcast::Sender<serde_json::Value>,
}

impl AgentInstance {
    fn new(agent_type: String, session_id: String, parent_session_id: Option<String>, session: Arc<Session>) -> Self {
        // broadcast channel with capacity 64 — oldest messages dropped if full
        let (tx, _) = broadcast::channel(64);
        Self {
            agent_type,
            session_id,
            parent_session_id,
            session,
            status: InstanceStatus::Running,
            created_at: Utc::now(),
            ws_connections: HashSet::new(),
            broadcast_tx: tx,
        }
    }
}

/// Thread-safe registry of running agent instances.
pub struct AgentInstanceRegistry {
    instances: Arc<RwLock<Vec<Arc<AgentInstance>>>>,
}

impl AgentInstanceRegistry {
    pub fn new() -> Self {
        Self {
            instances: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Get existing instance or create new one.
    pub async fn get_or_create(
        &self,
        agent_type: &str,
        session_id: &str,
        parent_session_id: Option<String>,
        store: Arc<FileSessionEntryStore>,
    ) -> Arc<AgentInstance> {
        let key = (agent_type.to_string(), session_id.to_string());

        // Check existing
        {
            let guard = self.instances.read().await;
            if let Some(instance) = guard.iter().find(|i| i.agent_type == key.0 && i.session_id == key.1) {
                return instance.clone();
            }
        }

        // Create new
        let entry_store = FileSessionEntryStore::with_agent_type(
            &store.entry_dir_for_type(agent_type),
            Some(agent_type.to_string()),
        );
        // Note: we need a helper on FileSessionEntryStore to get the base dir
        // Actually, let's simplify: pass entry_dir as String from caller

        let session = Arc::new(Session::new(Arc::new(entry_store)));
        let instance = Arc::new(AgentInstance::new(
            agent_type.to_string(),
            session_id.to_string(),
            parent_session_id,
            session,
        ));

        let mut guard = self.instances.write().await;
        guard.push(instance.clone());
        instance
    }

    /// Add a WS connection to an instance.
    pub async fn add_connection(&self, agent_type: &str, session_id: &str, conn_id: String) -> Option<Arc<AgentInstance>> {
        let guard = self.instances.read().await;
        let instance = guard.iter().find(|i| i.agent_type == agent_type && i.session_id == session_id)?;
        instance.ws_connections.write().await.insert(conn_id);
        Some(instance.clone())
    }

    /// Remove a WS connection from an instance.
    pub async fn remove_connection(&self, agent_type: &str, session_id: &str, conn_id: &str) {
        let guard = self.instances.read().await;
        if let Some(instance) = guard.iter().find(|i| i.agent_type == agent_type && i.session_id == session_id) {
            instance.ws_connections.remove(conn_id);
        }
    }

    /// Get broadcast sender for an instance.
    pub async fn get_broadcast(&self, agent_type: &str, session_id: &str) -> Option<broadcast::Sender<serde_json::Value>> {
        let guard = self.instances.read().await;
        let instance = guard.iter().find(|i| i.agent_type == agent_type && i.session_id == session_id)?;
        Some(instance.broadcast_tx.clone())
    }

    /// List all running instances.
    pub async fn list_instances(&self) -> Vec<AgentInstanceSummary> {
        let guard = self.instances.read().await;
        guard.iter().map(|i| AgentInstanceSummary {
            agent_type: i.agent_type.clone(),
            session_id: i.session_id.clone(),
            parent_session_id: i.parent_session_id.clone(),
            status: i.status,
            connection_count: i.ws_connections.len(),
            created_at: i.session.created_at,
        }).collect()
    }

    /// Destroy an instance (does NOT delete session files).
    pub async fn destroy(&self, agent_type: &str, session_id: &str) {
        let mut guard = self.instances.write().await;
        guard.retain(|i| !(i.agent_type == agent_type && i.session_id == session_id));
    }
}

impl Default for AgentInstanceRegistry {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 2: Add helper to FileSessionEntryStore to expose entry_dir**

Add a public getter in `file_store.rs`:

```rust
impl FileSessionEntryStore {
    /// Get the base entry directory path.
    pub fn entry_dir(&self) -> &Path {
        &self.entry_dir
    }
}
```

- [ ] **Step 3: Update lib.rs**

```rust
// In crates/vol-agent-manager/src/lib.rs, add:
pub mod instance;

// Add to AppRouterState:
use instance::AgentInstanceRegistry;

#[derive(Clone)]
pub struct AppRouterState {
    pub state_manager: Arc<AgentStateManager>,
    pub metrics: Arc<MetricsCollector>,
    pub event_bus: Arc<EventBus>,
    pub task_dispatcher: Arc<TaskDispatcher>,
    pub config: ManagerConfig,
    pub instance_registry: Arc<AgentInstanceRegistry>,
    pub agent_loader: Arc<vol_llm_agent::AgentLoader>,
}
```

- [ ] **Step 4: Write tests**

```rust
// In instance.rs, add tests module:
#[cfg(test)]
mod tests {
    use super::*;
    use vol_session::InMemoryEntryStore;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_registry_get_or_create_new() {
        let registry = AgentInstanceRegistry::new();
        let store = Arc::new(FileSessionEntryStore::new("/tmp/test_agents"));
        let instance = registry.get_or_create("qa", "sess-1", None, store).await;
        assert_eq!(instance.agent_type, "qa");
        assert_eq!(instance.session_id, "sess-1");
    }

    #[tokio::test]
    async fn test_registry_get_or_create_returns_existing() {
        let registry = AgentInstanceRegistry::new();
        let store = Arc::new(FileSessionEntryStore::new("/tmp/test_agents"));
        let first = registry.get_or_create("qa", "sess-1", None, store.clone()).await;
        let second = registry.get_or_create("qa", "sess-1", None, store).await;
        assert!(Arc::ptr_eq(&first, &second));
    }

    #[tokio::test]
    async fn test_registry_list_instances() {
        let registry = AgentInstanceRegistry::new();
        let store = Arc::new(FileSessionEntryStore::new("/tmp/test_agents"));
        registry.get_or_create("qa", "s1", None, store.clone()).await;
        registry.get_or_create("code", "s2", None, store).await;
        let list = registry.list_instances().await;
        assert_eq!(list.len(), 2);
    }

    #[tokio::test]
    async fn test_registry_destroy() {
        let registry = AgentInstanceRegistry::new();
        let store = Arc::new(FileSessionEntryStore::new("/tmp/test_agents"));
        registry.get_or_create("qa", "sess-1", None, store).await;
        registry.destroy("qa", "sess-1").await;
        assert!(registry.list_instances().await.is_empty());
    }
}
```

- [ ] **Step 5: Run tests to verify**

```bash
cargo test -p vol-agent-manager instance -- --test-threads=1
```

Expected: Tests compile but may fail at this point since AppRouterState isn't updated in main.rs yet. The unit tests in instance.rs should pass.

- [ ] **Step 6: Commit**

```bash
git add crates/vol-agent-manager/src/instance.rs crates/vol-agent-manager/src/lib.rs crates/vol-session/src/file_store.rs
git commit -m "feat: add AgentInstance and AgentInstanceRegistry to vol-agent-manager"
```

---

### Task 4: AgentLoader integration into main.rs

**Files:**
- Modify: `crates/vol-agent-manager/src/main.rs`
- Modify: `crates/vol-agent-manager/src/lib.rs`

- [ ] **Step 1: Initialize AgentLoader in main.rs**

```rust
// In main.rs, after creating state_manager, metrics, etc:
let agent_loader = Arc::new(vol_llm_agent::AgentLoader::new(None));

// Discover agents from .agents/agents/ on startup
if let Err(e) = agent_loader.discover_all().await {
    tracing::warn!(error = %e, "Failed to discover agent definitions");
}

let instance_registry = Arc::new(vol_agent_manager::instance::AgentInstanceRegistry::new());

let app_state = AppRouterState {
    state_manager: state_manager.clone(),
    metrics: metrics.clone(),
    event_bus: event_bus.clone(),
    task_dispatcher: task_dispatcher.clone(),
    config: config.clone(),
    instance_registry: instance_registry.clone(),
    agent_loader: agent_loader.clone(),
};
```

- [ ] **Step 2: Run check to verify compilation**

```bash
cargo check -p vol-agent-manager 2>&1 | head -40
```

Expected: Compiles successfully (new fields in AppRouterState, loader initialized).

- [ ] **Step 3: Commit**

```bash
git add crates/vol-agent-manager/src/main.rs crates/vol-agent-manager/src/lib.rs
git commit -m "feat: integrate AgentLoader into vol-agent-manager startup"
```

---

### Task 5: WS Router — path-based routing

**Files:**
- Create: `crates/vol-agent-manager/src/ws/router.rs`
- Modify: `crates/vol-agent-manager/src/ws/mod.rs`
- Modify: `crates/vol-agent-manager/src/ws/server.rs`

- [ ] **Step 1: Create router.rs**

```rust
use axum::{
    extract::{Path, State, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Router,
};
use tracing::warn;

use crate::instance::AgentInstanceRegistry;
use crate::AppRouterState;

/// Create routes for agent-specific WebSocket connections.
pub fn create_agent_ws_router() -> Router<AppRouterState> {
    Router::new()
        .route("/ws/agents/:agent_type/session/:session_id", get(upgrade_agent_ws))
}

#[derive(Debug, serde::Deserialize)]
pub struct AgentWsQuery {
    pub parent_session_id: Option<String>,
}

async fn upgrade_agent_ws(
    Path((agent_type, session_id)): Path<(String, String)>,
    query: Option<axum::extract::Query<AgentWsQuery>>,
    ws: WebSocketUpgrade,
    State(state): State<AppRouterState>,
) -> impl IntoResponse {
    // Validate agent type exists in AgentLoader
    let agent_def = state.agent_loader.get(&agent_type).await;
    if agent_def.is_none() {
        warn!(agent_type = %agent_type, "Agent type not found in definitions");
        // Return 404 response (axum will handle it)
        return axum::http::StatusCode::NOT_FOUND.into_response();
    }

    let parent_session_id = query.and_then(|q| q.0.parent_session_id);

    ws.on_upgrade(move |socket| {
        handle_agent_ws(socket, agent_type, session_id, parent_session_id, state)
    })
}

pub async fn handle_agent_ws(
    mut ws: axum::extract::ws::WebSocket,
    agent_type: String,
    session_id: String,
    parent_session_id: Option<String>,
    state: AppRouterState,
) {
    use axum::extract::ws::Message;
    use tokio::sync::broadcast;

    let conn_id = uuid::Uuid::new_v4().to_string();

    // Get or create instance
    // For now, we need to pass the session store dir. Use config or default path.
    let base_store = Arc::new(vol_session::FileSessionEntryStore::new("/tmp/vol-sessions"));
    let instance = state
        .instance_registry
        .get_or_create(&agent_type, &session_id, parent_session_id, base_store)
        .await;

    let broadcast_tx = instance.broadcast_tx.clone();
    let mut broadcast_rx = broadcast_tx.subscribe();

    // Add connection
    state.instance_registry.add_connection(&agent_type, &session_id, conn_id.clone()).await;

    // Send welcome message
    let welcome = serde_json::json!({
        "message_type": "connected",
        "agent_type": agent_type,
        "session_id": session_id,
    });
    let _ = ws.send(Message::Text(welcome.to_string())).await;

    // Spawn broadcast receiver task
    let ws_sender = tokio::spawn(async move {
        loop {
            match broadcast_rx.recv().await {
                Ok(data) => {
                    let msg = Message::Text(data.to_string());
                    if ws.send(msg).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Closed) => break,
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
            }
        }
    });

    // Main receive loop
    loop {
        match ws.recv().await {
            Some(Ok(Message::Text(text))) => {
                // Parse user input and forward to agent
                if let Ok(input) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(content) = input.get("content").and_then(|v| v.as_str()) {
                        // TODO: forward to agent run in Task 6
                        tracing::info!(agent_type = %agent_type, session_id = %session_id, "Received user input: {}", content);
                    }
                }
            }
            Some(Ok(Message::Close(_))) | None => {
                break;
            }
            Some(Ok(Message::Binary(_) | Message::Ping(_) | Message::Pong(_))) => {
                // Ignore
            }
            Some(Err(e)) => {
                warn!(error = %e, "WebSocket error");
                break;
            }
        }
    }

    // Cleanup
    state.instance_registry.remove_connection(&agent_type, &session_id, &conn_id).await;
    let _ = ws_sender.abort();
}
```

- [ ] **Step 2: Update ws/mod.rs**

```rust
pub mod handler;
pub mod protocol;
pub mod router;
pub mod server;
```

- [ ] **Step 3: Update ws/server.rs — add agent router to main app**

```rust
// In create_ws_router(), add:
pub fn create_ws_router() -> Router<AppRouterState> {
    Router::new()
        .route("/ws", get(upgrade_ws))
        .merge(crate::ws::router::create_agent_ws_router())
}
```

- [ ] **Step 4: Run check**

```bash
cargo check -p vol-agent-manager 2>&1 | head -40
```

Expected: Compiles successfully.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-agent-manager/src/ws/router.rs crates/vol-agent-manager/src/ws/mod.rs crates/vol-agent-manager/src/ws/server.rs
git commit -m "feat: add WS router for agent-specific WebSocket connections"
```

---

### Task 6: REST API — agent types and instances endpoints

**Files:**
- Modify: `crates/vol-agent-manager/src/main.rs`
- Modify: `crates/vol-agent-manager/src/instance.rs` (serialization)

- [ ] **Step 1: Add serialization to InstanceStatus**

In `instance.rs`, add `#[derive(serde::Serialize)]`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub enum InstanceStatus {
    Running,
    Completed,
    Failed,
}
```

- [ ] **Step 2: Add REST handlers in main.rs**

```rust
// Add new routes to the app:
.route("/api/v1/agent-types", get(list_agent_types))
.route("/api/v1/agent-instances", get(list_agent_instances))
.route("/api/v1/agent-instances/:type/:session_id", delete(destroy_agent_instance))

// New handler functions:

async fn list_agent_types(State(state): State<AppRouterState>) -> impl IntoResponse {
    let metadata = state.agent_loader.list_metadata().await;
    axum::Json(serde_json::json!({
        "agent_types": metadata.iter().map(|m| serde_json::json!({
            "name": m.name,
            "type": m.r#type,
            "description": m.description,
            "scope": format!("{:?}", m.scope),
        })).collect::<Vec<_>>()
    }))
}

async fn list_agent_instances(State(state): State<AppRouterState>) -> impl IntoResponse {
    let instances = state.instance_registry.list_instances().await;
    axum::Json(serde_json::json!({ "instances": instances }))
}

async fn destroy_agent_instance(
    State(state): State<AppRouterState>,
    Path((agent_type, session_id)): Path<(String, String)>,
) -> impl IntoResponse {
    state.instance_registry.destroy(&agent_type, &session_id).await;
    (axum::http::StatusCode::NO_CONTENT, ())
}
```

- [ ] **Step 3: Run check**

```bash
cargo check -p vol-agent-manager 2>&1 | head -40
```

Expected: Compiles successfully.

- [ ] **Step 4: Commit**

```bash
git add crates/vol-agent-manager/src/main.rs crates/vol-agent-manager/src/instance.rs
git commit -m "feat: add REST API endpoints for agent types and instances"
```

---

### Task 7: Agent instantiation and run loop

**Files:**
- Modify: `crates/vol-agent-manager/src/ws/router.rs` (implement actual agent run)
- Modify: `crates/vol-agent-manager/src/lib.rs` (add LLM config to state)

- [ ] **Step 1: Add LLM config to AppRouterState**

In `lib.rs`:

```rust
pub struct AppRouterState {
    // ... existing fields
    pub llm_config: vol_llm_provider::LLMConfig,
}
```

In `main.rs`, create LLM config:

```rust
let llm_config = vol_llm_provider::LLMConfig::with_env_key(
    vol_llm_core::LLMProvider::Anthropic,
    "qwen3.5-plus",
    "ANTHROPIC_AUTH_TOKEN",
    "https://coding.dashscope.aliyuncs.com/apps/anthropic",
);

// Add to app_state
let app_state = AppRouterState {
    // ... existing fields
    llm_config,
};
```

- [ ] **Step 2: Implement agent run in router.rs**

Replace the TODO in the message receive loop with actual agent instantiation and execution:

```rust
use vol_llm_agent::{AgentBuilder, AgentDef, AgentLoader};
use vol_llm_provider::{create_provider, LLMConfig};
use std::sync::Arc;

// Function to create and run an agent from definition
async fn run_agent_instance(
    agent_def: AgentDef,
    session: Arc<vol_session::Session>,
    llm_config: LLMConfig,
    broadcast_tx: broadcast::Sender<serde_json::Value>,
    instance_registry: Arc<AgentInstanceRegistry>,
    agent_type: String,
    session_id: String,
) {
    // Create LLM client
    let llm = match create_provider(&llm_config) {
        Ok(client) => client,
        Err(e) => {
            let err = serde_json::json!({
                "message_type": "agent_error",
                "error": format!("Failed to create LLM client: {}", e),
            });
            let _ = broadcast_tx.send(err);
            instance_registry.destroy(&agent_type, &session_id).await;
            return;
        }
    };

    // Build agent from definition
    let agent = AgentBuilder::new()
        .with_llm(Arc::from(llm))
        .with_system_prompt(agent_def.content)
        .with_session(session)
        .with_max_iterations(agent_def.max_iterations.unwrap_or(10))
        .build();

    let agent = match agent {
        Ok(a) => a,
        Err(e) => {
            let err = serde_json::json!({
                "message_type": "agent_error",
                "error": format!("Failed to build agent: {}", e),
            });
            let _ = broadcast_tx.send(err);
            instance_registry.destroy(&agent_type, &session_id).await;
            return;
        }
    };

    // Run agent and broadcast events
    let (mut rx, _handle) = match agent.run_stream("").await {
        Ok(result) => result,
        Err(e) => {
            let err = serde_json::json!({
                "message_type": "agent_error",
                "error": format!("Agent run error: {}", e),
            });
            let _ = broadcast_tx.send(err);
            instance_registry.destroy(&agent_type, &session_id).await;
            return;
        }
    };

    while let Some(event) = rx.recv().await {
        match event {
            Ok(e) => {
                let data = serde_json::json!({
                    "message_type": "agent_event",
                    "event": format!("{:?}", e),
                });
                let _ = broadcast_tx.send(data);
            }
            Err(e) => {
                let data = serde_json::json!({
                    "message_type": "agent_error",
                    "error": e.to_string(),
                });
                let _ = broadcast_tx.send(data);
                break;
            }
        }
    }

    // Agent completed
    instance_registry.destroy(&agent_type, &session_id).await;
    let done = serde_json::json!({
        "message_type": "agent_complete",
    });
    let _ = broadcast_tx.send(done);
}
```

- [ ] **Step 3: Wire up user input to agent run**

In `handle_agent_ws`, when receiving user input, spawn the agent run:

```rust
// In handle_agent_ws, after getting instance:
let agent_def = state.agent_loader.get(&agent_type).await.unwrap();
let llm_config = state.llm_config.clone();
let instance_registry = state.instance_registry.clone();
let session = instance.session.clone();

// Spawn agent run on first user input
let agent_spawned = Arc::new(tokio::sync::Mutex::new(false));

// In the message loop, when receiving user input:
if !*agent_spawned.lock().await {
    *agent_spawned.lock().await = true;
    let content = content.to_string();
    tokio::spawn({
        let agent_def = agent_def.clone();
        let llm_config = llm_config.clone();
        let broadcast_tx = broadcast_tx.clone();
        let instance_registry = instance_registry.clone();
        let session = session.clone();
        let agent_type = agent_type.clone();
        let session_id = session_id.clone();
        async move {
            run_agent_instance(
                agent_def, session, llm_config, broadcast_tx,
                instance_registry, agent_type, session_id,
            ).await;
        }
    });
}
```

- [ ] **Step 4: Run check**

```bash
cargo check -p vol-agent-manager 2>&1
```

Expected: May have minor compile errors to fix (import issues, etc.)

- [ ] **Step 5: Commit**

```bash
git add crates/vol-agent-manager/src/ws/router.rs crates/vol-agent-manager/src/lib.rs crates/vol-agent-manager/src/main.rs
git commit -m "feat: implement agent instantiation and run loop in WS handler"
```

---

### Task 8: Integration test

**Files:**
- Modify: `crates/vol-agent-manager/tests/integration.rs`

- [ ] **Step 1: Add integration test for agent WS routing**

```rust
// In tests/integration.rs, add:

#[tokio::test]
async fn test_agent_ws_router_rejects_unknown_type() {
    use axum::{body::Body, http::Request};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    // Build test app with no agent definitions
    let app = build_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/ws/agents/unknown-type/session/test-123")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_list_agent_types_empty() {
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    let app = build_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/agent-types")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["agent_types"], serde_json::json!([]));
}

#[tokio::test]
async fn test_list_agent_instances_empty() {
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    let app = build_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/agent-instances")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["instances"], serde_json::json!([]));
}
```

- [ ] **Step 2: Create test app builder helper**

```rust
// In tests/integration.rs, add helper:
async fn build_test_app() -> axum::Router {
    use std::sync::Arc;
    use vol_agent_manager::*;
    use vol_agent_manager::instance::AgentInstanceRegistry;

    let state_manager = Arc::new(state::manager::AgentStateManager::new());
    let metrics = Arc::new(metrics::collector::MetricsCollector::new());
    let event_bus = Arc::new(events::EventBus::new());
    let task_dispatcher = Arc::new(task::dispatcher::TaskDispatcher::new());
    let instance_registry = Arc::new(AgentInstanceRegistry::new());
    let agent_loader = Arc::new(vol_llm_agent::AgentLoader::new_empty());
    let llm_config = vol_llm_provider::LLMConfig::with_env_key(
        vol_llm_core::LLMProvider::Anthropic,
        "qwen3.5-plus",
        "ANTHROPIC_AUTH_TOKEN",
        "https://coding.dashscope.aliyuncs.com/apps/anthropic",
    );

    let config = config::ManagerConfig::default();

    let app_state = AppRouterState {
        state_manager,
        metrics,
        event_bus,
        task_dispatcher,
        config,
        instance_registry,
        agent_loader,
        llm_config,
    };

    ws::server::create_ws_router()
        .route("/health", get(|| async { axum::Json(serde_json::json!({"status": "ok"})) }))
        .route("/metrics", get(|State(s): State<AppRouterState>| async move {
            let encoder = prometheus::TextEncoder::new();
            let metric_families = s.metrics.gather();
            let mut buffer = Vec::new();
            encoder.encode(&metric_families, &mut buffer).unwrap();
            (
                [(axum::http::header::CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")],
                String::from_utf8(buffer).unwrap(),
            )
        }))
        .route("/api/v1/agents", get(list_agents))
        .route("/api/v1/agents/:id", get(get_agent))
        .route("/api/v1/agents/:id/tasks", post(dispatch_task))
        .route("/api/v1/tasks/:id", get(get_task))
        .route("/api/v1/tasks", get(list_tasks))
        .route("/api/v1/events", get(events_handler))
        .route("/api/v1/agent-types", get(list_agent_types))
        .route("/api/v1/agent-instances", get(list_agent_instances))
        .route("/api/v1/agent-instances/:type/:session_id", delete(destroy_agent_instance))
        .with_state(app_state)
}
```

- [ ] **Step 3: Run integration tests**

```bash
cargo test -p vol-agent-manager --test integration 2>&1
```

Expected: New tests pass. Existing tests may need minor updates for new AppRouterState fields.

- [ ] **Step 4: Run all tests**

```bash
cargo test -p vol-agent-manager 2>&1
```

Expected: All tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-agent-manager/tests/integration.rs
git commit -m "test: add integration tests for agent WS routing and REST API"
```
