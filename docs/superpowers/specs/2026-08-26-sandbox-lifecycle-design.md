# Sandbox Lifecycle Management Design

**Date:** 2026-08-26  
**Status:** Draft  
**Issue:** #59

## Overview

This document specifies the refactoring of the sandbox abstraction to introduce explicit instance lifecycle management. The current `Sandbox` trait conflates profile configuration, instance identity, execution handle, and lifecycle management. This design separates these concerns into distinct abstractions.

## Goals

1. Introduce stable `SandboxId` distinct from sandbox profile/name
2. Define explicit sandbox lifecycle states with validated transitions
3. Provide `SandboxManager` for orchestration and `SandboxProvider` for backend adapters
4. Support explicit start/stop/destroy lifecycle semantics
5. Model pause/resume where supported
6. Make unsupported lifecycle capabilities discoverable via `SandboxCapabilities`
7. Separate lifecycle destruction from connection/resource cleanup
8. Preserve existing execution and filesystem isolation APIs on `Sandbox`
9. Merge `SandboxRegistry` responsibilities into `SandboxManager`

## Non-Goals

- Frontend UI changes (separate task)
- Docker/Kubernetes provider implementations (future work)
- Firecracker/Wasm provider migration (out of scope for this refactor)
- Persistent `SandboxStore` (trait only; in-memory implementation for now)

## Scope

- **Backend only** — no frontend changes
- **Big bang refactor** — break existing code, then fix
- **Providers:** Local, Tmp, SSH (Firecracker and Wasm deferred)
- **Storage:** `SandboxStore` trait with `InMemorySandboxStore`
- **Registry:** Merge `SandboxRegistry` into `SandboxManager`

## Architecture

### Core Types

#### SandboxId

Stable instance identifier, distinct from profile name.

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SandboxId(String);  // e.g., "sb_01JXYZ..."
```

Generated via ULID or UUID v7 (time-ordered). Format: `sb_<ulid>`.

#### SandboxStatus

Explicit lifecycle states.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxStatus {
    Creating,
    Created,
    Starting,
    Running,
    Pausing,
    Paused,
    Stopping,
    Stopped,
    Destroying,
    Destroyed,
    Failed,
}
```

**Valid transitions:**
- `Creating -> Created -> Starting -> Running`
- `Running -> Pausing -> Paused -> Starting -> Running` (resume)
- `Running/Paused -> Stopping -> Stopped`
- `Stopped -> Destroying -> Destroyed`
- Any state -> `Failed` (on error)

`SandboxManager` enforces these transitions and rejects invalid ones.

#### SandboxCapabilities

Discoverable backend capabilities.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxCapabilities {
    pub persistent: bool,      // survives process restart
    pub pausable: bool,        // supports pause/resume
    pub stoppable: bool,       // supports stop (preserves workspace)
    pub destroyable: bool,     // supports destroy (removes resources)
}
```

**Provider defaults:**
- Local: `{persistent: true, pausable: false, stoppable: false, destroyable: false}`
- Tmp: `{persistent: false, pausable: false, stoppable: false, destroyable: true}`
- SSH: `{persistent: true, pausable: false, stoppable: false, destroyable: false}`

### Core Traits

#### SandboxProvider

Backend lifecycle adapter.

```rust
#[async_trait]
pub trait SandboxProvider: Send + Sync {
    fn kind(&self) -> SandboxKind;
    fn capabilities(&self) -> SandboxCapabilities;

    async fn create(&self, spec: &SandboxSpec) -> Result<BackendSandboxRef>;
    async fn get(&self, backend_id: &str) -> Result<SandboxRef>;
    async fn list(&self) -> Result<Vec<SandboxInfo>>;

    async fn start(&self, backend_id: &str) -> Result<()>;
    async fn pause(&self, backend_id: &str) -> Result<()>;
    async fn resume(&self, backend_id: &str) -> Result<()>;
    async fn stop(&self, backend_id: &str) -> Result<()>;
    async fn destroy(&self, backend_id: &str) -> Result<()>;
}
```

**`BackendSandboxRef`:**
```rust
pub struct BackendSandboxRef {
    pub backend_id: String,
    pub sandbox: Arc<dyn Sandbox>,
}
```

**Responsibilities:**
- Translate common lifecycle model into backend-specific operations
- Manage backend-specific instance storage (e.g., `HashMap<backend_id, Arc<dyn Sandbox>>`)
- Implement lifecycle operations (create, start, pause, resume, stop, destroy)
- Report unsupported operations via `SandboxCapabilities`

#### Sandbox

Instance handle for execution and filesystem operations.

```rust
#[async_trait]
pub trait Sandbox: Send + Sync {
    fn id(&self) -> &SandboxId;
    fn kind(&self) -> SandboxKind;
    fn status(&self) -> SandboxStatus;
    fn root_path(&self) -> Option<&Path>;

