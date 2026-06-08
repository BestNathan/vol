# Sandbox: Firecracker microVM + Wasmtime Integration Design

**Date:** 2026-06-07
**Status:** Draft

## Overview

Add two new sandbox backends to `vol-llm-sandbox`, both implementing the existing
`Sandbox` trait:

| Backend | Feature flag | Platform | Purpose |
|---------|-------------|----------|---------|
| `FirecrackerSandbox` | `firecracker` | Linux only (KVM) | Execute arbitrary code/scripts in lightweight microVMs |
| `WasmSandbox` | `wasm` | All | Execute Wasm modules (tool extensions + LLM-generated code) |

Both register via `SandboxRegistry` from `.agent/sandboxes/*.toml` config files,
same as existing `local` and `ssh` backends.

## Crate Structure

No new crates. Two new modules inside `crates/vol-llm-sandbox/`:

```
crates/vol-llm-sandbox/src/
├── lib.rs
├── local.rs              # existing
├── ssh.rs                # existing (feature = "ssh")
├── registry.rs           # existing — add "firecracker" / "wasm" branches
├── firecracker.rs        # NEW (feature = "firecracker")
│   ├── pool.rs           #   microVM pool manager
│   └── vm.rs             #   single microVM lifecycle (spawn/kill)
└── wasm.rs               # NEW (feature = "wasm")
```

Feature flags in `Cargo.toml`:

```toml
[features]
ssh = ["dep:ssh2"]
firecracker = []           # std-only; needs firecracker binary on PATH (Linux)
wasm = ["dep:wasmtime", "dep:wasmtime-wasi"]
```

## TOML Configuration

### Firecracker sandbox

```toml
# .agent/sandboxes/firecracker.toml
name = "fc"
type = "firecracker"
work_dir = "/tmp/fc-sandboxes"

[sandbox.firecracker]
kernel_image = "/opt/firecracker/kernel/vmlinux-5.10"
rootfs_image = "/opt/firecracker/rootfs/alpine-python.ext4"
rootfs_readonly = true
pool_size = 4
idle_timeout_secs = 300
connect_timeout_secs = 10
firecracker_binary = "/usr/bin/firecracker"   # optional, defaults to PATH lookup
guest_ssh_port = 22
```

Fields:

| Field | Required | Default | Description |
|-------|----------|---------|-------------|
| `kernel_image` | Yes | — | Path to guest kernel image (uncompressed ELF `vmlinux`) |
| `rootfs_image` | Yes | — | Path to ext4 rootfs image |
| `rootfs_readonly` | No | `false` | If true, mount rootfs read-only with tmpfs overlay for writes |
| `pool_size` | No | `1` | Number of pre-warmed idle microVMs |
| `idle_timeout_secs` | No | `300` | Idle VM reclamation timeout |
| `connect_timeout_secs` | No | `10` | SSH connection timeout |
| `firecracker_binary` | No | `"firecracker"` | Absolute path or PATH entry |
| `guest_ssh_port` | No | `22` | SSH port inside the microVM |

### Wasm sandbox

```toml
# .agent/sandboxes/wasm.toml
name = "wasm"
type = "wasm"
work_dir = "/tmp/wasm-sandboxes"

[sandbox.wasm]
max_memory_bytes = 134217728       # 128 MB
max_execution_ms = 30000           # 30 s

# Modules exposed as agent tools (Scenario A)
[[sandbox.wasm.modules]]
name = "python-linter"
path = "/opt/wasm-modules/ruff.wasm"
expose_as_tool = true

# Modules available for code execution (Scenario B)
[[sandbox.wasm.modules]]
name = "runner"
path = "/opt/wasm-modules/runner.wasm"
expose_as_tool = false
```

Fields:

| Field | Required | Default | Description |
|-------|----------|---------|-------------|
| `max_memory_bytes` | No | `134217728` (128 MB) | Max linear memory per module instance |
| `max_execution_ms` | No | `30000` (30 s) | Per-execution timeout |
| `modules[].name` | Yes | — | Logical name, used as `program` in `CommandRequest` |
| `modules[].path` | Yes | — | Path to the `.wasm` file on disk |
| `modules[].expose_as_tool` | No | `false` | Register the module as a named agent tool |

## FirecrackerSandbox Architecture

### microVM lifecycle (per FirecrackerVM)

```
spawn firecracker process
  → configure via REST API (kernel, rootfs, network)
  → InstanceStart
  → wait for guest SSH ready
  → ready for use

kill:
  → SIGTERM firecracker process (firecracker handles graceful shutdown)
  → remove API socket
```

The firecracker binary is spawned as a child process with `--api-sock <tmp_path>`.
All configuration is done via HTTP PUT to the Unix socket. The REST API path
prefix is `http://localhost`.

### Pool design

```
FirecrackerPool
├── idle: VecDeque<FirecrackerVM>     # pre-warmed, clean VMs
├── out_count: usize                  # currently borrowed
├── pool_size: usize                  # target idle count
└── idle_timeout: Duration            # shrink timer
```

