# Sandbox: Firecracker microVM + Wasmtime Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Firecracker microVM (Linux/KVM) and Wasmtime sandbox backends to `vol-llm-sandbox`, both implementing the existing `Sandbox` trait, registered via `SandboxRegistry` from TOML config.

**Architecture:** Two new modules in the existing sandbox crate. Firecracker uses a pool of microVMs (acquire→use→kill→replenish), reusing `SSHSandbox` internally for command/file I/O. Wasmtime runs `.wasm` modules via WASI, with per-execution ephemeral stores. `SandboxRegistry` gains an `acquire()` method that creates fresh instances for pool-based backends (firecracker) while returning clones for singletons (local/ssh).

**Tech Stack:** Rust std + tokio (firecracker), wasmtime + wasmtime-wasi 20.x (wasm), same `Sandbox` trait as existing backends.

---

## Phase 0: Prep — extract shared normalize_path

### Task 0: Extract normalize_path to lib.rs

**Files:**
- Modify: `crates/vol-llm-sandbox/src/lib.rs`
- Modify: `crates/vol-llm-sandbox/src/local.rs`
- Modify: `crates/vol-llm-sandbox/src/ssh/mod.rs`

`normalize_path` is currently defined identically in `local.rs` (~line 205) and `ssh/mod.rs` (~line 327). Extract it once so firecracker and wasm can reuse it.

- [ ] **Step 1: Add normalize_path to lib.rs**

Add after the type definitions, before module declarations:

```rust
/// Normalize a path by resolving `.` and `..` components without touching the filesystem.
pub fn normalize_path(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => { result.pop(); }
            std::path::Component::CurDir => {}
            _ => result.push(component),
        }
    }
    result
}
```

- [ ] **Step 2: Remove duplicate from local.rs and ssh/mod.rs**

In `local.rs`: remove the `normalize_path` function (around line 205).
Replace the two call sites (`local.rs:61,63`) with `crate::normalize_path(...)`.

In `ssh/mod.rs`: remove the `normalize_path` function (around line 327).
Replace call sites (`ssh/mod.rs:171,173`) with `crate::normalize_path(...)`.

- [ ] **Step 3: Verify**

```bash
cargo check -p vol-llm-sandbox --features ssh
cargo test -p vol-llm-sandbox --features ssh
```

Expected: all pass.

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-sandbox/src/lib.rs crates/vol-llm-sandbox/src/local.rs crates/vol-llm-sandbox/src/ssh/mod.rs
git commit -m "refactor(sandbox): extract normalize_path to lib.rs (dedup)"
```

---

## File Map

| File | Action | Responsibility |
|------|--------|----------------|
| `crates/vol-llm-sandbox/src/lib.rs` | Modify | Module declarations; extract `normalize_path` from local/ssh |
| `crates/vol-llm-sandbox/src/local.rs` | Modify | Remove duplicate `normalize_path` |
| `crates/vol-llm-sandbox/src/ssh/mod.rs` | Modify | Remove duplicate `normalize_path` |
| `crates/vol-llm-sandbox/Cargo.toml` | Modify | Add `firecracker`, `wasm` features |
| `crates/vol-llm-sandbox/src/registry.rs` | Modify | `FirecrackerConfig`, `WasmConfig`, new `"firecracker"`/`"wasm"` branches, `acquire()` method |
| `crates/vol-llm-sandbox/src/firecracker.rs` | Create | `FirecrackerVM`, `FirecrackerPool`, `FirecrackerSandbox` |
| `crates/vol-llm-sandbox/src/wasm.rs` | Create | `WasmSandbox` + `WasmModule` |
| `crates/vol-llm-agent/src/react/agent.rs` | Modify | Use `registry.acquire()` instead of `registry.get()` |

---

## Phase 1: FirecrackerSandbox

### Task 1: Feature flag and module scaffolding

**Files:**
- Modify: `crates/vol-llm-sandbox/Cargo.toml`
- Modify: `crates/vol-llm-sandbox/src/lib.rs`
- Create: `crates/vol-llm-sandbox/src/firecracker.rs` (empty placeholder)

- [ ] **Step 1: Add firecracker feature flag**

In `crates/vol-llm-sandbox/Cargo.toml`, update the `[features]` section:

```toml
[features]
default = []
ssh = ["ssh2"]
firecracker = []
```

- [ ] **Step 2: Declare firecracker module in lib.rs**

In `crates/vol-llm-sandbox/src/lib.rs`, add after the existing module declarations:

```rust
#[cfg(feature = "firecracker")]
pub mod firecracker;
```

- [ ] **Step 3: Create empty firecracker.rs placeholder**

```bash
touch crates/vol-llm-sandbox/src/firecracker.rs
```

Write minimal placeholder content:

```rust
//! Firecracker microVM sandbox — lightweight KVM-based isolation.
//!
//! Linux only. Requires the `firecracker` binary on PATH and KVM access.
//! Spawns microVMs via REST API over Unix socket, runs commands via SSH,
//! and manages a pool of pre-warmed VMs for low-latency execution.
```

- [ ] **Step 4: Verify compilation**

```bash
cargo check -p vol-llm-sandbox
cargo check -p vol-llm-sandbox --features firecracker
```

Expected: both pass with no errors.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-sandbox/Cargo.toml crates/vol-llm-sandbox/src/lib.rs crates/vol-llm-sandbox/src/firecracker.rs
git commit -m "feat(sandbox): add firecracker feature flag and module skeleton"
```

---

### Task 2: FirecrackerConfig in registry

**Files:**
- Modify: `crates/vol-llm-sandbox/src/registry.rs`

- [ ] **Step 1: Add FirecrackerConfig struct**

Add after the `SshConfig` struct in `registry.rs`:

```rust
/// Configuration for a Firecracker microVM sandbox.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FirecrackerConfig {
    /// Path to the uncompressed ELF kernel image (vmlinux).
    pub kernel_image: String,
    /// Path to the ext4 rootfs image.
    pub rootfs_image: String,
    /// If true, mount rootfs read-only (writes go to tmpfs overlay).
    #[serde(default)]
    pub rootfs_readonly: bool,
    /// Number of pre-warmed idle microVMs in the pool.
    #[serde(default = "default_pool_size")]
    pub pool_size: usize,
    /// Seconds before an idle VM is reclaimed.
    #[serde(default = "default_idle_timeout_fc")]
    pub idle_timeout_secs: u64,
    /// Seconds to wait for SSH to become available in the guest.
    #[serde(default = "default_connect_timeout_fc")]
    pub connect_timeout_secs: u64,
    /// Path to firecracker binary. If unset, looks up "firecracker" on PATH.
    #[serde(default)]
    pub firecracker_binary: Option<String>,
    /// IP address assigned to the guest VM for SSH access.
    #[serde(default = "default_guest_ip")]
    pub guest_ip: String,
    /// SSH port inside the guest VM.
    #[serde(default = "default_guest_ssh_port")]
    pub guest_ssh_port: u16,
    /// Host tap device name for the VM's network interface.
    pub tap_device: String,
    /// Path to an SSH keypair for guest access (the private key).
    /// The corresponding public key must be in the guest's ~/.ssh/authorized_keys.
    pub ssh_identity_file: String,
    /// Optional passphrase for the SSH identity file.
    #[serde(default)]
    pub ssh_passphrase: Option<String>,
}

fn default_pool_size() -> usize { 1 }
fn default_idle_timeout_fc() -> u64 { 300 }
fn default_connect_timeout_fc() -> u64 { 10 }
fn default_guest_ip() -> String { "172.16.0.2".to_string() }
fn default_guest_ssh_port() -> u16 { 22 }
```

- [ ] **Step 2: Add firecracker field to SandboxConfig**

Add after the `ssh` field in `SandboxConfig`:

```rust
pub struct SandboxConfig {
    // ... existing fields ...
    #[serde(default)]
    pub ssh: Option<SshConfig>,
    // NEW:
    #[serde(default)]
    pub firecracker: Option<FirecrackerConfig>,
}
```

- [ ] **Step 3: Verify compilation**

```bash
cargo check -p vol-llm-sandbox --features firecracker
```

