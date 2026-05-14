# Design: MCP Manager for Connection Lifecycle

> **Status:** Draft — pending user approval
> **Date:** 2026-05-13
> **Related:** [Requirements](../requirement/2026-05-13-mcp-manager-requirement.md)

## 1. Architecture & Data Model

### 1.1 Overview

`McpManager` replaces `McpSession` as the central MCP client coordinator. It manages per-server connection states, spawns background reconnect tasks on failure, caches discovered capabilities at connect time, and filters them by connection state so agents only see what's available.

### 1.2 ServerStatus Enum

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum ServerStatus {
    Connected,
    Disconnected, // Gracefully disconnected
    Connecting,   // In progress
    Error(String), // Failure detail
}
```

### 1.3 ServerState Struct

```rust
struct ServerState {
    config: McpServerConfig,
    status: ServerStatus,
    retry_count: usize,
    running_service: Option<RunningService<RoleClient, ClientInfo>>,
    cancel_token: CancellationToken,
    cached_tools: Vec<McpToolInfo>,
    cached_resources: Vec<Resource>,
    cached_resource_templates: Vec<ResourceTemplate>,
    cached_prompts: Vec<Prompt>,
    reconnect_handle: Option<tokio::task::JoinHandle<()>>,
}
```

Key design: internal mutability via `Arc<RwLock<>>`. All public methods take `&self`.

### 1.4 McpManager Struct

```rust
pub struct McpManager {
    servers: Arc<RwLock<HashMap<String, ServerState>>>,
    max_retries: usize,              // default: 5
    backoff_min: Duration,           // default: 1s
    backoff_max: Duration,           // default: 30s
}
```

### 1.5 Public API

```rust
impl McpManager {
    pub fn new(configs: Vec<McpServerConfig>) -> Self;
    pub fn with_max_retries(self, max: usize) -> Self;
    pub fn with_backoff(self, min: Duration, max: Duration) -> Self;

    pub async fn connect(&self) -> Result<(), McpError>;
    pub async fn disconnect(&self) -> Result<(), McpError>;
    pub async fn disconnect_server(&self, name: &str) -> Result<(), McpError>;

    pub fn server_status(&self) -> HashMap<String, ServerStatus>;
    pub async fn reconnect(&self, name: &str) -> Result<(), McpError>;

    // Tool protocol
    pub async fn list_all_tools(&self) -> Vec<McpToolInfo>;
    pub async fn call_tool(&self, server: &str, tool: &str, args: serde_json::Value)
        -> Result<serde_json::Value, McpError>;

    // Resource protocol
    pub async fn list_all_resources(&self) -> Vec<Resource>;
    pub async fn read_resource(&self, uri: &str) -> Result<(String, Option<String>), McpError>;
    pub async fn list_all_resource_templates(&self) -> Vec<ResourceTemplate>;

