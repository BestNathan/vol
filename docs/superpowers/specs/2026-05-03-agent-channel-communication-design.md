# Agent Channel Communication Design

**Date**: 2026-05-03
**Status**: Draft
**Author**: Claude Code

## Requirements

See `docs/superpowers/requirement/2026-05-03-agent-channel-communication-requirement.md`.

TL;DR:
- Submit-and-await via `oneshot::Receiver<RunResult>`
- Structured request with `req_id`, `target_id`, `sender_id`, `input`, `metadata`
- Single-threaded FIFO queue — one execution at a time
- Queue-only cancellation — `cancel(req_id)` only works on queued (not executing) requests
- Multi-agent routing via `AgentRouter`
- New crate: `vol-llm-agent-channel`

## Architecture

Two layers:

**Layer 1: `AgentDispatcher`** — wraps a single `ReActAgent` with request queueing. Manages one background loop that processes requests FIFO, one at a time.

**Layer 2: `AgentRouter`** — manages multiple `AgentDispatcher` instances. Routes messages between agents by `target_id`, enabling agent-to-agent communication.

### Relationship

```
AgentRouter (shared across agents)
  ├── Dispatcher A ──> ReActAgent A
  ├── Dispatcher B ──> ReActAgent B
  └── Dispatcher C ──> ReActAgent C
```

Agents don't know about the dispatcher. Callers interact with `AgentRouter` or `AgentDispatcher` directly.

## Crate Structure

```
crates/vol-llm-agent-channel/
├── Cargo.toml
└── src/
    ├── lib.rs           # Crate root, re-exports
    ├── request.rs       # AgentRequest, RunResult, PendingRequest
    ├── dispatcher.rs    # AgentDispatcher: single-agent queue
    ├── router.rs        # AgentRouter: multi-agent routing
    └── error.rs         # ChannelError type
```

## Key Types

### `request.rs`

```rust
/// External request to an agent.
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

/// Result delivered to the sender after execution.
pub struct RunResult {
    /// Original request ID.
    pub req_id: String,
    /// Target agent that processed this.
    pub target_id: String,
    /// Internal run_id from ReActAgent (only present on success).
    pub run_id: Option<String>,
    /// The agent response or error.
    pub response: Result<vol_llm_agent::AgentResponse, ChannelError>,
}
```

### `dispatcher.rs`

```rust
/// Wraps a ReActAgent with a FIFO request queue.
/// Clone to share across tasks (internally Arc-backed).
#[derive(Clone)]
pub struct AgentDispatcher {
    agent: Arc<ReActAgent>,
    queue: Arc<Mutex<VecDeque<PendingRequest>>>,
    // ... internal channel to wake background task
}

impl AgentDispatcher {
    /// Create a new dispatcher for the given agent.
    pub fn new(agent: Arc<ReActAgent>) -> Self;

    /// Submit a request. Returns immediately with a receiver for the result.
    pub fn submit(&self, request: AgentRequest) -> Result<oneshot::Receiver<RunResult>, ChannelError>;

    /// Cancel a queued request. Returns false if already executing or completed.
    pub fn cancel(&self, req_id: &str) -> bool;

    /// Number of requests waiting in the queue.
    pub fn queue_len(&self) -> usize;

    /// Whether the dispatcher is currently executing a request.
    pub fn is_busy(&self) -> bool;
}
```

Internal:

```rust
struct PendingRequest {
    request: AgentRequest,
    tx: oneshot::Sender<RunResult>,
}
```

### `router.rs`

```rust
/// Routes requests to registered dispatchers by agent_id.
/// Clone to share across tasks (internally Arc-backed).
#[derive(Clone)]
pub struct AgentRouter {
    dispatchers: Arc<RwLock<HashMap<String, Arc<AgentDispatcher>>>>,
}

impl AgentRouter {
    pub fn new() -> Self;

    /// Register a dispatcher for the given agent_id.
    pub fn register(&self, agent_id: String, dispatcher: Arc<AgentDispatcher>);

    /// Send a request to a target agent. Returns a receiver for the result.
    pub fn send(&self, target_id: &str, request: AgentRequest)
        -> Result<oneshot::Receiver<RunResult>, ChannelError>;
}
```

### `error.rs`

```rust
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

## Data Flow

### Submit → Execute → Reply

```
Caller.submit(req) ──────> AgentDispatcher.submit(req)
                               │
                               ├── creates oneshot (tx, rx)
                               ├── pushes PendingRequest { req, tx } to queue
                               └── returns rx immediately
                                   │
Caller.await(rx) <─────────────────────────┘

Background Loop (one task per dispatcher):
    loop {
        // Wait for queue to have items
        let req = wait_for_queue_item().await;

        // Pop from queue and execute
        let pending = queue.lock().pop_front();
        let result = agent.run(&pending.request.input).await;

        // Send result back to caller
        let run_result = RunResult {
            req_id: pending.request.req_id,
            target_id: pending.request.target_id,
            run_id: result.as_ref().ok().map(|r| r.run_id.clone()),
            response: result.map_err(ChannelError::AgentError),
        };
        let _ = pending.tx.send(run_result);
    }
```

### Cancel

```
Caller.cancel(req_id) ──> AgentDispatcher.cancel(req_id)
                               │
                               ├── lock queue
                               ├── find PendingRequest by req_id
                               ├── remove from queue (do NOT send result)
                               └── drop tx → caller's rx gets RecvError
```

## Background Task Lifecycle

Each `AgentDispatcher` spawns a tokio background task on creation. The task runs the execution loop until the dispatcher is dropped. The task detects dispatcher drop via a `watch::Receiver<bool>` or by checking if the queue sender still exists.

## Edge Case Handling

| Edge Case | Behavior |
|-----------|----------|
| Submit while busy | Request queued, `oneshot::Receiver` returned immediately |
| Multiple requests queued | Executed FIFO, one at a time |
| Cancel queued request | Removed from queue, sender's channel closed |
| Cancel executing request | Returns `false`, no effect |
| Cancel completed request | Returns `false` |
| Agent run fails | `ChannelError::AgentError` sent to caller |
| Dispatcher dropped | All pending senders receive errors |
| Duplicate `req_id` | Treated as separate request (no dedup) |

## Out of Scope

- Concurrent execution (multiple runs in parallel)
- Mid-execution cancellation
- Streaming events to caller (use existing `AgentStreamEvent` broadcast)
- Persistent queue
- WebSocket/HTTP API layer

## Testing Strategy

- Unit tests for dispatcher queue ordering
- Unit tests for cancel on queued request
- Unit tests for router routing (register, send to correct dispatcher)
- Integration test: mock ReActAgent, submit 3 requests, verify FIFO order
- Integration test: cancel middle request, verify it doesn't execute