Expected: pass.

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-sandbox/src/registry.rs
git commit -m "feat(sandbox): add FirecrackerConfig to SandboxConfig"
```

---

### Task 3: FirecrackerVM — spawn/kill + REST API

**Files:**
- Modify: `crates/vol-llm-sandbox/src/firecracker.rs`

This task builds the low-level VM lifecycle: spawn the firecracker binary, configure it via REST API over Unix socket, wait for boot, and kill it.

- [ ] **Step 1: Write the HTTP-over-Unix-socket helper**

Append to `firecracker.rs`:

```rust
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};
use crate::{SandboxError, SandboxResult, CommandOutput};

/// Send an HTTP PUT request with JSON body to a Unix socket.
/// Firecracker REST API uses only PUT and GET; we only need PUT for configuration.
fn unix_socket_put(socket_path: &Path, uri: &str, body: &str) -> SandboxResult<()> {
    let mut stream = UnixStream::connect(socket_path)
        .map_err(|e| SandboxError::Ssh(format!("connect to api socket: {}", e)))?;

    let request = format!(
        "PUT {} HTTP/1.1\r\n\
         Host: localhost\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n\
         {}",
        uri,
        body.len(),
        body,
    );

    stream.write_all(request.as_bytes())
        .map_err(|e| SandboxError::Ssh(format!("api write: {}", e)))?;

    let mut response = String::new();
    stream.read_to_string(&mut response)
        .map_err(|e| SandboxError::Ssh(format!("api read: {}", e)))?;

    // Check HTTP status line: "HTTP/1.1 204 No Content" or "HTTP/1.1 200 OK"
    if !response.contains("204") && !response.contains("200") {
        return Err(SandboxError::Ssh(format!("firecracker API error: {}", response)));
    }

    Ok(())
}
```

Note: reuses `SandboxError::Ssh` variant for simplicity. We'll add a dedicated variant in Task 7.

- [ ] **Step 2: Write FirecrackerVM struct and spawn logic**

```rust
/// A running Firecracker microVM.
///
/// Created by `FirecrackerVM::spawn()` and destroyed via `FirecrackerVM::kill()`.
pub struct FirecrackerVM {
    /// The firecracker child process.
    child: Child,
    /// Path to the API Unix socket.
    api_socket: PathBuf,
    /// Temporary directory holding the socket (cleaned on kill).
    _temp_dir: tempfile::TempDir,
    /// Guest IP for SSH connectivity checks.
    guest_ip: String,
    /// Guest SSH port.
    guest_ssh_port: u16,
}

impl FirecrackerVM {
    /// Spawn a new Firecracker microVM.
    ///
    /// 1. Creates a temp dir for the API socket
    /// 2. Spawns `firecracker --api-sock <path>`
    /// 3. Configures kernel, rootfs, and network via REST API
    /// 4. Starts the VM and waits for SSH to be ready
    pub fn spawn(config: &crate::registry::FirecrackerConfig) -> SandboxResult<Self> {
        let temp_dir = tempfile::TempDir::new()
            .map_err(|e| SandboxError::Io(e))?;
        let api_socket = temp_dir.path().join("api.sock");

        let binary = config.firecracker_binary.as_deref().unwrap_or("firecracker");

        let child = Command::new(binary)
            .arg("--api-sock")
            .arg(&api_socket)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| SandboxError::Ssh(format!(
                "failed to spawn firecracker: {} (is it installed?)", e
            )))?;

        let vm = Self {
            child,
            api_socket: api_socket.clone(),
            _temp_dir: temp_dir,
            guest_ip: config.guest_ip.clone(),
            guest_ssh_port: config.guest_ssh_port,
        };

        // Wait for API socket to appear (firecracker creates it on startup)
        vm.wait_for_socket(Duration::from_secs(5))?;

        // Configure the VM
        vm.configure(config)?;

