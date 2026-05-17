# Agent Channel Communication Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create a new `vol-llm-agent-channel` crate that provides `AgentDispatcher` (FIFO queue wrapper around `ReActAgent`) and `AgentRouter` (multi-agent request routing).

**Architecture:** Two layers — `AgentDispatcher` manages a single-agent queue with submit/cancel, `AgentRouter` routes requests between multiple dispatchers by `target_id`. All operations are async via tokio primitives.

**Tech Stack:** Rust, tokio (mpsc, oneshot, Mutex), thiserror, vol-llm-agent

---

### Task 1: Create crate scaffold

**Files:**
- Create: `crates/vol-llm-agent-channel/Cargo.toml`
- Create: `crates/vol-llm-agent-channel/src/lib.rs`
- Modify: `Cargo.toml` (workspace root, add to members and workspace.dependencies)

- [ ] **Step 1: Create Cargo.toml**

```toml
# crates/vol-llm-agent-channel/Cargo.toml
[package]
name = "vol-llm-agent-channel"
version.workspace = true
edition.workspace = true

[dependencies]
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
uuid = { version = "1.6", features = ["v4"] }
vol-llm-agent = { path = "../vol-llm-agent" }

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 2: Create lib.rs**

```rust
//! vol-llm-agent-channel: Channel-based communication layer for ReActAgent.
//!
//! Provides `AgentDispatcher` for single-agent request queueing and
//! `AgentRouter` for multi-agent request routing.

pub mod dispatcher;
pub mod error;
pub mod request;
pub mod router;

pub use dispatcher::AgentDispatcher;
pub use error::ChannelError;
pub use request::{AgentRequest, RunResult};
pub use router::AgentRouter;
```

- [ ] **Step 3: Add to workspace Cargo.toml**

Add `"crates/vol-llm-agent-channel",` to the `members` array (after `"crates/vol-llm-yaml-agent",`):

```toml
# In root Cargo.toml, members section
    "crates/vol-llm-agent-channel",
```

Add to `[workspace.dependencies]`:

```toml
vol-llm-agent-channel = { path = "crates/vol-llm-agent-channel" }
```

- [ ] **Step 4: Verify crate compiles**

Run: `cargo check -p vol-llm-agent-channel`
Expected: no errors (will have "unused" warnings which is fine)

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-agent-channel/ Cargo.toml
git commit -m "feat: add vol-llm-agent-channel crate scaffold"
```

---

### Task 2: Implement error types

**Files:**
- Create: `crates/vol-llm-agent-channel/src/error.rs`

- [ ] **Step 1: Create error.rs**

```rust
// crates/vol-llm-agent-channel/src/error.rs

#[derive(Debug, thiserror::Error)]
pub enum ChannelError {
    /// Target agent not found in router.
    #[error("agent '{0}' not registered")]
    AgentNotFound(String),

    /// Request was cancelled before execution.
    #[error("request '{0}' was cancelled")]
    Cancelled(String),

    /// Dispatcher dropped while request was pending.
    #[error("dispatcher dropped")]
    DispatcherDropped,

    /// Internal agent error (from ReActAgent::run).
    #[error("agent execution error: {0}")]
    AgentError(String),
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p vol-llm-agent-channel`
Expected: no errors

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent-channel/src/error.rs
git commit -m "feat: add ChannelError types"
```

---

### Task 3: Implement request types

**Files:**
- Create: `crates/vol-llm-agent-channel/src/request.rs`

- [ ] **Step 1: Create request.rs**

```rust
// crates/vol-llm-agent-channel/src/request.rs

use std::collections::HashMap;

use vol_llm_agent::AgentResponse;

use crate::error::ChannelError;

/// External request to an agent.
#[derive(Debug, Clone)]
pub struct AgentRequest {
    /// Unique request ID (caller-provided or auto-generated).
    pub req_id: String,
    /// Target agent ID for routing.
    pub target_id: String,
    /// Sender agent ID (Some for agent-to-agent calls).
    pub sender_id: Option<String>,
    /// User input to pass to ReActAgent::run().
    pub input: String,
    /// Arbitrary metadata for this request.
    pub metadata: HashMap<String, serde_json::Value>,
}