**acquire():**
1. If `idle` is non-empty, pop and return immediately.
2. If idle is empty, spawn a new VM synchronously (block on guest ready).
3. Increment `out_count`.

**release(vm):**
1. Kill the used VM immediately (no state reuse).
2. Decrement `out_count`.
3. Spawn a background task to replenish the pool back to `pool_size`.

**Shrink (background task):**
- Runs periodically. If any idle VM has been sitting > `idle_timeout`, kill it.
- Idle count drops below `pool_size` → next `acquire` triggers replenish.

Rationale for kill-on-release: Firecracker snapshot/restore is heavier than
clean-boot (~125ms). Destroying and spawning fresh is simpler and prevents
state leakage between executions.

### Guest communication

`FirecrackerSandbox` composes an internal `SSHSandbox` instance to reuse the
existing SSH command-execution and SFTP file-I/O logic.

```
FirecrackerSandbox {
    vm: FirecrackerVM,
    ssh: crate::ssh::SSHSandbox,   // reuse SSH + SFTP
    root_path: PathBuf,            // work_dir inside microVM
}
```

All `Sandbox` trait methods delegate to `self.ssh.*` — no SSH logic duplication.

The microVM guest image must include `dropbear` (lightweight SSH server, ~200KB).
Host connects to guest via a tap device + static IP assigned during VM config.

### Platform guard

Non-Linux: the `"firecracker"` type is recognized but a warning is logged and
the sandbox is not registered. The `firecracker` module compiles on all platforms
(no `#[cfg(target_os)]` in module gating) but the pool returns errors at runtime
outside Linux.

```rust
// registry.rs
"firecracker" => {
    #[cfg(all(feature = "firecracker", target_os = "linux"))]
    { /* normal registration */ }
    #[cfg(not(target_os = "linux"))]
    { tracing::warn!("Firecracker sandbox requires Linux; skipping"); }
}
```

## WasmSandbox Architecture

### Engine

Uses `wasmtime` + `wasmtime-wasi` crates. A single `wasmtime::Engine` is shared
across all precompiled modules. Each execution creates a fresh `wasmtime::Store`
with its own `WasiCtx` — no state leaks between executions.

### File system mapping

WASI file I/O is mapped to the sandbox `work_dir`. The `WasiCtx` is configured
with a preopened directory that maps to the sandbox root — Wasm modules see it
as `/`. Path traversal protection is inherent (WASI sandbox cannot escape its
preopened dirs).

### execute() semantics

Since Wasm modules cannot spawn processes, `execute()` invokes a named export
function on the preloaded module:

- `req.program` → selects which module (by `modules[].name`)
- `req.args` → passed to the module's `main` function (or `_start` with args via WASI)
- `req.stdin` → WASI stdin stream
- `req.env` → WASI environment variables
- `req.timeout` → enforced via `wasmtime::Store::epoch_deadline`

The exit code is the return value of `main()`. stdout/stderr are captured from
the WASI preopened file descriptors.

### Tool extension path (Scenario A)

Modules with `expose_as_tool = true` are registered into the tool registry under
the name `modules[].name`. The tool definition includes the module's exported
function signature as the tool's input schema (if the module provides one).

### Code execution path (Scenario B)

Modules with `expose_as_tool = false` are available for ad-hoc execution via
`Sandbox::execute()`. The coding agent's tool layer can target these modules
directly by setting `program` to the module name.

The pipeline for LLM-generated code:
```
LLM generates Rust code
  → rustc --target wasm32-wasi → .wasm
  → write .wasm to sandbox work_dir
  → Sandbox::execute(program = "runner", args = ["my_module.wasm"])
```

## Implementation Scope

### Phase 1: FirecrackerSandbox
- `firecracker.rs` — FirecrackerVM (spawn/kill, REST API config)
- `pool.rs` — FirecrackerPool (acquire/release/replenish/shrink)
- `FirecrackerSandbox` — impl `Sandbox` trait via delegation to SSHSandbox
- `registry.rs` — register "firecracker" type
- Tests: pool unit tests, integration test with `#[ignore]` (needs KVM)

### Phase 2: WasmSandbox
- `wasm.rs` — WasmSandbox (Engine, Module cache, WasiCtx, execute/read/write)
- `registry.rs` — register "wasm" type
- Tests: execute/read/write unit tests, tool extension registration test

## Non-Goals

- Production rootfs images — users supply their own. Documentation will cover
  how to build Alpine-based rootfs with dropbear.
- Network access from microVMs — initially no NAT/outbound networking. The
  sandbox is an isolated execution environment.
- Firecracker snapshot/restore — kill-on-release is sufficient for the MVP.
- Multiple Wasm engines — wasmtime only. Wasmer may be considered later if
  specific language support (e.g., Python compiled to Wasm) is needed.