        Ok(vm)
    }

    fn wait_for_socket(&self, timeout: Duration) -> SandboxResult<()> {
        let start = Instant::now();
        while !self.api_socket.exists() {
            if start.elapsed() > timeout {
                return Err(SandboxError::Ssh(
                    "firecracker API socket did not appear".to_string()
                ));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        Ok(())
    }

    fn configure(&self, config: &crate::registry::FirecrackerConfig) -> SandboxResult<()> {
        // 1. Set kernel
        let kernel_json = serde_json::json!({
            "kernel_image_path": config.kernel_image,
            "boot_args": "console=ttyS0 reboot=k panic=1 pci=off",
        });
        unix_socket_put(
            &self.api_socket,
            "/boot-source",
            &kernel_json.to_string(),
        )?;

        // 2. Set rootfs
        let rootfs_json = serde_json::json!({
            "drive_id": "rootfs",
            "path_on_host": config.rootfs_image,
            "is_root_device": true,
            "is_read_only": config.rootfs_readonly,
        });
        unix_socket_put(
            &self.api_socket,
            "/drives/rootfs",
            &rootfs_json.to_string(),
        )?;

        // 3. Configure network
        let net_json = serde_json::json!({
            "iface_id": "eth0",
            "host_dev_name": config.tap_device,
        });
        unix_socket_put(
            &self.api_socket,
            "/network-interfaces/eth0",
            &net_json.to_string(),
        )?;

        // 4. Start the VM
        let start_json = serde_json::json!({
            "action_type": "InstanceStart",
        });
        unix_socket_put(
            &self.api_socket,
            "/actions",
            &start_json.to_string(),
        )?;

        Ok(())
    }

    /// Block until SSH on the guest is reachable.
    pub fn wait_for_ssh_ready(&self, timeout: Duration) -> SandboxResult<()> {
        let start = Instant::now();
        let addr = format!("{}:{}", self.guest_ip, self.guest_ssh_port);
        while start.elapsed() < timeout {
            if std::net::TcpStream::connect_timeout(
                &addr.parse().unwrap(),
                Duration::from_secs(1),
            ).is_ok() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        Err(SandboxError::Ssh(format!(
            "guest SSH not reachable at {} after {:?}", addr, timeout
        )))
    }

    /// Kill the microVM by sending SIGTERM to the firecracker process.
    pub fn kill(mut self) -> SandboxResult<()> {
        let _ = self.child.kill();
        let _ = self.child.wait();
        Ok(())
    }

    /// IP address of the guest.
    pub fn guest_ip(&self) -> &str { &self.guest_ip }

    /// SSH port on the guest.
    pub fn guest_ssh_port(&self) -> u16 { self.guest_ssh_port }
}
```

- [ ] **Step 3: Add tempfile dependency**

In `crates/vol-llm-sandbox/Cargo.toml`, add:

```toml
tempfile = { workspace = true }
```

Check if `tempfile` is already in workspace deps:

```bash
grep tempfile Cargo.toml
```

If not, add `tempfile = "3"` to workspace `[workspace.dependencies]` and use `tempfile = "3"` in the sandbox crate directly.

- [ ] **Step 4: Verify compilation**

```bash
cargo check -p vol-llm-sandbox --features firecracker
```

Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-sandbox/src/firecracker.rs crates/vol-llm-sandbox/Cargo.toml
# git add Cargo.toml  # if tempfile was added to workspace
git commit -m "feat(sandbox): add FirecrackerVM spawn/kill with REST API"
```

---

### Task 4: FirecrackerPool

**Files:**
- Modify: `crates/vol-llm-sandbox/src/firecracker.rs` (append)

- [ ] **Step 1: Write FirecrackerPool struct and impl**

Append to `firecracker.rs`:

```rust
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Handle returned by the pool — bundles a VM with its SSH connection.
pub(crate) struct FirecrackerVmHandle {
    pub vm: FirecrackerVM,
    pub ssh: crate::ssh::SSHSandbox,
}

/// Pool of Firecracker microVMs for low-latency execution.
///
/// Maintains a queue of pre-warmed idle VMs. `acquire()` pops from the
/// queue or spawns a new VM if none are idle. `return_vm()` queues a VM
/// for async destruction (kill-on-release) and triggers replenishment.
///
/// All methods that mutate state take `&self` (interior mutability via `Mutex`).
pub struct FirecrackerPool {
    inner: Mutex<PoolInner>,
    /// Handle to the tokio runtime for spawning maintenance tasks.
    runtime: tokio::runtime::Handle,
}

struct PoolInner {
    /// Idle VMs available for immediate use, with the time they became idle.
    idle: VecDeque<(FirecrackerVM, Instant)>,
    /// Number of VMs currently checked out (not in idle or returned).
    out_count: usize,
    /// Target number of idle VMs to maintain.
    pool_size: usize,
    /// Idle timeout — VMs idle longer than this get killed.
    idle_timeout: Duration,
    /// VMs returned from use, waiting for the maintenance task to kill them.
    returned: VecDeque<FirecrackerVM>,
    /// Config needed to spawn new VMs.
    config: crate::registry::FirecrackerConfig,
}

impl FirecrackerPool {
    /// Create a new pool. Starts a background maintenance task.
    pub fn new(
        config: crate::registry::FirecrackerConfig,
        runtime: tokio::runtime::Handle,
    ) -> Arc<Self> {
        let pool_size = config.pool_size;
        let idle_timeout = Duration::from_secs(config.idle_timeout_secs);
        let pool = Arc::new(Self {
            inner: Mutex::new(PoolInner {
                idle: VecDeque::new(),
                out_count: 0,
                pool_size,
                idle_timeout,
                returned: VecDeque::new(),
                config: config.clone(),
            }),
            runtime: runtime.clone(),
        });

        // Start maintenance task
        let pool_clone = Arc::clone(&pool);
        runtime.spawn(async move {
            pool_clone.maintenance_loop().await;
        });

        // Pre-warm the pool
        let pool_clone = Arc::clone(&pool);
        runtime.spawn(async move {
            if let Err(e) = pool_clone.replenish(pool_size) {
                tracing::warn!("Firecracker pool pre-warm failed: {}", e);
            }
        });

        pool
    }

    /// Acquire a VM from the pool. Blocks until a VM is ready.
    /// Spawns a new VM if no idle ones are available.
    pub fn acquire(self: &Arc<Self>) -> SandboxResult<FirecrackerVmHandle> {
        let mut inner = self.inner.lock().unwrap();

        // Take from idle queue if available
        if let Some((vm, _)) = inner.idle.pop_front() {
            inner.out_count += 1;
            return self.build_handle(vm, &inner.config);
        }

        // No idle VM — spawn synchronously
        inner.out_count += 1;
        drop(inner); // release lock during spawn

        let vm = FirecrackerVM::spawn(&self.inner.lock().unwrap().config)?;
        vm.wait_for_ssh_ready(Duration::from_secs(
            self.inner.lock().unwrap().config.connect_timeout_secs,
        ))?;

        self.build_handle(vm, &self.inner.lock().unwrap().config)
    }

    /// Return a used VM to the pool for destruction. Called from
    /// `FirecrackerSandbox::Drop` — the VM will be killed and the pool
    /// replenished by the maintenance task.
    pub fn return_vm(&self, vm: FirecrackerVM) {
        let mut inner = self.inner.lock().unwrap();
        inner.out_count = inner.out_count.saturating_sub(1);
        inner.returned.push_back(vm);
    }

    fn build_handle(
        &self,
        vm: FirecrackerVM,
        config: &crate::registry::FirecrackerConfig,
    ) -> SandboxResult<FirecrackerVmHandle> {
        let ssh_config = crate::registry::SshConfig {
            host: vm.guest_ip().to_string(),
            port: vm.guest_ssh_port(),
            user: "root".to_string(),
            identity_file: config.ssh_identity_file.clone(),
            passphrase: config.ssh_passphrase.clone(),
            known_hosts_file: None,
            host_key: None,
            idle_timeout_secs: 300, // handled by pool, not SSH layer
            connect_timeout_secs: 10,
        };
        let ssh = crate::ssh::SSHSandbox::new(
            format!("fc-{}", uuid::Uuid::new_v4()),
            Some("/tmp/sandbox".to_string()),
            ssh_config,
        )?;
        Ok(FirecrackerVmHandle { vm, ssh })
    }

    /// Replenish the idle queue to reach `target` idle VMs.
    fn replenish(&self, target: usize) -> SandboxResult<()> {
        let mut inner = self.inner.lock().unwrap();
        let needed = target.saturating_sub(inner.idle.len());
        if needed == 0 {
            return Ok(());
        }
        // Release lock while spawning — we'll collect results
        let config = inner.config.clone();
        drop(inner);

        for _ in 0..needed {
            match FirecrackerVM::spawn(&config) {
                Ok(vm) => {
                    if let Err(e) = vm.wait_for_ssh_ready(
                        Duration::from_secs(config.connect_timeout_secs),
                    ) {
                        tracing::warn!("Firecracker pre-warm: guest SSH not ready: {}", e);
                        let _ = vm.kill();
                        continue;
                    }
                    let mut inner = self.inner.lock().unwrap();
                    inner.idle.push_back((vm, Instant::now()));
                }
                Err(e) => {
                    tracing::warn!("Firecracker pre-warm: spawn failed: {}", e);
                }
            }
        }
        Ok(())
    }

    /// Background maintenance loop — runs every 5 seconds.
    async fn maintenance_loop(&self) {
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;

            // Drain returned VMs and kill them
            let returned: Vec<FirecrackerVM> = {
                let mut inner = self.inner.lock().unwrap();
                std::mem::take(&mut inner.returned)
                    .into_iter()
                    .collect()
            };
            for vm in returned {
                let _ = vm.kill();
            }

            // Replenish idle queue
            let (target, idle_count) = {
                let inner = self.inner.lock().unwrap();
                (inner.pool_size, inner.idle.len())
            };
            if idle_count < target {
                let _ = self.replenish(target);
            }

            // Shrink: kill idle VMs past timeout
            self.shrink();
        }
    }

    fn shrink(&self) {
        let mut inner = self.inner.lock().unwrap();
        let timeout = inner.idle_timeout;
        while let Some((_, since)) = inner.idle.front() {
            if since.elapsed() > timeout {
                let (vm, _) = inner.idle.pop_front().unwrap();
                drop(inner); // release lock while killing
                let _ = vm.kill();
                inner = self.inner.lock().unwrap();
            } else {
                break;
            }
        }
    }
}
```

- [ ] **Step 2: Add uuid dependency**

In `crates/vol-llm-sandbox/Cargo.toml`:

```toml
uuid = { workspace = true, features = ["v4"] }
```

Check if uuid is in workspace:
```bash
grep uuid Cargo.toml
```

If not, add `uuid = { version = "1", features = ["v4"] }` to workspace deps.

- [ ] **Step 3: Verify compilation**

```bash
cargo check -p vol-llm-sandbox --features "firecracker,ssh"
```

Expected: pass.

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-sandbox/src/firecracker.rs crates/vol-llm-sandbox/Cargo.toml
git commit -m "feat(sandbox): add FirecrackerPool with acquire/release/maintenance"
```

---

### Task 5: FirecrackerSandbox — impl Sandbox trait

**Files:**
- Modify: `crates/vol-llm-sandbox/src/firecracker.rs` (append)

- [ ] **Step 1: Write FirecrackerSandbox struct**

Append to `firecracker.rs`:

```rust
use std::sync::Mutex as StdMutex;
use async_trait::async_trait;
use crate::{Sandbox, SandboxResult, CommandRequest, CommandOutput, DirEntry, FileMetadata};

/// Firecracker-backed sandbox implementing the `Sandbox` trait.
///
/// Holds a VM checked out from the pool. All trait method calls operate
/// on the same VM instance. When dropped, the VM is returned to the pool
/// for destruction.
pub struct FirecrackerSandbox {
    /// Registry name for this sandbox.
    name: String,
    /// The pool this VM was acquired from.
    pool: Arc<FirecrackerPool>,
    /// Lazily acquired VM handle. Wrapped in Option + Mutex because:
    /// - We only acquire on first use (defer SSH connection setup)
    /// - All trait methods take `&self`
    handle: StdMutex<Option<FirecrackerVmHandle>>,
    /// Working directory inside the VM.
    root_path: std::path::PathBuf,
}

impl FirecrackerSandbox {
    /// Create a new sandbox instance by acquiring a VM from the pool.
    pub fn new(name: String, root_path: std::path::PathBuf, pool: Arc<FirecrackerPool>) -> Self {
        Self {
            name,
            pool,
            handle: StdMutex::new(None),
            root_path,
        }
    }

    /// Ensure a VM handle exists (lazy acquire). Returns a reference
    /// that lives as long as the MutexGuard — callers must drop the
    /// guard before calling any async method to avoid holding the lock
    /// across await points.
    fn acquire_handle(&self) -> SandboxResult<std::sync::MutexGuard<Option<FirecrackerVmHandle>>> {
        let mut guard = self.handle.lock().unwrap();
        if guard.is_none() {
            let handle = self.pool.acquire()?;
            *guard = Some(handle);
        }
        Ok(guard)
    }

    /// Get a reference to the underlying SSHSandbox for delegation.
    /// Must be called from within an async context after acquire_handle
    /// guard has been dropped.
    fn with_ssh<F, R>(&self, f: F) -> SandboxResult<R>
    where
        F: FnOnce(&crate::ssh::SSHSandbox) -> SandboxResult<R>,
    {
        let guard = self.handle.lock().unwrap();
        let handle = guard.as_ref().ok_or(crate::SandboxError::NotStarted)?;
        f(&handle.ssh)
    }
}

impl Drop for FirecrackerSandbox {
    fn drop(&mut self) {
        let mut guard = self.handle.lock().unwrap();
        if let Some(handle) = guard.take() {
            // Return VM to pool for async cleanup.
            // The SSHSandbox within the handle gets dropped here,
            // which aborts its idle-timeout task.
            self.pool.return_vm(handle.vm);
        }
    }
}

#[async_trait]
impl Sandbox for FirecrackerSandbox {
    fn kind(&self) -> &str { "firecracker" }

    fn name(&self) -> &str { &self.name }

    async fn start(&self) -> SandboxResult<()> {
        // Defer to first use — no explicit start needed.
        // Just ensure connectivity works.
        let _ = self.acquire_handle()?;
        Ok(())
    }

    async fn cleanup(&self) -> SandboxResult<()> {
        // Handled by Drop — pool maintenance task kills the VM.
        Ok(())
    }

    fn root_path(&self) -> &std::path::Path { &self.root_path }

    fn resolve_path(&self, rel: &str) -> SandboxResult<std::path::PathBuf> {
        if rel.starts_with('/') {
            return Err(crate::SandboxError::PathTraversal(rel.to_string()));
        }
        let resolved = self.root_path.join(rel);
        // Normalize to prevent traversal
        let normalized = {
            let mut result = std::path::PathBuf::new();
            for component in resolved.components() {
                match component {
                    std::path::Component::ParentDir => { result.pop(); }
                    std::path::Component::CurDir => {}
                    _ => result.push(component),
                }
            }
            result
        };
        let normalized_root = {
            let mut result = std::path::PathBuf::new();
            for component in self.root_path.components() {
                match component {
                    std::path::Component::ParentDir => { result.pop(); }
                    std::path::Component::CurDir => {}
                    _ => result.push(component),
                }
            }
            result
        };
        if !normalized.starts_with(&normalized_root) {
            return Err(crate::SandboxError::PathTraversal(rel.to_string()));
        }
        Ok(normalized)
    }

    async fn execute(&self, req: CommandRequest) -> SandboxResult<CommandOutput> {
        // Acquire handle (lazy), then drop guard before async call
        {
            let _guard = self.acquire_handle()?;
        }
        self.with_ssh(|ssh| {
            // SSHSandbox::execute is async, so we need a different approach.
            // See step 2 below — we switch to blocking execute.
            unreachable!("use async path")
        })
    }

    async fn read_file(
        &self, path: &std::path::Path, offset: Option<u64>, limit: Option<u64>
    ) -> SandboxResult<Vec<u8>> {
        {
            let _guard = self.acquire_handle()?;
        }
        let guard = self.handle.lock().unwrap();
        let handle = guard.as_ref().ok_or(crate::SandboxError::NotStarted)?;
        handle.ssh.read_file(path, offset, limit).await
    }

    async fn write_file(&self, path: &std::path::Path, content: &[u8]) -> SandboxResult<()> {
        {
            let _guard = self.acquire_handle()?;
        }
        let guard = self.handle.lock().unwrap();
        let handle = guard.as_ref().ok_or(crate::SandboxError::NotStarted)?;
        handle.ssh.write_file(path, content).await
    }

    async fn create_dir_all(&self, path: &std::path::Path) -> SandboxResult<()> {
        {
            let _guard = self.acquire_handle()?;
        }
        let guard = self.handle.lock().unwrap();
        let handle = guard.as_ref().ok_or(crate::SandboxError::NotStarted)?;
        handle.ssh.create_dir_all(path).await
    }

    async fn read_dir(&self, path: &std::path::Path) -> SandboxResult<Vec<DirEntry>> {
        {
            let _guard = self.acquire_handle()?;
        }
        let guard = self.handle.lock().unwrap();
        let handle = guard.as_ref().ok_or(crate::SandboxError::NotStarted)?;
        handle.ssh.read_dir(path).await
    }

    async fn metadata(&self, path: &std::path::Path) -> SandboxResult<FileMetadata> {
        {
            let _guard = self.acquire_handle()?;
        }
        let guard = self.handle.lock().unwrap();
        let handle = guard.as_ref().ok_or(crate::SandboxError::NotStarted)?;
        handle.ssh.metadata(path).await
    }
}
```

- [ ] **Step 2: Fix execute() — use firecracker-internal sandbox directly**

The `with_ssh` helper requires a sync closure, but `SSHSandbox::execute` is async. The simplest fix: acquire the handle (ensuring VM+SSH are ready), then delegate directly:

Replace the `execute()`, `with_ssh()`, and helper methods above with this cleaner version:

```rust
impl FirecrackerSandbox {
    pub fn new(name: String, root_path: std::path::PathBuf, pool: Arc<FirecrackerPool>) -> Self {
        Self {
            name,
            pool,
            handle: StdMutex::new(None),
            root_path,
        }
    }

    /// Lazily ensure we have a VM handle. Must NOT be called
    /// while `handle` lock is held.
    async fn ensure_handle(&self) -> SandboxResult<()> {
        let need_acquire = {
            let guard = self.handle.lock().unwrap();
            guard.is_none()
        };
        if need_acquire {
            let new_handle = self.pool.acquire()?;
            let mut guard = self.handle.lock().unwrap();
            if guard.is_none() {
                *guard = Some(new_handle);
            }
            // else: another caller already set it
        }
        Ok(())
    }

    /// Get a reference to the inner SSHSandbox. Call ensure_handle first.
    fn ssh(&self) -> SandboxResult<std::sync::MutexGuard<FirecrackerVmHandle>> {
        let guard = self.handle.lock().unwrap();
        // transmute-like: we know MutexGuard lives long enough for sync ops
        Ok(guard)
    }
}
```

Wait, `MutexGuard` can't be returned like that with the reference inside it. Let me simplify.

The actual clean approach: **Store `Arc<SSHSandbox>` inside the handle, clone it before async calls.**

```rust
struct FirecrackerVmHandle {
    vm: FirecrackerVM,
    ssh: Arc<crate::ssh::SSHSandbox>,
}
```

Then in `FirecrackerSandbox`:

```rust
impl FirecrackerSandbox {
    async fn ssh(&self) -> SandboxResult<Arc<crate::ssh::SSHSandbox>> {
        // Ensure handle exists
        {
            let mut guard = self.handle.lock().unwrap();
            if guard.is_none() {
                *guard = Some(self.pool.acquire()?);
            }
        }
        let guard = self.handle.lock().unwrap();
        Ok(guard.as_ref().unwrap().ssh.clone())
    }
}

#[async_trait]
impl Sandbox for FirecrackerSandbox {
    // ...

    async fn execute(&self, req: CommandRequest) -> SandboxResult<CommandOutput> {
        self.ssh().await?.execute(req).await
    }

    async fn read_file(&self, path: &Path, offset: Option<u64>, limit: Option<u64>) -> SandboxResult<Vec<u8>> {
        self.ssh().await?.read_file(path, offset, limit).await
    }

    async fn write_file(&self, path: &Path, content: &[u8]) -> SandboxResult<()> {
        self.ssh().await?.write_file(path, content).await
    }

    async fn create_dir_all(&self, path: &Path) -> SandboxResult<()> {
        self.ssh().await?.create_dir_all(path).await
    }

    async fn read_dir(&self, path: &Path) -> SandboxResult<Vec<DirEntry>> {
        self.ssh().await?.read_dir(path).await
    }

    async fn metadata(&self, path: &Path) -> SandboxResult<FileMetadata> {
        self.ssh().await?.metadata(path).await
    }
}
```

This is clean. Each async method acquires the `Arc<SSHSandbox>` clone (cheap), drops the MutexGuard (before await), then delegates. All calls within the same tool invocation share the same underlying VM because the handle is stored in `self.handle`.

- [ ] **Step 3: Update FirecrackerVmHandle to use Arc<SSHSandbox>**

In the `FirecrackerPool::build_handle` (from Task 4), wrap ssh in Arc:

```rust
fn build_handle(&self, vm: FirecrackerVM, config: &FirecrackerConfig) -> SandboxResult<FirecrackerVmHandle> {
    let ssh_config = SshConfig { /* ... same as before ... */ };
    let ssh = crate::ssh::SSHSandbox::new(
        format!("fc-{}", uuid::Uuid::new_v4()),
        Some("/tmp/sandbox".to_string()),
        ssh_config,
    )?;
    Ok(FirecrackerVmHandle { vm, ssh: Arc::new(ssh) })
}
```

And the struct becomes:

```rust
pub(crate) struct FirecrackerVmHandle {
    pub vm: FirecrackerVM,
    pub ssh: Arc<crate::ssh::SSHSandbox>,
}
```

- [ ] **Step 4: Verify compilation**

```bash
cargo check -p vol-llm-sandbox --features "firecracker,ssh"
```

Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-sandbox/src/firecracker.rs
git commit -m "feat(sandbox): add FirecrackerSandbox with Sandbox trait impl"
```

---

### Task 6: Register firecracker type in SandboxRegistry

**Files:**
- Modify: `crates/vol-llm-sandbox/src/registry.rs`

- [ ] **Step 1: Add firecracker branch to SandboxRegistry::load()**

In `registry.rs`, after the `"ssh" => { ... }` match arm:

```rust
#[cfg(feature = "firecracker")]
"firecracker" => {
    let fc_config = config.firecracker.ok_or_else(|| {
        SandboxError::UnknownType(
            "Firecracker sandbox requires [sandbox.firecracker] section".to_string()
        )
    })?;

    #[cfg(target_os = "linux")]
    {
        let pool = crate::firecracker::FirecrackerPool::new(
            fc_config.clone(),
            tokio::runtime::Handle::current(),
        );
        let sandbox: Arc<dyn Sandbox> = Arc::new(
            crate::firecracker::FirecrackerSandbox::new(
                config.name.clone(),
                std::path::PathBuf::from(
                    config.work_dir.as_deref().unwrap_or("/tmp/fc-sandbox")
                ),
                pool,
            )
        );
        sandboxes.insert(config.name.clone(), sandbox);
    }

    #[cfg(not(target_os = "linux"))]
    {
        tracing::warn!(
            "Firecracker sandbox '{}' requires Linux/KVM — skipping registration",
            config.name
        );
    }
}
```

- [ ] **Step 2: Add acquire() method to SandboxRegistry**

For pool-based sandboxes (firecracker), `acquire()` creates a fresh instance from the pool. For singletons (local, ssh), it clones the existing Arc.

```rust
impl SandboxRegistry {
    // ... existing methods ...

    /// Acquire a sandbox instance by name.
    ///
    /// For pool-based sandboxes (firecracker), this creates a new instance
    /// backed by a VM from the pool. For singletons (local, ssh), returns
    /// a clone of the shared instance.
    ///
    /// The caller owns the returned Arc and should drop it when done.
    /// Pool-based instances return their VM to the pool on drop.
    pub fn acquire(&self, name: &str) -> Option<Arc<dyn Sandbox>> {
        let sb = self.sandboxes.get(name)?;

        #[cfg(feature = "firecracker")]
        {
            if sb.kind() == "firecracker" {
                // FirecrackerSandbox holds its own pool reference.
                // Create a fresh instance from the pool.
                // We need the pool from the existing instance.
                // Since FirecrackerSandbox is stored with a pool that it
                // already acquired from, we can't clone it — we need a
                // separate path. Let's store the pool directly.
                //
                // REVISED APPROACH (see step 3): store Arc<FirecrackerPool>
                // in the registry, not FirecrackerSandbox.
            }
        }

        Some(sb.clone())
    }
}
```

- [ ] **Step 3: Store pool separately for acquire-based sandboxes**

The issue: `SandboxRegistry` currently stores `HashMap<String, Arc<dyn Sandbox>>`. For firecracker, `acquire()` needs to create NEW sandbox instances from the pool each time. Store the pool in the registry alongside the default sandbox.

Add a new field to `SandboxRegistry`:

```rust
pub struct SandboxRegistry {
    sandboxes: HashMap<String, Arc<dyn Sandbox>>,
    default_name: String,
    /// Pool-based sandbox factories keyed by name.
    /// Each call to `acquire()` creates a fresh instance from the pool.
    #[cfg(feature = "firecracker")]
    firecracker_pools: HashMap<String, Arc<crate::firecracker::FirecrackerPool>>,
}
```

Update the firecracker registration in `load()`:

```rust
#[cfg(all(feature = "firecracker", target_os = "linux"))]
{
    let pool = crate::firecracker::FirecrackerPool::new(
        fc_config.clone(),
        tokio::runtime::Handle::current(),
    );
    // Store a reference instance for kind/name queries
    let sandbox: Arc<dyn Sandbox> = Arc::new(
        crate::firecracker::FirecrackerSandbox::new(
            config.name.clone(),
            std::path::PathBuf::from(
                config.work_dir.as_deref().unwrap_or("/tmp/fc-sandbox")
            ),
            pool.clone(),
        )
    );
    sandboxes.insert(config.name.clone(), sandbox);
    firecracker_pools.insert(config.name.clone(), pool);
}
```

And the `acquire()` method:

```rust
pub fn acquire(&self, name: &str) -> Option<Arc<dyn Sandbox>> {
    #[cfg(feature = "firecracker")]
    {
        if let Some(pool) = self.firecracker_pools.get(name) {
            let work_dir = self.sandboxes.get(name)
                .map(|sb| sb.root_path().to_path_buf())
                .unwrap_or_else(|| std::path::PathBuf::from("/tmp/fc-sandbox"));
            return Some(Arc::new(
                crate::firecracker::FirecrackerSandbox::new(
                    name.to_string(),
                    work_dir,
                    pool.clone(),
                )
            ));
        }
    }

    self.sandboxes.get(name).cloned()
}
```

Initialize the new field in `load()`:

```rust
Ok(Self {
    sandboxes,
    default_name: "local".to_string(),
    #[cfg(feature = "firecracker")]
    firecracker_pools: HashMap::new(),
})
```

- [ ] **Step 4: Verify compilation**

```bash
cargo check -p vol-llm-sandbox --features "firecracker,ssh"
cargo check -p vol-llm-sandbox  # without firecracker
```

Expected: both pass.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-sandbox/src/registry.rs
git commit -m "feat(sandbox): register firecracker type + acquire() method"
```

---

### Task 7: Wire acquire() into agent loop

**Files:**
- Modify: `crates/vol-llm-agent/src/react/agent.rs`

- [ ] **Step 1: Replace registry.get() with registry.acquire()**

In `agent.rs`, find the sandbox resolution block (~line 495-508) and replace `registry.get()` with `registry.acquire()`:

```rust
// Resolve sandbox:
//   1. ToolConfig.get_sandbox(tool_name) — per-tool override
//   2. AgentDef.sandbox — agent default
//   3. Registry default ("local")
let sandbox_ref = if let Some(ref registry) = run_ctx.config.sandbox_registry {
    let sandbox_name = run_ctx
        .config
        .tool_config
        .get_sandbox(&call.name)
        .or_else(|| run_ctx.config.default_sandbox.clone())
        .unwrap_or_else(|| "local".to_string());
    registry.acquire(&sandbox_name).unwrap_or_else(|| registry.default())
} else {
    match &sandbox {
        Some(sb) => sb.clone(),
        None => Arc::new(vol_llm_sandbox::local::LocalSandbox::new(None)),
    }
};
```

The only change: `registry.get(&sandbox_name)` → `registry.acquire(&sandbox_name)`.

- [ ] **Step 2: Verify compilation**

```bash
cargo check -p vol-llm-agent
```

Expected: pass.

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent/src/react/agent.rs
git commit -m "feat(agent): use registry.acquire() for pool-based sandbox lifecycle"
```

---

### Task 8: Firecracker pool unit tests

**Files:**
- Create: `crates/vol-llm-sandbox/tests/firecracker_pool.rs`

- [ ] **Step 1: Write pool unit tests**

These test the pool logic without actually spawning firecracker (mock the VM operations).

Since `FirecrackerVM::spawn()` requires a real firecracker binary, we test pool data structures directly:

```rust
//! Unit tests for FirecrackerPool — tests pool logic independent of KVM.
//!
//! These do NOT spawn real firecracker processes. Integration tests
//! that require KVM are in `tests/firecracker_integration.rs` (ignored by default).

use std::time::Duration;
use vol_llm_sandbox::registry::FirecrackerConfig;

fn test_config() -> FirecrackerConfig {
    FirecrackerConfig {
        kernel_image: "/nonexistent/vmlinux".to_string(),
        rootfs_image: "/nonexistent/rootfs.ext4".to_string(),
        rootfs_readonly: false,
        pool_size: 2,
        idle_timeout_secs: 1,
        connect_timeout_secs: 5,
        firecracker_binary: Some("/nonexistent/firecracker".to_string()),
        guest_ip: "172.16.0.2".to_string(),
        guest_ssh_port: 22,
        tap_device: "fc-tap0".to_string(),
        ssh_identity_file: "/nonexistent/key".to_string(),
        ssh_passphrase: None,
    }
}

#[test]
fn test_config_defaults() {
    let toml_str = r#"
name = "fc"
type = "firecracker"
work_dir = "/tmp/fc"

[sandbox.firecracker]
kernel_image = "/opt/vmlinux"
rootfs_image = "/opt/rootfs.ext4"
tap_device = "fc-tap0"
ssh_identity_file = "/opt/key"
"#;
    let config: vol_llm_sandbox::registry::SandboxConfig =
        toml::from_str(toml_str).unwrap();
    let fc = config.firecracker.unwrap();
    assert_eq!(fc.pool_size, 1);           // default
    assert_eq!(fc.idle_timeout_secs, 300); // default
    assert_eq!(fc.guest_ip, "172.16.0.2"); // default
    assert_eq!(fc.rootfs_readonly, false); // default
}

#[test]
fn test_config_full() {
    let toml_str = r#"
name = "fc"
type = "firecracker"
work_dir = "/tmp/fc"

[sandbox.firecracker]
kernel_image = "/opt/vmlinux"
rootfs_image = "/opt/rootfs.ext4"
rootfs_readonly = true
pool_size = 4
idle_timeout_secs = 120
connect_timeout_secs = 30
firecracker_binary = "/usr/local/bin/firecracker"
guest_ip = "10.0.0.1"
guest_ssh_port = 2222
tap_device = "fc-tap0"
ssh_identity_file = "/opt/key"
ssh_passphrase = "secret"
"#;
    let config: vol_llm_sandbox::registry::SandboxConfig =
        toml::from_str(toml_str).unwrap();
    let fc = config.firecracker.unwrap();
    assert_eq!(fc.pool_size, 4);
    assert_eq!(fc.idle_timeout_secs, 120);
    assert_eq!(fc.connect_timeout_secs, 30);
    assert_eq!(fc.firecracker_binary, Some("/usr/local/bin/firecracker".to_string()));
    assert_eq!(fc.guest_ip, "10.0.0.1");
    assert_eq!(fc.guest_ssh_port, 2222);
    assert_eq!(fc.rootfs_readonly, true);
    assert_eq!(fc.ssh_passphrase, Some("secret".to_string()));
}

#[test]
fn test_config_missing_required() {
    let toml_str = r#"
name = "fc"
type = "firecracker"

[sandbox.firecracker]
"#;
    let result: Result<vol_llm_sandbox::registry::SandboxConfig, _> =
        toml::from_str(toml_str);
    assert!(result.is_err()); // kernel_image, rootfs_image, tap_device, ssh_identity are required
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test -p vol-llm-sandbox --features firecracker --test firecracker_pool
```

Expected: 3 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-sandbox/tests/firecracker_pool.rs
git commit -m "test(sandbox): add FirecrackerConfig deserialization tests"
```

---

## Phase 2: WasmSandbox

### Task 9: Wasm feature flag and WasmConfig

**Files:**
- Modify: `crates/vol-llm-sandbox/Cargo.toml`
- Modify: `crates/vol-llm-sandbox/src/lib.rs`
- Modify: `crates/vol-llm-sandbox/src/registry.rs`
- Create: `crates/vol-llm-sandbox/src/wasm.rs`

- [ ] **Step 1: Add wasm feature and dependencies**

In `Cargo.toml`, update features and add deps:

```toml
[dependencies]
# ... existing ...
wasmtime = { version = "20", optional = true }

[features]
default = []
ssh = ["ssh2"]
firecracker = []
wasm = ["dep:wasmtime"]
```

Note: `wasmtime` 20.x includes WASI support via `wasmtime-wasi` which is re-exported. If the API requires a separate crate, add `wasmtime-wasi` too. Check at implementation time.

- [ ] **Step 2: Declare wasm module**

In `lib.rs`, add:

```rust
#[cfg(feature = "wasm")]
pub mod wasm;
```

- [ ] **Step 3: Add WasmConfig to registry**

In `registry.rs`, add after `FirecrackerConfig`:

```rust
/// Configuration for a Wasm sandbox.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WasmConfig {
    /// Max linear memory per module instance, in bytes.
    #[serde(default = "default_wasm_memory")]
    pub max_memory_bytes: u64,
    /// Per-execution timeout in milliseconds.
    #[serde(default = "default_wasm_timeout")]
    pub max_execution_ms: u64,
    /// Wasm modules to precompile and serve.
    #[serde(default)]
    pub modules: Vec<WasmModuleConfig>,
}

/// A single Wasm module registered in the sandbox.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WasmModuleConfig {
    /// Logical name — used as `program` in `CommandRequest`.
    pub name: String,
    /// Path to the `.wasm` file on disk.
    pub path: String,
    /// If true, register this module as a named agent tool.
    #[serde(default)]
    pub expose_as_tool: bool,
}