    async fn execute(&self, req: CommandRequest) -> Result<CommandOutput>;
    async fn read_file(&self, path: &Path) -> Result<Vec<u8>>;
    async fn write_file(&self, path: &Path, content: &[u8]) -> Result<()>;
    async fn create_dir_all(&self, path: &Path) -> Result<()>;
    async fn read_dir(&self, path: &Path) -> Result<Vec<DirEntry>>;
    async fn metadata(&self, path: &Path) -> Result<FileMetadata>;

    fn resolve_path(&self, rel: &Path) -> Result<PathBuf>;
}
```

**Removed from current `Sandbox` trait:**
- `name()` — replaced by `id()` for instance identity; profile name lives in `SandboxSpec`
- `start()` / `cleanup()` — moved to `SandboxProvider` lifecycle methods
- `bind_metadata()` — moved to `SandboxSpec` (metadata passed at creation time)

#### SandboxStore

Instance metadata persistence.

```rust
#[async_trait]
pub trait SandboxStore: Send + Sync {
    async fn insert(&self, record: SandboxRecord) -> Result<()>;
    async fn get(&self, id: &SandboxId) -> Result<Option<SandboxRecord>>;
    async fn list(&self, filter: Option<SandboxFilter>) -> Result<Vec<SandboxRecord>>;
    async fn update_status(&self, id: &SandboxId, status: SandboxStatus) -> Result<()>;
    async fn delete(&self, id: &SandboxId) -> Result<()>;
}

pub struct SandboxRecord {
    pub id: SandboxId,
    pub profile: String,                    // e.g., "coding", "devbox"
    pub provider_kind: SandboxKind,         // e.g., "local", "ssh"
    pub backend_id: String,                 // provider-specific identifier
    pub status: SandboxStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
}

pub struct SandboxFilter {
    pub profile: Option<String>,
    pub provider_kind: Option<SandboxKind>,
    pub status: Option<SandboxStatus>,
}
```

**`InMemorySandboxStore`:**
```rust
pub struct InMemorySandboxStore {
    records: RwLock<HashMap<SandboxId, SandboxRecord>>,
}
```

Future: `SqliteSandboxStore`, `PostgresSandboxStore` for persistence.

### SandboxSpec

Replaces TOML config + `bind_metadata`.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxSpec {
    pub name: String,                    // profile name
    pub provider: SandboxKind,           // "local", "tmp", "ssh"
    
    #[serde(flatten)]
    pub config: SandboxProviderConfig,
    
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "provider", rename_all = "lowercase")]
pub enum SandboxProviderConfig {
    Local { work_dir: Option<PathBuf> },
    Tmp { sub_dir: Option<String> },
    Ssh { 
        host: String, 
        user: String, 
        work_dir: PathBuf,
        port: Option<u16>,
        key_path: Option<PathBuf>,
    },
}
```

**Migration:** Existing `.agent/sandboxes/*.toml` files are loaded as `SandboxSpec` by `SandboxManager`.

### SandboxManager

Unified orchestration service (replaces `SandboxRegistry`).

