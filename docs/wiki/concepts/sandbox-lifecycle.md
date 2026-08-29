---
type: concept
category: architecture
tags: [sandbox, lifecycle, manager, provider, store, state-machine, capability-discovery]
created: 2026-08-11
updated: 2026-08-28
source_count: 3
---

# Sandbox Lifecycle Management

## Overview
The sandbox system implements explicit instance lifecycle management with stable instance identity, state tracking, and provider-based backend abstraction. The architecture separates concerns into three layers: execution interface (`Sandbox`), backend adapters (`SandboxProvider`), and orchestration (`SandboxManager`).

## Architecture

```
Agent Session
  sandbox_id = sb_123
        │
        ▼
SandboxManager (orchestration)
        │
        ├── SandboxStore (metadata)
        │     sb_123 -> {provider: local, backend_id: ..., status: Running}
        │
        ▼
SandboxProvider (backend adapter)
        │
   ┌────┼──────────────┐
   ▼    ▼              ▼
Local  Tmp           SSH
Provider Provider    Provider
        │
        ▼
   Sandbox instances
```

## Lifecycle States

```
Created
   │ start
   ▼
Running ── pause ──► Paused
   ▲                   │
   └──── resume ───────┘

Running / Paused
   │ stop
   ▼
Stopped
   │ destroy
   ▼
Destroyed
```

**Valid transitions:**
- `Creating → Created → Starting → Running`
- `Running → Pausing → Paused → Starting → Running` (resume)
- `Running/Paused → Stopping → Stopped`
- `Stopped → Destroying → Destroyed`
- Any state → `Failed` (on error)

`SandboxManager` enforces these transitions and rejects invalid ones.

## Key Components

### SandboxId
ULID-based stable instance identifier (e.g., `sb_01JXYZ...`). Distinct from profile name. Used for tracking instances across operations.

### SandboxStatus
Explicit lifecycle states: `Creating`, `Created`, `Starting`, `Running`, `Pausing`, `Paused`, `Stopping`, `Stopped`, `Destroying`, `Destroyed`, `Failed`.

### SandboxCapabilities
Discoverable backend capabilities:
- `persistent` — survives process restart
- `pausable` — supports pause/resume
- `stoppable` — supports stop (preserves workspace)
- `destroyable` — supports destroy (removes resources)

Provider-specific defaults:
| Provider | Persistent | Pausable | Stoppable | Destroyable |
|----------|------------|----------|-----------|-------------|
| Local | ✓ | ✗ | ✗ | ✗ |
| Tmp | ✗ | ✗ | ✗ | ✓ |
| SSH | ✓ | ✗ | ✗ | ✗ |

### SandboxProvider Trait
Backend lifecycle adapter with methods:
- `kind()` / `capabilities()` — backend identification and capability discovery
- `create(spec)` / `get(backend_id)` / `list()` — instance management
- `start()` / `pause()` / `resume()` / `stop()` / `destroy()` — lifecycle operations

### SandboxStore Trait
Instance metadata persistence:
- `insert()` / `get()` / `list()` / `update_status()` / `delete()`
- `InMemorySandboxStore` — HashMap-based implementation
- Future: SQLite/Postgres implementations for persistence

### SandboxManager
Unified orchestration service:
- `load_profiles(dir)` — loads `.agents/sandboxes/*.toml`
- `register_profile(spec)` / `register_instance(spec, sandbox)` — registration
- `create(profile)` / `get(id)` / `list(filter)` — instance management
- `start()` / `stop()` / `destroy()` — lifecycle operations with state validation
- `default()` — returns single instance or creates fresh TmpSandbox

## Lifecycle Flow

### 1. Profile Definition
Sandbox profiles defined in `.agents/sandboxes/*.toml`:
```toml
name = "devbox"
provider = "ssh"
work_dir = "/workspace"
host = "10.0.0.10"
user = "nathan"
```

### 2. Profile Loading
`SandboxManager::load_profiles(dir)` reads TOML files and parses them as `SandboxSpec`.

### 3. Instance Creation
```rust
let id = manager.create("devbox").await?;
// SandboxManager:
//   1. Looks up spec by profile name
//   2. Routes to correct provider
//   3. Provider creates backend instance
//   4. Stores record in SandboxStore
//   5. Caches sandbox handle
//   6. Returns SandboxId
```