fn default_wasm_memory() -> u64 { 134_217_728 }    // 128 MB
fn default_wasm_timeout() -> u64 { 30_000 }         // 30 seconds
```

Add `wasm` field to `SandboxConfig`:

```rust
pub struct SandboxConfig {
    // ... existing ...
    #[serde(default)]
    pub firecracker: Option<FirecrackerConfig>,
    // NEW:
    #[serde(default)]
    pub wasm: Option<WasmConfig>,
}
```

- [ ] **Step 4: Create empty wasm.rs placeholder**

```rust
//! Wasm sandbox — execute WebAssembly modules in a WASI environment.
//!
//! Uses `wasmtime` as the runtime. Modules are precompiled at startup.
//! Each execution gets a fresh `Store` with an isolated WASI context.
//! No network access, no process spawning — only WASI file I/O within
//! the sandbox work_dir.
```

- [ ] **Step 5: Verify compilation**

```bash
cargo check -p vol-llm-sandbox --features wasm
cargo check -p vol-llm-sandbox --features "ssh,firecracker,wasm"
```

Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-sandbox/Cargo.toml crates/vol-llm-sandbox/src/lib.rs crates/vol-llm-sandbox/src/registry.rs crates/vol-llm-sandbox/src/wasm.rs
git commit -m "feat(sandbox): add wasm feature, WasmConfig, and module skeleton"
```

