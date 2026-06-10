//! Firecracker microVM sandbox — lightweight KVM-based isolation.
//!
//! Linux only. Requires the `firecracker` binary on PATH and KVM access.
//! Spawns microVMs via REST API over Unix socket, runs commands via SSH,
//! and manages a pool of pre-warmed VMs for low-latency execution.

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;

use crate::registry::{FirecrackerConfig, SshConfig};
use crate::{DirEntry, FileMetadata, Sandbox, SandboxError, SandboxResult};

// ---------------------------------------------------------------------------
// Part A: HTTP-over-Unix-socket helper
// ---------------------------------------------------------------------------

/// Send an HTTP PUT request with a JSON body to a Unix socket.
fn unix_socket_put(socket_path: &Path, uri: &str, body: &str) -> SandboxResult<()> {
    let mut stream = UnixStream::connect(socket_path)
        .map_err(|e| SandboxError::Firecracker(format!("connect to api socket: {}", e)))?;

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

    stream
        .write_all(request.as_bytes())
        .map_err(|e| SandboxError::Firecracker(format!("api write: {}", e)))?;

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|e| SandboxError::Firecracker(format!("api read: {}", e)))?;

    if !response.contains("204") && !response.contains("200") {
        return Err(SandboxError::Firecracker(format!(
            "firecracker API error: {}",
            response
        )));
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Part B: FirecrackerVM
// ---------------------------------------------------------------------------

/// A running Firecracker microVM.
pub struct FirecrackerVM {
    child: Child,
    api_socket: PathBuf,
    _temp_dir: tempfile::TempDir,
    guest_ip: String,
    guest_ssh_port: u16,
}

impl FirecrackerVM {
    /// Spawn a new Firecracker microVM using the provided configuration.
    pub fn spawn(config: &FirecrackerConfig) -> SandboxResult<Self> {
        let temp_dir = tempfile::TempDir::new().map_err(SandboxError::Io)?;
        let api_socket = temp_dir.path().join("api.sock");

        let binary = config
            .firecracker_binary
            .as_deref()
            .unwrap_or("firecracker");

        let child = Command::new(binary)
            .arg("--api-sock")
            .arg(&api_socket)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| {
                SandboxError::Firecracker(format!(
                    "failed to spawn firecracker: {} (is it installed?)",
                    e
                ))
            })?;

        let vm = Self {
            child,
            api_socket: api_socket.clone(),
            _temp_dir: temp_dir,
            guest_ip: config.guest_ip.clone(),
            guest_ssh_port: config.guest_ssh_port,
        };

        // Wait for API socket to appear
        vm.wait_for_socket(Duration::from_secs(5))?;

        // Configure the VM
        vm.configure(config)?;

        Ok(vm)
    }

    /// Poll until the Unix socket appears (up to `timeout`).
    fn wait_for_socket(&self, timeout: Duration) -> SandboxResult<()> {
        let start = Instant::now();
        while !self.api_socket.exists() {
            if start.elapsed() > timeout {
                return Err(SandboxError::Firecracker(
                    "firecracker API socket did not appear".to_string(),
                ));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        Ok(())
    }

    /// Configure kernel, rootfs, network, and start the microVM via Firecracker API.
    fn configure(&self, config: &FirecrackerConfig) -> SandboxResult<()> {
        // 1. Set kernel
        let kernel_json = serde_json::json!({
            "kernel_image_path": config.kernel_image,
            "boot_args": "console=ttyS0 reboot=k panic=1 pci=off",
        });
        unix_socket_put(&self.api_socket, "/boot-source", &kernel_json.to_string())?;

        // 2. Set rootfs
        let rootfs_json = serde_json::json!({
            "drive_id": "rootfs",
            "path_on_host": config.rootfs_image,
            "is_root_device": true,
            "is_read_only": config.rootfs_readonly,
        });
        unix_socket_put(&self.api_socket, "/drives/rootfs", &rootfs_json.to_string())?;

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
        let start_json = serde_json::json!({ "action_type": "InstanceStart" });
        unix_socket_put(&self.api_socket, "/actions", &start_json.to_string())?;

        Ok(())
    }

    /// Wait until the guest's SSH port is reachable.
    pub fn wait_for_ssh_ready(&self, timeout: Duration) -> SandboxResult<()> {
        let start = Instant::now();
        let addr = format!("{}:{}", self.guest_ip, self.guest_ssh_port);
        while start.elapsed() < timeout {
            if std::net::TcpStream::connect_timeout(
                &addr
                    .parse()
                    .map_err(|e| SandboxError::Firecracker(format!("bad address: {}", e)))?,
                Duration::from_secs(1),
            )
            .is_ok()
            {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        Err(SandboxError::Firecracker(format!(
            "guest SSH not reachable at {} after {:?}",
            addr, timeout
        )))
    }

    /// Kill the microVM process and wait for it to exit.
    pub fn kill(mut self) -> SandboxResult<()> {
        let _ = self.child.kill();
        let _ = self.child.wait();
        Ok(())
    }

    /// IP address of the guest VM.
    pub fn guest_ip(&self) -> &str {
        &self.guest_ip
    }

    /// SSH port of the guest VM.
    pub fn guest_ssh_port(&self) -> u16 {
        self.guest_ssh_port
    }
}

// ---------------------------------------------------------------------------
// Part C: FirecrackerPool
// ---------------------------------------------------------------------------

/// A handle returned when a VM is acquired from the pool, holding the VM
/// and its pre-constructed SSH sandbox.
pub struct FirecrackerVmHandle {
    pub vm: FirecrackerVM,
    pub ssh: Arc<crate::ssh::SSHSandbox>,
}

/// Pool of pre-warmed Firecracker microVMs.
///
/// Maintains an idle queue and a background task that:
/// - Drains recently-returned VMs (kills them asynchronously).
/// - Replenishes idle VMs up to `pool_size`.
/// - Shrinks stale idle VMs beyond `idle_timeout`.
pub struct FirecrackerPool {
    inner: StdMutex<PoolInner>,
    _runtime: tokio::runtime::Handle,
}

struct PoolInner {
    idle: VecDeque<(FirecrackerVM, Instant)>,
    out_count: usize,
    pool_size: usize,
    idle_timeout: Duration,
    returned: VecDeque<FirecrackerVM>,
    config: FirecrackerConfig,
}

impl FirecrackerPool {
    /// Create a new pool and start its background maintenance task.
    ///
    /// Pre-warms `pool_size` VMs immediately.
    pub fn new(config: FirecrackerConfig, runtime: tokio::runtime::Handle) -> Arc<Self> {
        let pool_size = config.pool_size;
        let idle_timeout = Duration::from_secs(config.idle_timeout_secs);
        let pool = Arc::new(Self {
            inner: StdMutex::new(PoolInner {
                idle: VecDeque::new(),
                out_count: 0,
                pool_size,
                idle_timeout,
                returned: VecDeque::new(),
                config: config.clone(),
            }),
            _runtime: runtime.clone(),
        });

        // Start maintenance task
        let pool_clone = Arc::clone(&pool);
        runtime.spawn(async move {
            pool_clone.maintenance_loop().await;
        });

        // Pre-warm the pool
        let pool_clone = Arc::clone(&pool);
        runtime.spawn(async move {
            if let Err(e) = pool_clone.replenish_async(pool_size).await {
                tracing::warn!("Firecracker pool pre-warm failed: {}", e);
            }
        });

        pool
    }

    /// Acquire a VM from the pool (or spawn one if none idle).
    pub fn acquire(self: &Arc<Self>) -> SandboxResult<FirecrackerVmHandle> {
        let mut inner = self.inner.lock().unwrap();

        if let Some((vm, _)) = inner.idle.pop_front() {
            inner.out_count += 1;
            return self.build_handle(vm, &inner.config);
        }

        inner.out_count += 1;
        drop(inner);
        self.spawn_and_build_handle()
    }

    /// Return a VM to the pool for async cleanup.
    pub fn return_vm(&self, vm: FirecrackerVM) {
        let mut inner = self.inner.lock().unwrap();
        inner.out_count = inner.out_count.saturating_sub(1);
        inner.returned.push_back(vm);
    }

    fn spawn_and_build_handle(&self) -> SandboxResult<FirecrackerVmHandle> {
        let config = self.inner.lock().unwrap().config.clone();
        let vm = FirecrackerVM::spawn(&config)?;
        vm.wait_for_ssh_ready(Duration::from_secs(config.connect_timeout_secs))?;
        self.build_handle(vm, &config)
    }

    fn build_handle(
        &self,
        vm: FirecrackerVM,
        config: &FirecrackerConfig,
    ) -> SandboxResult<FirecrackerVmHandle> {
        let ssh_config = SshConfig {
            host: vm.guest_ip().to_string(),
            port: vm.guest_ssh_port(),
            user: "root".to_string(),
            identity_file: config.ssh_identity_file.clone(),
            passphrase: config.ssh_passphrase.clone(),
            known_hosts_file: None,
            host_key: Some("".to_string()), // Accept any host key for local microVM
            idle_timeout_secs: config.idle_timeout_secs,
            connect_timeout_secs: config.connect_timeout_secs,
        };
        let ssh = crate::ssh::SSHSandbox::new(
            format!("fc-{}", std::process::id()),
            Some("/tmp/sandbox".to_string()),
            ssh_config,
        )?;
        Ok(FirecrackerVmHandle {
            vm,
            ssh: Arc::new(ssh),
        })
    }

    /// Ensure the pool has at least `target` idle VMs.
    /// Synchronous version for use in non-async contexts.
    #[allow(dead_code)]
    fn replenish(&self, target: usize) -> SandboxResult<()> {
        let needed = {
            let inner = self.inner.lock().unwrap();
            target.saturating_sub(inner.idle.len())
        };

        if needed == 0 {
            return Ok(());
        }

        let config = {
            let inner = self.inner.lock().unwrap();
            inner.config.clone()
        };

        for _ in 0..needed {
            match FirecrackerVM::spawn(&config) {
                Ok(vm) => {
                    if let Err(e) =
                        vm.wait_for_ssh_ready(Duration::from_secs(config.connect_timeout_secs))
                    {
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

    /// Async version of `replenish` that wraps blocking VM spawn in `spawn_blocking`.
    async fn replenish_async(&self, target: usize) -> SandboxResult<()> {
        let (needed, config) = {
            let inner = self.inner.lock().unwrap();
            let n = target.saturating_sub(inner.idle.len());
            let c = inner.config.clone();
            (n, c)
        };

        for _ in 0..needed {
            let cfg = config.clone();
            let vm = tokio::task::spawn_blocking(move || {
                let vm = FirecrackerVM::spawn(&cfg)?;
                vm.wait_for_ssh_ready(Duration::from_secs(cfg.connect_timeout_secs))?;
                Ok::<_, SandboxError>(vm)
            })
            .await
            .map_err(|e| {
                SandboxError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                ))
            })??;

            let mut inner = self.inner.lock().unwrap();
            inner.idle.push_back((vm, Instant::now()));
        }
        Ok(())
    }

    /// Remove idle VMs that have exceeded the idle timeout, returning them for cleanup.
    /// The caller is responsible for killing the returned VMs (e.g. via spawn_blocking).
    fn drain_expired(&self) -> Vec<FirecrackerVM> {
        let mut inner = self.inner.lock().unwrap();
        let timeout = inner.idle_timeout;
        let mut expired = Vec::new();
        loop {
            match inner.idle.front() {
                Some((_, since)) if since.elapsed() > timeout => {
                    let (vm, _) = inner.idle.pop_front().unwrap();
                    expired.push(vm);
                }
                _ => break,
            }
        }
        expired
    }

    /// Background loop: kill returned VMs, replenish idle, shrink stale.
    async fn maintenance_loop(&self) {
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;

            // Drain returned VMs and kill them in a blocking thread
            let returned: Vec<FirecrackerVM> = {
                let mut inner = self.inner.lock().unwrap();
                std::mem::take(&mut inner.returned).into_iter().collect()
            };
            for vm in returned {
                let _ = tokio::task::spawn_blocking(move || vm.kill()).await;
            }

            // Replenish if below pool_size
            let target = {
                let inner = self.inner.lock().unwrap();
                inner.pool_size
            };
            let idle_count = {
                let inner = self.inner.lock().unwrap();
                inner.idle.len()
            };
            if idle_count < target {
                let _ = self.replenish_async(target).await;
            }

            // Shrink stale idle VMs — kill expired VMs in a blocking thread
            let expired = self.drain_expired();
            for vm in expired {
                let _ = tokio::task::spawn_blocking(move || vm.kill()).await;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Part D: FirecrackerSandbox — impl Sandbox trait
// ---------------------------------------------------------------------------

/// Sandbox implementation backed by a Firecracker microVM.
///
/// All [`Sandbox`] trait methods delegate to the inner [`SSHSandbox`].
/// On drop, the VM is returned to the pool for asynchronous cleanup.
pub struct FirecrackerSandbox {
    name: String,
    pool: Arc<FirecrackerPool>,
    handle: StdMutex<Option<FirecrackerVmHandle>>,
    root_path: PathBuf,
}

impl FirecrackerSandbox {
    /// Create a new `FirecrackerSandbox`.
    ///
    /// The VM is not acquired until the first operation ([`start`](Sandbox::start)
    /// or one of the file/exec methods).
    pub fn new(name: String, root_path: PathBuf, pool: Arc<FirecrackerPool>) -> Self {
        Self {
            name,
            pool,
            handle: StdMutex::new(None),
            root_path,
        }
    }

    /// Lazily ensure we have a VM handle. Returns an `Arc` clone of the inner
    /// [`SSHSandbox`] so callers can borrow it without holding the mutex.
    async fn ssh(&self) -> SandboxResult<Arc<crate::ssh::SSHSandbox>> {
        {
            let guard = self.handle.lock().unwrap();
            if let Some(ref handle) = *guard {
                return Ok(handle.ssh.clone());
            }
        }
        // Lock released -- safe to do potentially-slow acquire
        let handle = self.pool.acquire()?;
        let mut guard = self.handle.lock().unwrap();
        if guard.is_none() {
            *guard = Some(handle);
        }
        Ok(guard.as_ref().unwrap().ssh.clone())
    }
}

impl Drop for FirecrackerSandbox {
    fn drop(&mut self) {
        let mut guard = self.handle.lock().unwrap();
        if let Some(handle) = guard.take() {
            self.pool.return_vm(handle.vm);
        }
    }
}

#[async_trait]
impl Sandbox for FirecrackerSandbox {
    fn kind(&self) -> &str {
        "firecracker"
    }

    fn name(&self) -> &str {
        &self.name
    }

    async fn start(&self) -> SandboxResult<()> {
        // Defer to first use. Just ensure connectivity works.
        let _ = self.ssh().await?;
        Ok(())
    }

    async fn cleanup(&self) -> SandboxResult<()> {
        // Handled by Drop
        Ok(())
    }

    fn root_path(&self) -> &Path {
        &self.root_path
    }

    fn resolve_path(&self, rel: &str) -> SandboxResult<PathBuf> {
        if rel.starts_with('/') {
            return Err(SandboxError::PathTraversal(rel.to_string()));
        }
        let resolved = self.root_path.join(rel);
        let normalized = crate::normalize_path(&resolved);
        let normalized_root = crate::normalize_path(&self.root_path);
        if !normalized.starts_with(&normalized_root) {
            return Err(SandboxError::PathTraversal(rel.to_string()));
        }
        Ok(normalized)
    }

    async fn execute(&self, req: crate::CommandRequest) -> SandboxResult<crate::CommandOutput> {
        self.ssh().await?.execute(req).await
    }

    async fn read_file(
        &self,
        path: &Path,
        offset: Option<u64>,
        limit: Option<u64>,
    ) -> SandboxResult<Vec<u8>> {
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