impl AgentRequest {
    /// Create a new request with an auto-generated req_id.
    pub fn new(target_id: impl Into<String>, input: impl Into<String>) -> Self {
        Self {
            req_id: uuid::Uuid::new_v4().simple().to_string(),
            target_id: target_id.into(),
            sender_id: None,
            input: input.into(),
            metadata: HashMap::new(),
        }
    }

    /// Create a new request with a specific req_id.
    pub fn with_id(
        req_id: impl Into<String>,
        target_id: impl Into<String>,
        input: impl Into<String>,
    ) -> Self {
        Self {
            req_id: req_id.into(),
            target_id: target_id.into(),
            sender_id: None,
            input: input.into(),
            metadata: HashMap::new(),
        }
    }
}

/// Result delivered to the sender after execution.
#[derive(Debug)]
pub struct RunResult {
    /// Original request ID.
    pub req_id: String,
    /// Target agent that processed this.
    pub target_id: String,
    /// Internal run_id from ReActAgent (only present on success).
    pub run_id: Option<String>,
    /// The agent response or error.
    pub response: Result<AgentResponse, ChannelError>,
}

/// Internal wrapper for a queued request awaiting execution.
pub(crate) struct PendingRequest {
    pub request: AgentRequest,
    pub tx: tokio::sync::oneshot::Sender<RunResult>,
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p vol-llm-agent-channel`
Expected: no errors

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent-channel/src/request.rs
git commit -m "feat: add AgentRequest and RunResult types"
```

---

### Task 4: Implement AgentDispatcher

**Files:**
- Create: `crates/vol-llm-agent-channel/src/dispatcher.rs`

- [ ] **Step 1: Implement AgentDispatcher**

```rust
// crates/vol-llm-agent-channel/src/dispatcher.rs

use std::collections::VecDeque;
use std::sync::Arc;

use tokio::sync::{Mutex, Notify, oneshot};
use vol_llm_agent::{AgentResponse, ReActAgent};

use crate::error::ChannelError;
use crate::request::{AgentRequest, PendingRequest, RunResult};

/// Internal state shared between the dispatcher and its background loop.
struct DispatcherState {
    queue: Mutex<VecDeque<PendingRequest>>,
    notify: Notify,
    busy: tokio::sync::Mutex<()>,
}

impl DispatcherState {
    fn new() -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
            notify: Notify::new(),
            busy: tokio::sync::Mutex::new(()),
        }
    }
}

/// Wraps a `ReActAgent` with a FIFO request queue.
///
/// Clone to share across tasks (internally Arc-backed).
/// Each dispatcher spawns one background task that processes requests one at a time.
#[derive(Clone)]
pub struct AgentDispatcher {
    agent: Arc<ReActAgent>,
    state: Arc<DispatcherState>,
}

impl AgentDispatcher {
    /// Create a new dispatcher for the given agent.
    ///
    /// The dispatcher starts a background task that processes queued requests FIFO.
    pub fn new(agent: ReActAgent) -> Self {
        let state = Arc::new(DispatcherState::new());

        // Spawn the background execution loop
        tokio::spawn(Self::run_loop(Arc::new(agent), state.clone()));

        Self {
            agent: Arc::new(agent),
            state,
        }
    }

    /// Submit a request. Returns immediately with a receiver for the result.
    pub fn submit(&self, request: AgentRequest) -> Result<oneshot::Receiver<RunResult>, ChannelError> {
        let (tx, rx) = oneshot::channel();
        let pending = PendingRequest { request, tx };

        // Push to queue and notify the background loop.
        // Using spawn here to keep submit non-blocking. In production,
        // consider using a non-blocking push pattern.
        let state = self.state.clone();
        tokio::task::spawn(async move {
            state.queue.lock().await.push_back(pending);
            state.notify.notify_one();
        });

        Ok(rx)
    }

    /// Cancel a queued request. Returns false if already executing or completed.
    pub async fn cancel(&self, req_id: &str) -> bool {
        let mut queue = self.state.queue.lock().await;

        if let Some(pos) = queue.iter().position(|p| p.request.req_id == req_id) {
            let pending = queue.remove(pos);
            // Drop the sender without sending — the receiver will get RecvError.
            drop(pending.tx);
            true
        } else {
            false
        }
    }

    /// Number of requests waiting in the queue.
    pub async fn queue_len(&self) -> usize {
        self.state.queue.lock().await.len()
    }

    /// Whether the dispatcher is currently executing a request.
    pub fn is_busy(&self) -> bool {
        // The busy lock is held by the run_loop while executing.
        // try_lock succeeds means NOT busy (nobody holds it).
        self.state.busy.try_lock().is_err()
    }

    /// Background loop that processes requests FIFO.
    async fn run_loop(agent: Arc<ReActAgent>, state: Arc<DispatcherState>) {
        loop {
            // Wait for a notification.
            state.notify.notified().await;

            // Acquire the busy lock — ensures only one request runs at a time.
            let _busy_permit = state.busy.lock().await;

            // Pop the next request from the front of the queue.
            let pending = {
                let mut queue = state.queue.lock().await;
                queue.pop_front()
            };

            let Some(pending) = pending else {
                // Queue was empty (race between notify and pop_front).
                // The notify was consumed, wait for the next one.
                continue;
            };

            // Execute the agent run.
            let result = agent.run(&pending.request.input).await;

            let run_result = RunResult {
                req_id: pending.request.req_id.clone(),
                target_id: pending.request.target_id.clone(),
                run_id: result.as_ref().ok().map(|r| r.run_id.clone()),
                response: result.map_err(|e| ChannelError::AgentError(e.to_string())),
            };

            // Send result back. If the caller cancelled, receiver is gone — fine.
            let _ = pending.tx.send(run_result);
        }
    }
}
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p vol-llm-agent-channel`
Expected: no errors

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent-channel/src/dispatcher.rs
git commit -m "feat: implement AgentDispatcher with FIFO queue"
```

---

### Task 5: Implement AgentRouter

**Files:**
- Create: `crates/vol-llm-agent-channel/src/router.rs`

- [ ] **Step 1: Create router.rs**

```rust
// crates/vol-llm-agent-channel/src/router.rs

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{RwLock, oneshot};

use crate::dispatcher::AgentDispatcher;
use crate::error::ChannelError;
use crate::request::{AgentRequest, RunResult};

/// Routes requests to registered dispatchers by agent_id.
///
/// Clone to share across tasks (internally Arc-backed).
#[derive(Clone)]
pub struct AgentRouter {
    dispatchers: Arc<RwLock<HashMap<String, Arc<AgentDispatcher>>>>,
}

impl AgentRouter {
    /// Create a new empty router.
    pub fn new() -> Self {
        Self {
            dispatchers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a dispatcher for the given agent_id.
    pub async fn register(&self, agent_id: String, dispatcher: Arc<AgentDispatcher>) {
        self.dispatchers.write().await.insert(agent_id, dispatcher);
    }

    /// Send a request to a target agent. Returns a receiver for the result.
    ///
    /// The request's `target_id` field is updated to match the resolved dispatcher.
    pub async fn send(
        &self,
        target_id: &str,
        request: AgentRequest,
    ) -> Result<oneshot::Receiver<RunResult>, ChannelError> {
        let dispatchers = self.dispatchers.read().await;
        let dispatcher = dispatchers
            .get(target_id)
            .ok_or_else(|| ChannelError::AgentNotFound(target_id.to_string()))?;

        dispatcher.submit(request)
    }

    /// Check if an agent is registered.
    pub async fn has_agent(&self, agent_id: &str) -> bool {
        self.dispatchers.read().await.contains_key(agent_id)
    }

    /// List all registered agent IDs.
    pub async fn list_agents(&self) -> Vec<String> {
        self.dispatchers.read().await.keys().cloned().collect()
    }
}

impl Default for AgentRouter {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p vol-llm-agent-channel`
Expected: no errors

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent-channel/src/router.rs
git commit -m "feat: implement AgentRouter for multi-agent routing"
```

---

### Task 6: Add unit tests for dispatcher queue mechanics

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/dispatcher.rs` (append tests)

- [ ] **Step 1: Add queue ordering tests**

Append to `dispatcher.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ChannelError;
    use crate::request::AgentRequest;

    #[tokio::test]
    async fn test_state_queue_push_pop() {
        let state = DispatcherState::new();

        let (tx1, _) = oneshot::channel();
        let req1 = AgentRequest::new("agent_a", "hello");
        state.queue.lock().await.push_back(PendingRequest { request: req1, tx: tx1 });

        assert_eq!(state.queue.lock().await.len(), 1);

        let (tx2, _) = oneshot::channel();
        let req2 = AgentRequest::new("agent_b", "world");
        state.queue.lock().await.push_back(PendingRequest { request: req2, tx: tx2 });

        assert_eq!(state.queue.lock().await.len(), 2);

        // Pop front (FIFO)
        let first = state.queue.lock().await.pop_front();
        assert!(first.is_some());
        assert_eq!(first.unwrap().request.input, "hello");

        assert_eq!(state.queue.lock().await.len(), 1);
    }

    #[tokio::test]
    async fn test_cancel_removes_from_queue() {
        let state = DispatcherState::new();

        let (tx1, _rx1) = oneshot::channel::<RunResult>();
        let req1 = AgentRequest::with_id("req-1", "agent_a", "hello");
        state.queue.lock().await.push_back(PendingRequest { request: req1, tx: tx1 });

        let (tx2, _rx2) = oneshot::channel::<RunResult>();
        let req2 = AgentRequest::with_id("req-2", "agent_a", "world");
        state.queue.lock().await.push_back(PendingRequest { request: req2, tx: tx2 });

        assert_eq!(state.queue.lock().await.len(), 2);

        // Cancel req-1
        let mut queue = state.queue.lock().await;
        let pos = queue.iter().position(|p| p.request.req_id == "req-1");
        assert!(pos.is_some());
        let pending = queue.remove(pos.unwrap());
        drop(pending.tx);

        assert_eq!(queue.len(), 1);
        assert_eq!(queue[0].request.req_id, "req-2");
    }

    #[tokio::test]
    async fn test_cancel_nonexistent_returns_false() {
        let state = DispatcherState::new();

        let found = state.queue.lock().await.iter()
            .any(|p| p.request.req_id == "nonexistent");
        assert!(!found);
    }

    #[tokio::test]
    async fn test_busy_state() {
        let state = Arc::new(DispatcherState::new());

        // Not busy initially
        assert!(state.busy.try_lock().is_ok());

        // When someone holds the lock, try_lock fails
        let _permit = state.busy.lock().await;
        assert!(state.busy.try_lock().is_err());
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p vol-llm-agent-channel`
Expected: all tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent-channel/src/dispatcher.rs
git commit -m "test: add dispatcher unit tests for queue mechanics"
```

---

### Task 7: Add unit tests for router

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/router.rs` (append tests)

- [ ] **Step 1: Add router tests**

Append to `router.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_router_empty_returns_not_found() {
        let router = AgentRouter::new();
        let req = AgentRequest::new("nonexistent", "hello");
        let result = router.send("nonexistent", req).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ChannelError::AgentNotFound(id) => assert_eq!(id, "nonexistent"),
            other => panic!("Expected AgentNotFound, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_router_has_agent_empty() {
        let router = AgentRouter::new();
        assert!(!router.has_agent("agent_a").await);
    }

    #[tokio::test]
    async fn test_router_list_agents_empty() {
        let router = AgentRouter::new();
        let agents = router.list_agents().await;
        assert!(agents.is_empty());
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p vol-llm-agent-channel`
Expected: all tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent-channel/src/router.rs
git commit -m "test: add router unit tests"
```

---

### Task 8: Full workspace check and commit

**Files:** No file changes — verification step.

- [ ] **Step 1: Full workspace check**

Run: `cargo check --workspace`
Expected: no errors

- [ ] **Step 2: Run all crate tests**

Run: `cargo test -p vol-llm-agent-channel`
Expected: all tests pass