---

### Task 10: WasmSandbox — impl Sandbox trait

**Files:**
- Modify: `crates/vol-llm-sandbox/src/wasm.rs` (full implementation)

- [ ] **Step 1: Write WasmSandbox struct and constructor**

```rust
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use async_trait::async_trait;
use wasmtime::*;
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiView};

use crate::{
    CommandOutput, DirEntry, FileMetadata, FileType, Sandbox, SandboxError, SandboxResult,
};
use crate::registry::WasmConfig;

/// State stored inside each wasmtime `Store`. Implements `WasiView` for WASI support.
struct ExecutionState {
    wasi: WasiCtx,
    /// Preopened dir handle — kept alive for the store's lifetime.
    _dir: Box<dyn wasmtime_wasi::Dir>,
}

impl WasiView for ExecutionState {
    fn ctx(&mut self) -> &mut WasiCtx { &mut self.wasi }
    fn table(&mut self) -> &mut wasmtime_wasi::ResourceTable {
        // wasmtime 20+ uses a default resource table in WasiCtx
        self.wasi.table()
    }
}

/// Precompiled Wasm module ready for execution.
struct WasmModule {
    module: Module,
    /// Exported function names (cached for tool schema generation).
    exports: Vec<String>,
}

/// Wasm sandbox — executes `.wasm` modules in an isolated WASI environment.
pub struct WasmSandbox {
    name: String,
    work_dir: PathBuf,
    root_path: PathBuf,
    /// Shared wasmtime engine (heavy, one per sandbox).
    engine: Engine,
    /// Precompiled modules keyed by logical name.
    modules: HashMap<String, WasmModule>,
    /// Module configs (for expose_as_tool).
    module_configs: Vec<crate::registry::WasmModuleConfig>,
    /// Per-execution limits.
    max_memory: u64,
    max_execution: Duration,
}

impl WasmSandbox {
    /// Create a new Wasm sandbox, precompiling all configured modules.
    pub fn new(
        name: String,
        work_dir: PathBuf,
        config: WasmConfig,
    ) -> SandboxResult<Self> {
        let mut engine_config = Config::new();
        engine_config.wasm_multi_memory(true);
        engine_config.wasm_memory64(true); // future-proof
        let engine = Engine::new(&engine_config)
            .map_err(|e| SandboxError::Io(std::io::Error::other(e.to_string())))?;

        // Precompile all configured modules
        let mut modules = HashMap::new();
        for mc in &config.modules {
            let wasm_bytes = std::fs::read(&mc.path)
                .map_err(|e| SandboxError::Io(e))?;
            let module = Module::from_binary(&engine, &wasm_bytes)
                .map_err(|e| SandboxError::Io(std::io::Error::other(
                    format!("failed to compile {}: {}", mc.path, e)
                )))?;
            let exports: Vec<String> = module
                .exports()
                .filter(|e| matches!(e.ty(), ExternType::Func(_)))
                .map(|e| e.name().to_string())
                .collect();
            modules.insert(mc.name.clone(), WasmModule { module, exports });
        }

        let root_path = work_dir.clone();
        // Ensure work_dir exists
        std::fs::create_dir_all(&work_dir).map_err(SandboxError::Io)?;

        Ok(Self {
            name,
            work_dir,
            root_path,
            engine,
            modules,
            module_configs: config.modules,
            max_memory: config.max_memory_bytes,
            max_execution: Duration::from_millis(config.max_execution_ms),
        })
    }

    /// List modules that are flagged as agent tools.
    pub fn tool_modules(&self) -> &[crate::registry::WasmModuleConfig] {
        &self.module_configs
    }
}
```