    // Prompt protocol
    pub async fn list_all_prompts(&self) -> Vec<Prompt>;
    pub async fn get_prompt(&self, name: &str, args: Option<HashMap<String, serde_json::Value>>)
        -> Result<(Option<String>, Vec<PromptMessage>), McpError>;
    pub async fn complete_prompt(&self, name: &str, argument_name: &str, value: &str)
        -> Result<CompletePromptResult, McpError>;
}
```

### 1.6 Design Principles

- `&self` methods throughout — no `&mut self` needed
- `connect()` is idempotent — calling twice on already-connected servers is a no-op for those servers
- Background reconnect tasks are spawned inside `connect()`, requiring no external event loop
- Caches populated at connect time, cleared on disconnect, filtered by state for `list_*` methods
- Live calls (`call_tool`, `read_resource`, `get_prompt`) always query the running service directly

## 2. Error Handling & Edge Cases

### 2.1 McpError Variants

```rust
pub enum McpError {
    ConfigParse { path: String, detail: String },
    ServerNotFound(String),
    ConnectionFailed { server: String, detail: String },
    InitializeTimeout { server: String },
    ToolCallFailed { server: String, tool: String, detail: String },
    ResourceReadFailed { server: String, uri: String, detail: String },
    PromptGetFailed { server: String, name: String, detail: String },
    TransportError(String),
    ServerDisconnected(String), // server_name
}
```

### 2.2 Key Edge Cases

- **Disconnect during tool call:** `call_tool()` checks live service before calling. If dead, returns `McpError::ServerDisconnected(name)`.
- **Retry exhaustion:** After `max_retries` failures, state becomes `Error("max retries exceeded")`. Auto-reconnect stops. Caches cleared. Manual `reconnect()` resets counter.
- **Concurrent reconnects:** `reconnect()` cancels in-progress attempt via per-server `CancellationToken`, starts fresh.
- **Empty config:** Valid manager with zero servers. `connect()` is no-op.
- **Reconnect on Connected server:** Gracefully disconnects first, then reconnects with reset retry counter.
- **Partial server failure:** Manager operates normally for connected servers. Failed servers show `Error` status.
- **Tool call on never-connected server:** Returns `ServerNotFound` or `ServerDisconnected` — never panics or hangs.

## 3. Connection Lifecycle Flow

### 3.1 Initial connect()

For each server config:
1. Set state → `Connecting`
2. Spawn `TokioChildProcess` (command + args + env)
3. Create `rmcp` transport (stdio)
4. `ClientInfo.serve_with_ct(transport, cancel_token)`
5. Wait for peer ready (10s timeout)
6. Fetch & cache: tools, resources, resource templates, prompts
7. Set state → `Connected`

On failure (any step 2-5):
1. Set state → `Error(detail)`
2. Increment `retry_count`
3. If `retry_count < max_retries`: wait backoff, spawn background reconnect
4. If `retry_count >= max_retries`: clear caches, stop auto-reconnects

### 3.2 Background Reconnect Loop

For each server in `Error` state with retries remaining:
1. Spawn async task with server-specific `CancellationToken`
2. Wait backoff delay
3. Loop: Connecting → attempt connect → success/exit or failure/retry or cancel/exit

### 3.3 Manual reconnect()

1. Cancel any in-progress reconnect via `CancellationToken`
2. If `Connected` → disconnect single server first
3. Reset `retry_count` to 0
4. Run full connection flow
5. On failure → `retry_count++`, auto-reconnect resumes if under limit

### 3.4 Disconnect()

For each server (or single server by name):
1. Cancel reconnect `CancellationToken`
2. Kill `TokioChildProcess`
3. Stop `RunningService`
4. Clear caches
5. Set state → `Disconnected`

## 4. API Details

### 4.1 Return Types

- `call_tool()` → `serde_json::Value` — raw tool result from `rmcp::CallToolResult.content`
- `read_resource()` → `(String, Option<String>)` — text content and optional MIME type. Multiple contents concatenated with newline.
- `get_prompt()` → `(Option<String>, Vec<PromptMessage>)` — description + messages from `rmcp::GetPromptResult`
- `complete_prompt()` → `rmcp::CompletePromptResult` — autocompletion suggestions

### 4.2 Cache Behavior

- `list_*` methods return cached results filtered by `Connected` servers
- On `Connected` → `Disconnected` transition: caches cleared, tools excluded
- On successful reconnect: caches invalidated and re-fetched
- `call_*`/`read_*`/`get_*` methods always query live service

### 4.3 Migration Impact

| Component | Current | After |
|-----------|---------|-------|
| `vol-llm-mcp/src/lib.rs` | re-exports `McpSession` | re-exports `McpManager` |
| `vol-llm-tool/src/mcp_tool.rs` | `Arc<McpSession>` | `Arc<McpManager>` |
| `vol-llm-tool/src/registry.rs` | `register_from_mcp(Arc<McpSession>)` | `register_from_mcp(Arc<McpManager>)` |
| `vol-llm-agent/src/react/agent.rs` | `mcp_session: Option<Arc<McpSession>>` | `mcp_manager: Option<Arc<McpManager>>` |
| `vol-llm-agent/src/react/config_builder.rs` | `with_mcp_from_config()` creates `McpSession` | creates `McpManager` |
| `docs_rs_mcp_example.rs` | works as-is | works as-is (transparent migration) |

## 5. Testing Approach

### 5.1 Unit Tests (inline in `manager.rs`)

1. **Empty config** — `McpManager::new([])` → valid manager, no-op connect
2. **State transitions** — `Connecting` → `Error` → `Connecting` → ... → exhausted
3. **Max retry exhaustion** — invalid command, verify exactly N attempts then stop
4. **Manual reconnect after exhaustion** — retry counter resets, attempt made
5. **Disconnected tools excluded** — two servers, one fails, only good tools listed
6. **Cancel concurrent reconnect** — second reconnect cancels first

### 5.2 Integration Tests

- All 11 existing `vol-llm-mcp` tests updated to use `McpManager`
- `docs_rs_mcp_example.rs` must still discover 4 tools and complete
- `cargo test -p vol-llm-mcp` passes

### 5.3 Coverage Goals

- All `McpError` variants exercised
- All state transitions covered
- Cache invalidation on disconnect verified
- Partial server failure tested

## 6. Constraints

- Uses existing `rmcp 1.6` crate (already a dependency)
- Only stdio child process transport (no HTTP/SSE for McpManager)
- Background reconnect spawned within `connect()`, no external event loop
- Must be `Send + Sync` for use across async contexts
- Backwards-compatible at agent config level (same `.with_mcp_from_config()` builder)
- No explicit OTel/Loki integration — `tracing::info!`/`tracing::warn!`/`tracing::error!` sufficient