```rust
pub struct SandboxManager {
    providers: HashMap<SandboxKind, Arc<dyn SandboxProvider>>,
    store: Arc<dyn SandboxStore>,
    specs: RwLock<HashMap<String, SandboxSpec>>,  // profile name -> spec
}

impl SandboxManager {
    /// Load sandbox profiles from directory (replaces SandboxRegistry::load)
    pub async fn load_profiles(&mut self, sandboxes_dir: &Path) -> Result<()>;
    
    /// Register a profile spec programmatically
    pub fn register_profile(&mut self, spec: SandboxSpec);
    
    /// Create a new sandbox instance from a profile
    pub async fn create(&self, profile: &str) -> Result<SandboxId>;
    
    /// Get a sandbox handle by ID (replaces acquire)
    pub async fn get(&self, id: &SandboxId) -> Result<SandboxRef>;
    
    /// List all sandbox instances
    pub async fn list(&self, filter: Option<SandboxFilter>) -> Result<Vec<SandboxInfo>>;
    
    /// Lifecycle operations
    pub async fn start(&self, id: &SandboxId) -> Result<()>;
    pub async fn pause(&self, id: &SandboxId) -> Result<()>;
    pub async fn resume(&self, id: &SandboxId) -> Result<()>;
    pub async fn stop(&self, id: &SandboxId) -> Result<()>;
    pub async fn destroy(&self, id: &SandboxId) -> Result<()>;
    
    /// Get the default sandbox (creates a TmpSandbox if none exists)
    pub async fn default(&self) -> Result<SandboxRef>;
    
    /// Register a pre-existing sandbox instance (for backward compat)
    pub async fn register_instance(&self, spec: SandboxSpec, sandbox: SandboxRef) -> Result<SandboxId>;
}
```

**Key behaviors:**
1. `create(profile)` looks up the spec, routes to the correct provider, creates instance, stores record
2. `get(id)` looks up record, routes to correct provider, returns sandbox handle
3. Lifecycle methods validate state transitions before delegating to provider
4. `default()` returns the first sandbox if exactly one exists; if zero or multiple exist, creates a fresh TmpSandbox with a random `sub_dir`
5. `register_instance()` allows backward compat (e.g., `LocalSandbox` registered at server startup). Assumes the sandbox is already in `Running` state; stores the record with `status: Running`

## Provider Implementations

### LocalSandboxProvider

```rust
pub struct LocalSandboxProvider {
    instances: Arc<RwLock<HashMap<String, Arc<LocalSandbox>>>>,
}
```

**Capabilities:** `{persistent: true, pausable: false, stoppable: false, destroyable: false}`

