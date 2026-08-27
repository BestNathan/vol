---
type: source
source_type: design
date: 2026-08-26
ingested: 2026-08-27
tags: [sandbox, lifecycle, manager, provider, store, refactor, issue-59]
---

# Sandbox Lifecycle Management Design

**Authors/Creators:** Vol team
**Date:** 2026-08-26
**Link:** docs/superpowers/specs/2026-08-26-sandbox-lifecycle-design.md
**Issue:** #59

## TL;DR

Comprehensive refactor of the sandbox abstraction to introduce explicit instance lifecycle management. Separates concerns into distinct abstractions: `SandboxId` for stable instance identity, `SandboxStatus` for lifecycle states, `SandboxCapabilities` for backend capability discovery, `SandboxProvider` for backend adapters, `SandboxStore` for instance metadata persistence, and `SandboxManager` for unified orchestration (replacing `SandboxRegistry`).

## Key Takeaways

- **Separation of concerns**: Profile config vs instance management vs execution interface
- **Stable instance identity**: `SandboxId` (ULID-based) distinct from profile name
- **Explicit lifecycle states**: Creating → Running → Paused/Stopped → Destroyed with validated transitions
- **Backend-agnostic**: Agent/Tool code doesn't depend on specific backend implementations
- **Provider pattern**: Each backend (Local/Tmp/SSH/Firecracker/Wasm) implements `SandboxProvider`
- **Capability discovery**: `SandboxCapabilities` exposes what operations each backend supports
- **Unified orchestration**: `SandboxManager` replaces `SandboxRegistry` with profile loading, instance creation, lifecycle operations
- **In-memory storage**: `SandboxStore` trait with `InMemorySandboxStore` implementation (persistent storage deferred)

## Detailed Summary

### Problem Statement

The original `Sandbox` trait conflated multiple concerns:
- Profile configuration (what kind of sandbox)
- Instance identity (which sandbox instance)
- Execution handle (how to use the sandbox)
- Lifecycle management (start/stop/cleanup)

This made it impossible to:
- Track sandbox instances across multiple tool calls
- Support session-scoped sandbox binding (issue #58)
- Implement proper lifecycle management (pause/resume/destroy)
- Add new backends without modifying core abstraction

### Architecture

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
   │    │              │
   ▼    ▼              ▼
Local  Tmp           SSH
Sandbox Sandbox      Sandbox
```

### Core Components

#### SandboxId
- ULID-based stable instance identifier (e.g., "sb_01JXYZ...")
- Distinct from profile name
- Used for tracking instances across operations

#### SandboxStatus
- Explicit lifecycle states: Creating, Created, Starting, Running, Pausing, Paused, Stopping, Stopped, Destroying, Destroyed, Failed
- Validated state transitions enforced by `SandboxManager`
- Supports pause/resume where backend allows

#### SandboxCapabilities
- Discoverable backend capabilities: persistent, pausable, stoppable, destroyable
- Allows UI/runtime to expose correct lifecycle actions
- Provider-specific defaults (e.g., Local is persistent but not pausable)

#### SandboxProvider Trait
- Backend lifecycle adapter
- Methods: create, get, list, start, pause, resume, stop, destroy
- Each backend implements this trait
- Returns `BackendSandboxRef` containing backend_id and sandbox handle

#### SandboxStore Trait
- Instance metadata persistence
- Methods: insert, get, list, update_status, delete
- `InMemorySandboxStore` implementation (HashMap-based)
- Future: SQLite/Postgres implementations for persistence

#### SandboxManager
- Unified orchestration service (replaces `SandboxRegistry`)
- Responsibilities:
  - Profile loading from `.agents/sandboxes/*.toml`
  - Instance creation via provider routing
  - Lifecycle operations with state validation
  - Default sandbox behavior (single instance or fresh TmpSandbox)
  - `register_instance()` for backward compatibility

#### SandboxSpec
- Replaces old TOML config + bind_metadata approach
- Contains profile name, provider kind, provider-specific config, metadata
- Provider config uses serde flatten with tagged enum

### Migration

**Breaking changes:**
- `Sandbox` trait: removed `name()`, `start()`, `cleanup()`, `bind_metadata()`; added `id()`, `status()`; changed `root_path()` to return `Option<&Path>`
- All sandbox implementations updated (Local, Tmp, SSH, Remote, Firecracker, Wasm)
- Callers migrated: `vol-agent-server`, `vol-llm-tool`, `vol-llm-agent`, `vol-llm-tools-builtin`

**Backward compatibility:**
- `SandboxManager::register_instance()` allows registering pre-built sandboxes
- `SandboxManager::default()` maintains same behavior (returns single instance or creates TmpSandbox)

### Configuration Format

Old format:
```toml
name = "devbox"
type = "ssh"
work_dir = "/workspace"

[ssh]
host = "10.0.0.10"
user = "nathan"
```

New format:
```toml
name = "devbox"
provider = "ssh"
work_dir = "/workspace"
host = "10.0.0.10"
user = "nathan"
```

Changes:
- `type` → `provider`
- SSH config flattened (no `[ssh]` section)
- `identity_file` → `key_path`
- Removed unsupported fields (idle_timeout_secs, connect_timeout_secs)

### Test Coverage

- **manager.rs**: 98.19% (was 46.38%)
- **store.rs**: 100% (was 73.68%)
- **spec.rs**: 100%
- Total: 35 new tests covering lifecycle operations, state transitions, error handling, provider routing

## Entities Mentioned

- [[vol-llm-sandbox-crate]]: Core sandbox abstraction crate, now includes SandboxManager, SandboxProvider, SandboxStore
- [[vol-agent-server-crate]]: Updated to use SandboxManager instead of SandboxRegistry
- [[vol-llm-agent-crate]]: Updated sandbox acquisition to use new API
- [[vol-llm-tool-crate]]: Updated to handle Option<&Path> from root_path()

## Concepts Covered

- [[sandbox-lifecycle]]: Updated with new lifecycle management architecture
- [[provider-pattern]]: Backend adapter pattern for sandbox implementations
- [[lifecycle-state-machine]]: Explicit state transitions with validation
- [[capability-discovery]]: Runtime discovery of backend capabilities

## Notes

- Firecracker and Wasm providers updated but not fully tested (deferred)
- Persistent `SandboxStore` implementations (SQLite/Postgres) not yet implemented
- Frontend UI changes not included (separate task)
- Docker/Kubernetes provider implementations deferred to future work
- State transition validation is strict but may need relaxation for edge cases

## Implementation Commits

1. `e69a5298` - Core types (SandboxId, SandboxStatus, SandboxCapabilities)
2. `446d648a` - SandboxStore trait + InMemorySandboxStore
3. `2ff45455` - SandboxProvider trait + SandboxSpec
4. `167f2a00` - Sandbox trait refactor
5. `db29b3e7` - SandboxManager orchestration layer
6. `c8dc0506` - Local/Temp/SSH Provider implementations
7. `9c7b0412` - Test coverage improvements (90%+)
8. `11a28f09` - Sandbox config format updates
9. `4de98896` - load_profiles integration
10. `a50426f1` - load_profiles tests
11. `2f4ea59e` - Quality workflow fixes