### 4. Instance Retrieval
```rust
let sandbox = manager.get(&id).await?;
// SandboxManager:
//   1. Looks up record from SandboxStore
//   2. Checks instance cache
//   3. Falls back to provider.get(backend_id)
//   4. Returns Arc<dyn Sandbox>
```

### 5. Lifecycle Operations
```rust
manager.stop(&id).await?;
// SandboxManager:
//   1. Validates state transition (Running → Stopped)
//   2. Delegates to provider.stop(backend_id)
//   3. Updates status in SandboxStore
```

### 6. Instance Destruction
```rust
manager.destroy(&id).await?;
// SandboxManager:
//   1. Delegates to provider.destroy(backend_id)
//   2. Removes from instance cache
//   3. Deletes record from SandboxStore
```

## Key Design Decisions

### Separation of Concerns
- **Sandbox trait** — execution and filesystem interface only
- **SandboxProvider** — backend-specific lifecycle management
- **SandboxManager** — orchestration and instance tracking
- **SandboxStore** — metadata persistence

### Stable Instance Identity
`SandboxId` is distinct from profile name. Multiple instances can be created from the same profile. Instances can be tracked across operations and sessions.

### Backend-Agnostic Orchestration
Agent/Tool code operates on `SandboxId` and `Sandbox` interface. Backend-specific lifecycle behavior is encapsulated in providers.

### Capability Discovery
`SandboxCapabilities` allows UI/runtime to expose correct lifecycle actions. Backends declare what they support; orchestration respects those limits.

### State Transition Validation
`SandboxManager` validates all state transitions. Invalid transitions return errors. This prevents undefined behavior.

### Default Sandbox Behavior
`manager.default()` returns the single existing instance if exactly one exists, otherwise creates a fresh TmpSandbox. This maintains backward compatibility.

## Configuration Format

Old format (deprecated):
```toml
name = "devbox"
type = "ssh"
work_dir = "/workspace"

[ssh]
host = "10.0.0.10"
user = "nathan"
identity_file = "/app/.ssh/id_ed25519"
```

New format:
```toml
name = "devbox"
provider = "ssh"
work_dir = "/workspace"
host = "10.0.0.10"
user = "nathan"
key_path = "/app/.ssh/id_ed25519"
```

Changes:
- `type` → `provider`
- SSH config flattened (no `[ssh]` section)
- `identity_file` → `key_path` (old spelling still parses as a serde alias)

Since 2026-08-28 `spec.rs` is the single schema source and `SandboxManager` the sole resolution path — the parallel `SandboxRegistry` loader was deleted after its stale schema caused [[schema-drift]]. `SandboxManager` gained profile-name lookup (`acquire_by_name` / `preload` / `build_inline` / `default_tmp`) alongside instance-ID lookup.

### Which providers have instance identity?

Recording an instance in the `SandboxStore` only makes sense when the instance carries state that the spec does not. `backend_id` is a pure function of the spec for `local` (the work dir path) and `ssh` (`user@host`) — build the same spec twice and you get two interchangeable objects, so there is no instance to track. `tmp` (a random `/tmp/<x>` path) and `firecracker` (a VM id) do carry per-instance state.

Consequence: `sandbox.list` is legitimately empty when every configured profile is local or ssh. "What sandboxes are configured?" is answered by `sandbox.list_specs`, which reads the spec map. Manufacturing store records for local/ssh just to populate `list` would be inventing identity that does not exist — and would have tripped the `default()` leak described in [[sandbox-default-idempotency]].

## Related
- [[vol-llm-sandbox-crate]] — implementation details
- [[provider-pattern]] — backend adapter pattern
- [[lifecycle-state-machine]] — state transition validation
- [[capability-discovery]] — runtime capability discovery
- [[tool-registry]] — tools use sandbox for all I/O
- [[vol-agent-server-crate]] — uses SandboxManager for orchestration
- [[schema-drift]] — the failure mode that motivated deleting the second loader
- [[sandbox-registry-manager-unification]] — the unification source page
- [[sandbox-default-idempotency]] — `default()` idempotency and per-provider instance identity