**Lifecycle:**
- `create(spec)` — create `LocalSandbox` with optional `work_dir`
- `get(backend_id)` — lookup by `work_dir` path
- `start/pause/resume/stop` — no-op (not supported)
- `destroy` — no-op (can't destroy host filesystem)

### TmpSandboxProvider

```rust
pub struct TmpSandboxProvider {
    instances: Arc<RwLock<HashMap<String, Arc<TmpSandbox>>>>,
    counter: Arc<AtomicUsize>,
}
```

**Capabilities:** `{persistent: false, pausable: false, stoppable: false, destroyable: true}`

**Lifecycle:**
- `create(spec)` — create `TmpSandbox`, apply `sub_dir` from spec, call `start()` to create directory
- `destroy(backend_id)` — call `cleanup()` to remove directory, remove from instances

### SSHSandboxProvider

```rust
pub struct SSHSandboxProvider {
    instances: Arc<RwLock<HashMap<String, Arc<SSHSandbox>>>>,
    session_pool: Arc<SSHSessionPool>,
}
```

**Capabilities:** `{persistent: true, pausable: false, stoppable: false, destroyable: false}`

**Lifecycle:**
- `create(spec)` — create `SSHSandbox`, establish connection, store in instances
- `get(backend_id)` — lookup by `user@host`
- `start` — reconnect if disconnected
- `stop` — disconnect session
- `destroy` — remove from instances, disconnect (NOT destroy remote machine)

## Migration Plan

### Breaking Changes

1. **Remove `SandboxRegistry`** entirely
2. **Change `Sandbox` trait:**
   - Remove `name()`, `start()`, `cleanup()`, `bind_metadata()`
   - Add `id()`, `status()`
3. **Update all callers** to use `SandboxManager` instead of `SandboxRegistry`

### Files to Modify

**`crates/vol-llm-sandbox/src/`:**
- `lib.rs` — new types (`SandboxId`, `SandboxStatus`, `SandboxCapabilities`), traits (`SandboxProvider`, `SandboxStore`), refactor `Sandbox` trait
- `local.rs` — refactor `LocalSandbox` to implement new `Sandbox` trait, add `LocalSandboxProvider`
- `tmp.rs` — refactor `TmpSandbox`, add `TmpSandboxProvider`
- `ssh/` — refactor `SSHSandbox`, add `SSHSandboxProvider`
- `registry.rs` — **DELETE**, replaced by `SandboxManager`
- `manager.rs` — **NEW** — `SandboxManager`, `SandboxSpec`
- `store.rs` — **NEW** — `SandboxStore` trait, `InMemorySandboxStore`

**`crates/vol-agent-server/src/`:**
- `data_plane/core.rs` — use `SandboxManager` instead of `SandboxRegistry`
- `control_plane/core.rs` — use `SandboxManager`
- `data_plane/handlers/sandbox.rs` — update to use `SandboxManager`

**`crates/vol-llm-runtime/src/`:**
- `agent.rs` — update sandbox acquisition to use `SandboxManager`

### Backward Compatibility Shims

- Add `SandboxManager::register_instance()` to allow existing code to register pre-built sandboxes
- Keep `SandboxManager::default()` behavior (returns/creates TmpSandbox)

### RPC Protocol Changes

**`sandbox.list` response:**
```json
{
  "sandboxes": [
    {
      "id": "sb_01JXYZ...",
      "profile": "coding",
      "kind": "local",
      "status": "running",
      "root_path": "/app",
      "capabilities": {
        "persistent": true,
        "pausable": false,
        "stoppable": false,
        "destroyable": false
      }
    }
  ]
}
```

**New RPC methods:**
- `sandbox.create` — `{profile: string} -> {id: string}`
- `sandbox.start` — `{id: string} -> {}`
- `sandbox.pause` — `{id: string} -> {}`
- `sandbox.resume` — `{id: string} -> {}`
- `sandbox.stop` — `{id: string} -> {}`
- `sandbox.destroy` — `{id: string} -> {}`

## Testing Strategy

### Unit Tests

**`SandboxManager`:**
- `test_create_sandbox` — verify instance creation, record storage
- `test_get_sandbox` — verify retrieval by ID
- `test_list_sandboxes` — verify filtering by profile/kind/status
- `test_lifecycle_transitions` — verify valid transitions (Running -> Paused -> Running)
- `test_invalid_transitions` — verify rejection of invalid transitions (Destroyed -> Running)
- `test_default_sandbox` — verify default behavior (returns/creates TmpSandbox)

**`SandboxProvider` implementations:**
- `test_local_provider_create` — verify LocalSandbox creation
- `test_tmp_provider_create_destroy` — verify TmpSandbox creation and cleanup
- `test_ssh_provider_lifecycle` — verify SSH connection/disconnection

**`InMemorySandboxStore`:**
- `test_insert_get` — verify record storage and retrieval
- `test_list_filter` — verify filtering
- `test_update_status` — verify status updates
- `test_delete` — verify record deletion

### Integration Tests

**`sandbox_lifecycle_integration.rs`:**
- `test_full_lifecycle` — create -> start -> pause -> resume -> stop -> destroy
- `test_provider_routing` — verify correct provider is used for each kind
- `test_concurrent_access` — verify thread safety

## Acceptance Criteria

- [ ] Introduce stable `SandboxId` distinct from profile/name
- [ ] Define explicit sandbox lifecycle states with validated transitions
- [ ] Provide `SandboxManager` for orchestration (replaces `SandboxRegistry`)
- [ ] Provide `SandboxProvider` trait for backend adapters
- [ ] Implement `LocalSandboxProvider`, `TmpSandboxProvider`, `SSHSandboxProvider`
- [ ] Support explicit start/stop/destroy lifecycle semantics
- [ ] Model pause/resume where supported (via `SandboxCapabilities`)
- [ ] Make unsupported lifecycle capabilities discoverable
- [ ] Separate lifecycle destruction from connection/resource cleanup
- [ ] Preserve existing execution and filesystem isolation APIs on `Sandbox`
- [ ] Provide `SandboxStore` trait with `InMemorySandboxStore`
- [ ] Add lifecycle transition tests and invalid-transition tests
- [ ] Update `vol-agent-server` to use `SandboxManager`
- [ ] Update `vol-llm-runtime` to use `SandboxManager`
- [ ] Update RPC protocol to expose new lifecycle methods
- [ ] All existing tests pass after migration
- [ ] Coverage ≥ 80% for new code

## Future Work

- Persistent `SandboxStore` implementations (SQLite, Postgres)
- Docker/Kubernetes provider implementations
- Firecracker/Wasm provider migration
- Frontend UI for lifecycle management
- Session-scoped sandbox binding (issue #58)

## References

- Issue #59: refactor: introduce explicit sandbox instance lifecycle management
- Issue #58: session-scoped sandbox binding (depends on this design)
- [[sandbox-lifecycle]] — current design (to be replaced)
- [[vol-llm-sandbox-crate]] — implementation details