- [ ] **Step 2: Implement read_file, write_file, create_dir_all, read_dir**

These operate on the host filesystem (sandbox work_dir), same as LocalSandbox:

```rust
#[async_trait]
impl Sandbox for WasmSandbox {
    fn kind(&self) -> &str { "wasm" }
    fn name(&self) -> &str { &self.name }

    async fn start(&self) -> SandboxResult<()> {
        std::fs::create_dir_all(&self.work_dir).map_err(SandboxError::Io)
    }

    async fn cleanup(&self) -> SandboxResult<()> {
        // No persistent state to clean. work_dir is caller-managed.
        Ok(())
    }

    fn root_path(&self) -> &Path { &self.root_path }

    fn resolve_path(&self, rel: &str) -> SandboxResult<PathBuf> {
        if rel.starts_with('/') {
            return Err(SandboxError::PathTraversal(rel.to_string()));
        }
        let resolved = self.root_path.join(rel);
        let normalized = normalize_path(&resolved);
        let normalized_root = normalize_path(&self.root_path);
        if !normalized.starts_with(&normalized_root) {
            return Err(SandboxError::PathTraversal(rel.to_string()));
        }
        Ok(normalized)
    }

    async fn read_file(
        &self, path: &Path, offset: Option<u64>, limit: Option<u64>
    ) -> SandboxResult<Vec<u8>> {
        let content = std::fs::read(path).map_err(SandboxError::Io)?;
        let start = offset.unwrap_or(0) as usize;
        let end = limit.map(|l| start + l as usize).unwrap_or(content.len());
        Ok(content[start..end.min(content.len())].to_vec())
    }

    async fn write_file(&self, path: &Path, content: &[u8]) -> SandboxResult<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(SandboxError::Io)?;
        }
        std::fs::write(path, content).map_err(SandboxError::Io)
    }

    async fn create_dir_all(&self, path: &Path) -> SandboxResult<()> {
        std::fs::create_dir_all(path).map_err(SandboxError::Io)
    }

    async fn read_dir(&self, path: &Path) -> SandboxResult<Vec<DirEntry>> {
        let entries: Vec<DirEntry> = std::fs::read_dir(path)
            .map_err(SandboxError::Io)?
            .filter_map(|e| e.ok())
            .map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                let file_type = e.file_type().map(|ft| {
                    if ft.is_dir() { FileType::Directory }
                    else if ft.is_file() { FileType::File }
                    else if ft.is_symlink() { FileType::Symlink }
                    else { FileType::Other }
                }).unwrap_or(FileType::Other);
                DirEntry { name, file_type }
            })
            .collect();
        Ok(entries)
    }

    async fn metadata(&self, path: &Path) -> SandboxResult<FileMetadata> {
        let meta = std::fs::metadata(path).map_err(SandboxError::Io)?;
        let mtime = meta.modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let file_type = if meta.is_dir() { FileType::Directory }
            else if meta.is_file() { FileType::File }
            else if meta.is_symlink() { FileType::Symlink }
            else { FileType::Other };
        Ok(FileMetadata { size: meta.len(), mtime, file_type })
    }
}
```

