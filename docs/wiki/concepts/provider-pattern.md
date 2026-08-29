---
type: concept
category: pattern
tags: [provider, adapter, sandbox, backend-agnostic]
created: 2026-08-27
updated: 2026-08-28
source_count: 1
---

# Provider Pattern

## Overview
The Provider pattern is a backend adapter pattern that separates backend-specific lifecycle management from orchestration logic. A backend implements the `SandboxProvider` trait, allowing the `SandboxManager` to orchestrate instances without knowing backend-specific details.

> **As of 2026-08-28 only three providers exist:** `local`, `tmp`, and `ssh`. `FirecrackerSandbox` and `WasmSandbox` implement the `Sandbox` trait but have **no `SandboxProvider` impl**, so they cannot be reached through `SandboxManager` — a `provider = "firecracker"` profile parses and then fails to instantiate with `UnknownType`. See [[sandbox-architecture]].

## How It Works

### Trait Definition
```rust
#[async_trait]
pub trait SandboxProvider: Send + Sync {
    fn kind(&self) -> &str;
    fn capabilities(&self) -> SandboxCapabilities;

    async fn create(&self, spec: &SandboxSpec) -> SandboxResult<BackendSandboxRef>;
    async fn get(&self, backend_id: &str) -> SandboxResult<Arc<dyn Sandbox>>;
    async fn list(&self) -> SandboxResult<Vec<SandboxInfo>>;

    async fn start(&self, backend_id: &str) -> SandboxResult<()>;
    async fn pause(&self, backend_id: &str) -> SandboxResult<()>;
    async fn resume(&self, backend_id: &str) -> SandboxResult<()>;
    async fn stop(&self, backend_id: &str) -> SandboxResult<()>;
    async fn destroy(&self, backend_id: &str) -> SandboxResult<()>;
}
```

### Backend Identification
Each provider returns a unique `kind()` string:
- `LocalSandboxProvider` → `"local"`
- `TmpSandboxProvider` → `"tmp"`
- `SSHSandboxProvider` → `"ssh"`

(`"firecracker"` and `"wasm"` are valid config values but have no provider implementation.)

### Capability Declaration
Providers declare their capabilities via `capabilities()`:
```rust
impl SandboxProvider for LocalSandboxProvider {
    fn capabilities(&self) -> SandboxCapabilities {
        SandboxCapabilities {
            persistent: true,   // Local sandbox persists
            pausable: false,    // Cannot pause local process
            stoppable: false,   // Cannot stop without destroying
            destroyable: false, // Cannot destroy host filesystem
        }
    }
}
```

### Instance Creation
Providers create backend-specific instances and return a `BackendSandboxRef`:
```rust
pub struct BackendSandboxRef {
    pub backend_id: String,      // Backend-specific identifier
    pub sandbox: Arc<dyn Sandbox>, // Execution handle
}
```

The `backend_id` is provider-specific:
- Local: filesystem path
- Tmp: temp directory path
- SSH: `user@host` identifier

Note the asymmetry: for `local` and `ssh` the `backend_id` is a pure function of the spec, so it conveys no per-instance information, while `tmp` generates a fresh path per create. This is what decides whether an instance is worth recording — see [[sandbox-lifecycle]].

### Instance Retrieval
Providers retrieve existing instances by `backend_id`:
```rust
async fn get(&self, backend_id: &str) -> SandboxResult<Arc<dyn Sandbox>> {
    // Provider-specific lookup logic
}
```

### Lifecycle Operations
Providers implement backend-specific lifecycle operations:
- `start()` — initialize backend resources
- `pause()` — suspend execution (if supported)
- `resume()` — resume execution (if supported)
- `stop()` — stop execution, preserve resources
- `destroy()` — release all resources

Unsupported operations return errors or no-op based on capabilities.

## Benefits

### Backend Agnostic
Orchestration code (`SandboxManager`) operates on abstract interfaces without knowing backend details.

### Extensibility
New backends can be added by implementing `SandboxProvider` without modifying orchestration logic.

### Capability Discovery
Backends declare what they support; orchestration respects those limits.

### Separation of Concerns
- **Provider** — backend-specific lifecycle management
- **Sandbox** — execution interface
- **Manager** — orchestration and instance tracking

## Implementation Example

### LocalSandboxProvider
```rust
pub struct LocalSandboxProvider;

#[async_trait]
impl SandboxProvider for LocalSandboxProvider {
    fn kind(&self) -> &str { "local" }

    fn capabilities(&self) -> SandboxCapabilities {
        SandboxCapabilities {
            persistent: true,
            pausable: false,
            stoppable: false,
            destroyable: false,
        }
    }

    async fn create(&self, spec: &SandboxSpec) -> SandboxResult<BackendSandboxRef> {
        let work_dir = spec.config.as_local().work_dir.clone();
        let sandbox = Arc::new(LocalSandbox::new(work_dir));
        let backend_id = sandbox.root_path()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        Ok(BackendSandboxRef { backend_id, sandbox })
    }

    async fn start(&self, _backend_id: &str) -> SandboxResult<()> {
        Ok(()) // Local sandbox is always ready
    }

    async fn destroy(&self, _backend_id: &str) -> SandboxResult<()> {
        Ok(()) // Cannot destroy host filesystem
    }
}
```

## Related Concepts
- [[sandbox-architecture]] — the four layers and which providers are wired
- [[sandbox-lifecycle]] — overall lifecycle management
- [[capability-discovery]] — runtime capability discovery
- [[vol-llm-sandbox-crate]] — implementation details
