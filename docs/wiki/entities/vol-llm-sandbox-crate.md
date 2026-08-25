---
type: entity
category: service
tags: [sandbox, container, ssh, firecracker, tmp, wasm, rust]
created: 2026-06-17
updated: 2026-08-25
source_count: 4
---

# vol-llm-sandbox Crate

## Overview
`vol-llm-sandbox` is the sandbox abstraction and lifecycle management crate. It defines the `Sandbox` trait and provides implementations for local execution, temp directories, SSH sandboxes, Firecracker VM sandboxes, and Wasm sandboxes. `SandboxRegistry` manages named sandbox instances loaded from TOML configuration files.

## Key Facts

### Sandbox trait
- `kind()` / `name()` / `root_path()` — identity and root path
- `bind_metadata(metadata)` — bind runtime metadata before `start()` (default no-op). TmpSandbox uses `sub_dir` key to set its root path
- `start()` / `cleanup()` — lifecycle
- `resolve_path(rel)` → absolute path within root. Rejects absolute paths and `~` — consistent across all implementations
- `execute(CommandRequest)` / `read_file` / `write_file` / `create_dir_all` / `read_dir` / `metadata`

### Implementations

| Type | Kind | Root | Use case |
|------|------|------|----------|
| `LocalSandbox` | `"local"` | `Some(path)` = fixed dir, `None` = random temp | Development, testing, DP nodes (at `/app`) |
| `TmpSandbox` | `"tmp"` | `/tmp/{random}/` → `bind_metadata("sub_dir")` overrides | Agent sandboxes, `registry.default()` fallback |
| `SSHSandbox` | `"ssh"` | Remote `work_dir` | Remote execution via SSH |
| `FirecrackerSandbox` | `"firecracker"` | VM rootfs | Full VM isolation (Linux/KVM) |
| `WasmSandbox` | `"wasm"` | Work dir preopened as `/` | Secure wasm module execution |

### TmpSandbox lifecycle
1. `TmpSandbox::new()` → random subdir: `/tmp/sandbox_{pid}_{count}/`
2. `bind_metadata({"sub_dir": "explore"})` → root = `/tmp/explore/`
3. `start()` → creates directory
4. `cleanup()` → removes directory

### SandboxRegistry
- `load(sandboxes_dir)` — loads `*.toml` configs. No built-in entries
- `register(name, sandbox)` — add programmatic sandbox
- `acquire(name)` — pure name lookup. Firecracker creates fresh instance
- `default()` — returns a fresh `TmpSandbox::new()` (random subdir)
- `names()` / `get()` / `len()` / `is_empty()`

### Path resolution contract
- All `resolve_path` implementations reject absolute paths (`/...`) and `~` paths
- `ToolContext::resolve_path` converts absolute paths within the sandbox root to relative before delegation
- This keeps tools working with absolute paths (e.g. from `tempfile`) while sandboxes see consistent relative input

### Runtime integration
- `AgentRuntimeBuilder::build()` loads registry, then `register("local", LocalSandbox(working_dir))`
- Agents use `sandbox = "local"` → `/app/` on DP nodes
- Agents without explicit sandbox → `registry.default()` → fresh TmpSandbox
- Agent loop calls `bind_metadata({"sub_dir": agent_id})` on every sandbox after acquire

## Modules
- `sandbox.rs` (trait in `lib.rs`) — `Sandbox` trait, `SandboxRef`, `SandboxError`, types
- `local.rs` — `LocalSandbox` implementation
- `tmp.rs` — `TmpSandbox` with random subdir + bind_metadata
- `ssh/` — SSH sandbox with session pooling (feature = "ssh")
- `firecracker.rs` — Firecracker VM sandbox with pool (feature = "firecracker")
- `wasm.rs` — WasmTime/WASI sandbox (feature = "wasm")
- `registry.rs` — `SandboxRegistry` with fault-tolerant TOML loading

## Timeline
- **2026-06-17**: Initial sandbox abstraction, LocalSandbox, SSHSandbox, FirecrackerSandbox
- **2026-08-11**: TmpSandbox added; resolve_path unified across all implementations; bind_metadata trait method; registry simplified to pure config loading; `Sandbox` trait fully documented
- **2026-08-19**: LocalSandbox timeout kill reworked — positive-pid kills only (`pkill -TERM/-KILL -P <pid>` for descendants + `kill <pid>` for the direct child, 2s grace poll instead of fixed 5s sleep). Process-group kills (`kill -TERM -pgid`) are forbidden: sandboxes (Claude Code bash sandbox verified) kill the caller's whole process tree when a group signal is delivered. `process_group(0)` on the child is retained but no longer depended on. Wasm exit-code test fixed: WAT modules must export `memory` or the wiggle shim bails with "missing required memory export" before calling any host function.
- **2026-08-25**: `SandboxHandler` in vol-agent-server refactored to accept `Arc<SandboxRegistry>` instead of `Arc<dyn Sandbox>`, enabling `sandbox.list` RPC to return all registered sandboxes (not just default). Frontend Sandboxes tab added to display all sandboxes.
