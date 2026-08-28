---
type: entity
category: service
tags: [sandbox, container, ssh, firecracker, tmp, wasm, rust, lifecycle, manager, provider]
created: 2026-06-17
updated: 2026-08-28
source_count: 6
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
Unified orchestration service — **the sole sandbox resolution path** since 2026-08-28 (`SandboxRegistry` deleted).

Instance-oriented (by `SandboxId`):
- `create(profile)` / `get(id)` / `list(filter)` — instance management
- `start()` / `stop()` / `destroy()` — lifecycle operations with state validation
- `register_instance(spec, sandbox)` — adopt a pre-existing sandbox

Profile-oriented (by name) — added 2026-08-28, absorbed from `SandboxRegistry`:
- `acquire_by_name(name)` — profile-name lookup; cache hit, else create via provider and cache
- `preload(dir)` — `load_profiles()` then eagerly instantiate every profile; per-profile failures warn+skip
- `build_inline(spec)` — one-off sandbox from a spec, not cached or tracked (cli-tool inline `[sandbox]`)
- `default_tmp()` — fresh `TmpSandbox`, falling back to `LocalSandbox`

Setup:
- `load_profiles(dir)` — loads `.agents/sandboxes/*.toml` as `SandboxSpec`
- `register_profile(spec)` — programmatic profile registration
- `register_provider(provider)` — backend registration

A `name_to_backend` index maps profile name → `backend_id`, maintained by `create()` / `acquire_by_name()` / `register_instance()` and pruned by `destroy()`.

`preload()` is safe at startup even for SSH profiles: `SshSession::new()` only stores config, connecting lazily on first use.

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

`spec.rs` is the **single schema source** for `.agents/sandboxes/*.toml` (since 2026-08-28 — the parallel `registry.rs` schema is gone).

```toml
name = "devbox"
provider = "ssh"
work_dir = "/workspace"
host = "10.0.0.10"
user = "nathan"
key_path = "/app/.ssh/id_ed25519"
host_key = "SHA256:..."          # or known_hosts_file
idle_timeout_secs = 300           # default 300
connect_timeout_secs = 10         # default 10
```

`SandboxProviderConfig` is a serde internally-tagged enum on `provider`. All five variants carry a shared `work_dir`, readable uniformly via `SandboxProviderConfig::work_dir()`.

| Variant | Layout |
|---|---|
| `local` | `work_dir` |
| `tmp` | `work_dir`, `sub_dir` |
| `ssh` | fields flattened into the top-level table |
| `firecracker` | nested `[firecracker]` table |
| `wasm` | nested `[wasm]` table + a `wasm.modules` array-of-tables |

Extraction helpers: `as_local()` / `as_tmp()` / `as_ssh()` / `as_firecracker()` / `as_wasm()`.

Changes from the pre-2026-08-27 format:
- `type` → `provider`
- SSH config flattened (no `[ssh]` subtable)
- `identity_file` → `key_path` (the old spelling still parses as a serde alias; `as_ssh()` resolves `key_path.or(identity_file)`)

`FirecrackerConfig` / `WasmConfig` / `WasmModuleConfig` live in `spec.rs` **without** feature gates — the type definitions are always available, so config parsing does not depend on which features are compiled.

### Path resolution contract
- All `resolve_path` implementations reject absolute paths (`/...`) and `~` paths
- `ToolContext::resolve_path` converts absolute paths within the sandbox root to relative before delegation
- This keeps tools working with absolute paths (e.g. from `tempfile`) while sandboxes see consistent relative input

### Runtime integration
- `AgentRuntimeBuilder::build()` creates `SandboxManager`, registers Local/Tmp/SSH providers, registers a `"local"` profile pointing at `working_dir`, then calls `preload()`
- `"local"` is an ordinary profile via `register_profile()`, not a special case — it resolves through the same path as disk-loaded profiles
- `DataPlaneServerCore` reuses `runtime.sandbox_manager` rather than constructing its own, so control-plane `sandbox.*` operations and data-plane tool execution observe the same instances
- ReAct tool loop resolves per call: `ToolConfig.get_sandbox(tool)` → `AgentDef.sandbox` → `"local"`, via `acquire_by_name().await` with `default_tmp().await` fallback
- Agents use `sandbox = "local"` → `/app/` on DP nodes
- `cli-tools-mcp` builds its own manager, registers providers, and calls `preload()`

## Modules
- `lib.rs` — Core types: `SandboxId`, `SandboxStatus`, `SandboxCapabilities`, `Sandbox` trait
- `manager.rs` — `SandboxManager` orchestration service
- `provider.rs` — `SandboxProvider` trait, `BackendSandboxRef`, `SandboxInfo`
- `store.rs` — `SandboxStore` trait, `InMemorySandboxStore`, `SandboxRecord`, `SandboxFilter`
- `spec.rs` — `SandboxSpec`, `SandboxProviderConfig`, `SshConfig`, `FirecrackerConfig`, `WasmConfig` — single schema source
- `local.rs` — `LocalSandbox` + `LocalSandboxProvider`
- `tmp.rs` — `TmpSandbox` + `TmpSandboxProvider`
- `ssh/` — `SSHSandbox` + `SSHSandboxProvider` (feature = "ssh")
- `firecracker.rs` — `FirecrackerSandbox` (feature = "firecracker")
- `wasm.rs` — `WasmSandbox` (feature = "wasm")

## Known gaps
- `InMemorySandboxStore` only — instance metadata does not survive restart
- No `SandboxProvider` implementation for `firecracker` or `wasm`; their spec variants parse, but `preload()` warns and skips those profiles for lack of a registered provider
- `SSHSandboxProvider::get(backend_id)` returns `NotFound` unconditionally — only the `instances` cache resolves SSH sandboxes after creation

## Timeline
- **2026-06-17**: Initial sandbox abstraction, LocalSandbox, SSHSandbox, FirecrackerSandbox
- **2026-08-11**: TmpSandbox added; resolve_path unified; bind_metadata trait method; registry simplified
- **2026-08-19**: LocalSandbox timeout kill reworked — positive-pid kills only
- **2026-08-25**: SandboxHandler refactored to accept `Arc<SandboxRegistry>` for listing all sandboxes
- **2026-08-27**: **Major refactor** — explicit lifecycle management with SandboxManager, SandboxProvider, SandboxStore, SandboxId, SandboxStatus, SandboxCapabilities. All implementations updated. 98%+ test coverage achieved.
- **2026-08-28**: **`SandboxRegistry` deleted** — `SandboxManager` is now the sole resolution path. `spec.rs` became the single schema source: SSH gained `host_key` / `known_hosts_file` / `passphrase` / timeout fields, `Firecracker` / `Wasm` variants added, all variants carry `work_dir`. `SSHSandboxProvider` dropped its `configs: Vec<SshConfig>` workaround and reads spec fields directly. Manager gained `acquire_by_name` / `preload` / `build_inline` / `default_tmp`. Fixes [[schema-drift]] that had `cli-tools-mcp` registering zero tools — see [[sandbox-registry-manager-unification]].