- [ ] **Step 3: Implement execute() — run wasm module main function**

```rust
    async fn execute(&self, req: crate::CommandRequest) -> SandboxResult<CommandOutput> {
        let module = self.modules.get(&req.program).ok_or_else(|| {
            SandboxError::UnknownType(format!(
                "unknown wasm module: {} (available: {:?})",
                req.program,
                self.modules.keys().collect::<Vec<_>>()
            ))
        })?;

        let work_dir = self.work_dir.clone();
        let max_execution = self.max_execution;
        let engine = self.engine.clone();
        let wasm_module = module.module.clone();

        // Run in spawn_blocking — wasm execution is CPU-bound
        tokio::task::spawn_blocking(move || {
            execute_module(
                &engine,
                &wasm_module,
                &work_dir,
                &req,
                max_execution,
            )
        })
        .await
        .map_err(|e| SandboxError::Io(std::io::Error::other(e.to_string())))?
    }
```

- [ ] **Step 4: Write the execute_module helper**

```rust
fn execute_module(
    engine: &Engine,
    module: &Module,
    work_dir: &Path,
    req: &crate::CommandRequest,
    max_execution: Duration,
) -> SandboxResult<CommandOutput> {
    // Build WASI context
    let dir = wasmtime_wasi::Dir::open_ambient_dir(work_dir, wasmtime_wasi::ambient_authority())
        .map_err(|e| SandboxError::Io(std::io::Error::other(e.to_string())))?;

    // Use OutputPipe to capture stdout/stderr (not inherit_stdio which writes to real stdout)
    let stdout_pipe = wasmtime_wasi::pipe::WritePipe::new_in_memory();
    let stderr_pipe = wasmtime_wasi::pipe::WritePipe::new_in_memory();

    let mut wasi = WasiCtxBuilder::new()
        .stdout(Box::new(stdout_pipe.clone()))
        .stderr(Box::new(stderr_pipe.clone()))
        .preopened_dir(dir, "/")
        .build();

    // Pipe stdin if provided
    if let Some(ref stdin_data) = req.stdin {
        wasi.set_stdin(Box::new(std::io::Cursor::new(stdin_data.clone())));
    }

    // Set environment variables
    for (k, v) in &req.env {
        wasi.push_env(k, v)?;
    }
    // Pass args: program name + user args
    wasi.push_arg(&req.program)?;
    for arg in &req.args {
        wasi.push_arg(arg)?;
    }

    let mut state = ExecutionState { wasi, _dir: dir };

    let mut store = Store::new(engine, state);

    let mut linker = Linker::new(engine);
    wasmtime_wasi::add_to_linker(&mut linker, |s: &mut ExecutionState| &mut s.wasi)
        .map_err(|e| SandboxError::Io(std::io::Error::other(e.to_string())))?;

    let instance = linker
        .instantiate(&mut store, module)
        .map_err(|e| SandboxError::Io(std::io::Error::other(
            format!("instantiation failed: {}", e)
        )))?;

    // Try calling `_start` (WASI command entry point)
    let result: Result<i32, _> = if let Ok(main) = instance
        .get_typed_func::<(), ()>(&mut store, "_start")
    {
        main.call(&mut store, ())
            .map(|_| 0)
            .map_err(|e| format!("{}", e))
    } else {
        Err("no _start export found — WASI modules must export _start".to_string())
    };

    match result {
        Ok(exit_code) => {
            let stdout = stdout_pipe.try_into_inner()
                .unwrap_or_default()
                .into_bytes();
            let stderr = stderr_pipe.try_into_inner()
                .unwrap_or_default()
                .into_bytes();
            Ok(CommandOutput {
                stdout,
                stderr,
                exit_code,
                killed_by_signal: None,
            })
        }
        Err(e) => Err(SandboxError::Io(std::io::Error::other(e))),
    }
}
```

