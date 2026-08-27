---
type: entity
category: service
tags: [sandbox, container, ssh, firecracker, tmp, wasm, rust, lifecycle, manager, provider]
created: 2026-06-17
updated: 2026-08-27
source_count: 5
---

# vol-llm-sandbox Crate

## Overview
`vol-llm-sandbox` is the sandbox abstraction and lifecycle management crate. It provides a complete lifecycle management system with explicit instance identity, state tracking, and provider-based backend abstraction. The crate defines the `Sandbox` trait for execution, `SandboxProvider` trait for backend adapters, `SandboxStore` trait for instance metadata, and `SandboxManager` for unified orchestration.

## Key Facts

### Architecture (2026-08-27 refactor)

The crate now implements a three-layer architecture:

1. **Sandbox trait** — execution and filesystem interface
2. **SandboxProvider trait** — backend lifecycle adapter
3. **SandboxManager** — orchestration and instance management

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
```

### Core Types

- **SandboxId** — ULID-based stable instance identifier (e.g., "sb_01JXYZ...")
- **SandboxStatus** — lifecycle states: Creating, Created, Starting, Running, Pausing, Paused, Stopping, Stopped, Destroying, Destroyed, Failed
- **SandboxCapabilities** — discoverable backend capabilities: persistent, pausable, stoppable, destroyable
- **SandboxSpec** — profile specification with provider-specific config
- **SandboxRecord** — instance metadata stored in SandboxStore

### Sandbox trait (updated 2026-08-27)
- `id()` — stable instance identifier
- `kind()` / `status()` / `root_path()` — identity, status, and root path
- `resolve_path(rel)` → absolute path within root. Rejects absolute paths and `~`
- `execute(CommandRequest)` / `read_file` / `write_file` / `create_dir_all` / `read_dir` / `metadata`
- Removed: `name()`, `start()`, `cleanup()`, `bind_metadata()` (moved to provider/manager)

### SandboxProvider trait
Backend lifecycle adapter with methods:
- `kind()` / `capabilities()` — backend identification and capability discovery
- `create(spec)` / `get(backend_id)` / `list()` — instance management
- `start()` / `pause()` / `resume()` / `stop()` / `destroy()` — lifecycle operations

### SandboxStore trait
Instance metadata persistence:
- `insert()` / `get()` / `list()` / `update_status()` / `delete()`
- `InMemorySandboxStore` — HashMap-based implementation
- Future: SQLite/Postgres implementations

### SandboxManager
Unified orchestration service:
- `load_profiles(dir)` — loads `.agents/sandboxes/*.toml`
- `register_profile(spec)` / `register_instance(spec, sandbox)` — registration
- `create(profile)` / `get(id)` / `list(filter)` — instance management
- `start()` / `stop()` / `destroy()` — lifecycle operations with state validation
- `default()` — returns single instance or creates fresh TmpSandbox
- `register_provider(provider)` — backend registration

### Implementations

| Type | Kind | Provider | Root | Use case |
|------|------|----------|------|----------|
| `LocalSandbox` | `"local"` | `LocalSandboxProvider` | `Some(path)` = fixed dir, `None` = random temp | Development, testing, DP nodes |
| `TmpSandbox` | `"tmp"` | `TmpSandboxProvider` | `/tmp/{sub_dir}/` | Agent sandboxes, default fallback |
| `SSHSandbox` | `"ssh"` | `SSHSandboxProvider` | Remote `work_dir` | Remote execution via SSH |
| `FirecrackerSandbox` | `"firecracker"` | (updated) | VM rootfs | Full VM isolation (Linux/KVM) |
| `WasmSandbox` | `"wasm"` | (updated) | Work dir preopened as `/` | Secure wasm module execution |

### Provider Capabilities

| Provider | Persistent | Pausable | Stoppable | Destroyable |
|----------|------------|----------|-----------|-------------|
| Local | ✓ | ✗ | ✗ | ✗ |
| Tmp | ✗ | ✗ | ✗ | ✓ |
| SSH | ✓ | ✗ | ✗ | ✗ |

### Configuration Format

Sandbox profiles in `.agents/sandboxes/*.toml`:

```toml
name = "devbox"
provider = "ssh"
work_dir = "/workspace"
host = "10.0.0.10"
user = "nathan"
key_path = "/app/.ssh/id_ed25519"
```

Changes from old format:
- `type` → `provider`
- SSH config flattened (no `[ssh]` section)
- `identity_file` → `key_path`

### Path resolution contract
- All `resolve_path` implementations reject absolute paths (`/...`) and `~` paths
- `ToolContext::resolve_path` converts absolute paths within the sandbox root to relative before delegation
- This keeps tools working with absolute paths (e.g. from `tempfile`) while sandboxes see consistent relative input

### Runtime integration
- `AgentRuntimeBuilder::build()` creates `SandboxManager`, registers providers, loads profiles
- `SandboxManager::default()` returns single instance or creates fresh TmpSandbox
- Agents use `sandbox = "local"` → `/app/` on DP nodes
- Instance lifecycle managed by `SandboxManager` with state validation

## Modules
- `lib.rs` — Core types: `SandboxId`, `SandboxStatus`, `SandboxCapabilities`, `Sandbox` trait
- `manager.rs` — `SandboxManager` orchestration service
- `provider.rs` — `SandboxProvider` trait, `BackendSandboxRef`, `SandboxInfo`
- `store.rs` — `SandboxStore` trait, `InMemorySandboxStore`, `SandboxRecord`, `SandboxFilter`
- `spec.rs` — `SandboxSpec`, `SandboxProviderConfig` enum
- `local.rs` — `LocalSandbox` + `LocalSandboxProvider`
- `tmp.rs` — `TmpSandbox` + `TmpSandboxProvider`
- `ssh/` — `SSHSandbox` + `SSHSandboxProvider` (feature = "ssh")
- `firecracker.rs` — `FirecrackerSandbox` (feature = "firecracker")
- `wasm.rs` — `WasmSandbox` (feature = "wasm")
- `registry.rs` — Legacy `SandboxRegistry` (deprecated, kept for compatibility)

## Timeline
- **2026-06-17**: Initial sandbox abstraction, LocalSandbox, SSHSandbox, FirecrackerSandbox
- **2026-08-11**: TmpSandbox added; resolve_path unified; bind_metadata trait method; registry simplified
- **2026-08-19**: LocalSandbox timeout kill reworked — positive-pid kills only
- **2026-08-25**: SandboxHandler refactored to accept `Arc<SandboxRegistry>` for listing all sandboxes
- **2026-08-27**: **Major refactor** — explicit lifecycle management with SandboxManager, SandboxProvider, SandboxStore, SandboxId, SandboxStatus, SandboxCapabilities. All implementations updated. 98%+ test coverage achieved.