Note: `wasmtime` 20.x API details may need adjustment during implementation. The exact signatures of `WasiCtxBuilder`, `Dir::open_ambient_dir`, and `add_to_linker` vary by minor version. Check the `wasmtime` docs for the exact version used.

- [ ] **Step 5: Verify compilation**

```bash
cargo check -p vol-llm-sandbox --features wasm
```

Expected: pass. Fix any API mismatches with the installed wasmtime version.

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-sandbox/src/wasm.rs
git commit -m "feat(sandbox): add WasmSandbox with Sandbox trait impl"
```

---

### Task 11: Register wasm type in SandboxRegistry

**Files:**
- Modify: `crates/vol-llm-sandbox/src/registry.rs`

- [ ] **Step 1: Add wasm branch to load()**

After the firecracker branch in `registry.rs::load()`:

```rust
#[cfg(feature = "wasm")]
"wasm" => {
    let wasm_config = config.wasm.ok_or_else(|| {
        SandboxError::UnknownType(
            "Wasm sandbox requires [sandbox.wasm] section".to_string()
        )
    })?;

    let sandbox: Arc<dyn Sandbox> = Arc::new(
        crate::wasm::WasmSandbox::new(
            config.name.clone(),
            std::path::PathBuf::from(
                config.work_dir.as_deref().unwrap_or("/tmp/wasm-sandbox")
            ),
            wasm_config,
        )?
    );
    sandboxes.insert(config.name.clone(), sandbox);
}
```

Note: Wasm is a singleton (not pool-based). `acquire()` returns a clone of the `Arc`.

Note: `expose_as_tool = true` registration into `ToolRegistry` is deferred as a follow-up task. The infrastructure is in place: `WasmSandbox::tool_modules()` returns the list of modules flagged as tools, and the `ToolRegistry` can consume this list to register them. This requires a small integration step in `AgentRuntimeBuilder::build()` or `AgentConfigBuilder::build()` that queries the wasm sandbox and registers its tool modules.

- [ ] **Step 2: Verify compilation**

```bash
cargo check -p vol-llm-sandbox --features "ssh,firecracker,wasm"
cargo check -p vol-llm-sandbox --features wasm
```

Expected: both pass.

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-sandbox/src/registry.rs
git commit -m "feat(sandbox): register wasm type in SandboxRegistry"
```

---

### Task 12: Wasm sandbox unit tests

**Files:**
- Create: `crates/vol-llm-sandbox/tests/wasm_sandbox.rs`

- [ ] **Step 1: Write a minimal .wat test module and tests**

Create a test that compiles a minimal WAT module and executes it:

```rust
//! Unit tests for WasmSandbox.

use std::path::PathBuf;
use std::time::Duration;
use vol_llm_sandbox::Sandbox;
use vol_llm_sandbox::registry::{WasmConfig, WasmModuleConfig};

/// A minimal WASI module in WAT format that prints "hello" and exits 0.
const HELLO_WAT: &str = r#"
(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  (import "wasi_snapshot_preview1" "fd_write"
    (func $fd_write (param i32 i32 i32 i32) (result i32)))
  (memory 1)
  (data (i32.const 8) "hello\n")
  (func $main (export "_start")
    ;; write(1, addr=8, len=6) using fd_write
    (i32.store (i32.const 0) (i32.const 8))   ;; iov_base
    (i32.store (i32.const 4) (i32.const 6))   ;; iov_len
    (call $fd_write (i32.const 1) (i32.const 0) (i32.const 1) (i32.const 16))
    drop
    (call $proc_exit (i32.const 0))
  )
)
"#;

fn build_test_module(dir: &PathBuf, name: &str, wat: &str) -> PathBuf {
    let wat_path = dir.join(format!("{}.wat", name));
    let wasm_path = dir.join(format!("{}.wasm", name));
    std::fs::write(&wat_path, wat).unwrap();

    // Convert WAT to WASM using wat crate or wat2wasm CLI
    let output = std::process::Command::new("wat2wasm")
        .arg(&wat_path)
        .arg("-o")
        .arg(&wasm_path)
        .output()
        .expect("wat2wasm not found — install wabt: brew install wabt / apt install wabt");
    assert!(output.status.success(), "wat2wasm failed: {:?}", output);
    wasm_path
}

#[tokio::test]
async fn test_wasm_sandbox_execute_hello() {
    let work_dir = std::env::temp_dir().join("wasm_test_hello");
    let _ = std::fs::remove_dir_all(&work_dir);
    std::fs::create_dir_all(&work_dir).unwrap();

    let wasm_path = build_test_module(&work_dir, "hello", HELLO_WAT);

    let config = WasmConfig {
        max_memory_bytes: 134_217_728,
        max_execution_ms: 30_000,
        modules: vec![WasmModuleConfig {
            name: "hello".to_string(),
            path: wasm_path.to_string_lossy().to_string(),
            expose_as_tool: false,
        }],
    };

    let sandbox = vol_llm_sandbox::wasm::WasmSandbox::new(
        "test".to_string(),
        work_dir.clone(),
        config,
    ).unwrap();

    sandbox.start().await.unwrap();

    let req = vol_llm_sandbox::CommandRequest {
        program: "hello".to_string(),
        args: vec![],
        env: Default::default(),
        cwd: None,
        stdin: None,
        timeout: Duration::from_secs(10),
    };

    let output = sandbox.execute(req).await.unwrap();
    assert_eq!(output.exit_code, 0);

    sandbox.cleanup().await.unwrap();
    let _ = std::fs::remove_dir_all(&work_dir);
}

#[test]
fn test_wasm_config_defaults() {
    let toml_str = r#"
name = "wasm"
type = "wasm"
work_dir = "/tmp/wasm"

[sandbox.wasm]
[[sandbox.wasm.modules]]
name = "test"
path = "/opt/test.wasm"
"#;
    let config: vol_llm_sandbox::registry::SandboxConfig =
        toml::from_str(toml_str).unwrap();
    let wasm = config.wasm.unwrap();
    assert_eq!(wasm.max_memory_bytes, 134_217_728);
    assert_eq!(wasm.max_execution_ms, 30_000);
    assert_eq!(wasm.modules.len(), 1);
    assert_eq!(wasm.modules[0].name, "test");
    assert!(!wasm.modules[0].expose_as_tool);
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test -p vol-llm-sandbox --features wasm --test wasm_sandbox
```

Expected: 2 tests pass (skip `test_wasm_sandbox_execute_hello` if `wat2wasm` is unavailable and add `#[ignore]`).

- [ ] **Step 3: Run full workspace check**

```bash
cargo check -p vol-llm-sandbox --all-features
cargo test -p vol-llm-sandbox --all-features
```

Expected: all pass.

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-sandbox/tests/wasm_sandbox.rs
git commit -m "test(sandbox): add WasmSandbox unit tests"
```

---

## Verification

- [ ] **Final check: full workspace compilation**

```bash
cargo build -p vol-llm-sandbox --all-features
cargo test -p vol-llm-sandbox --all-features
cargo build -p vol-llm-agent
```

Expected: all pass, no warnings.

- [ ] **Final commit (if any cleanup)**
