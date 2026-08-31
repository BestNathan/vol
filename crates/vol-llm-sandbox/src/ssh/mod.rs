//! SSH sandbox — routes all I/O to a remote host over SSH.
//!
//! Uses SSH channel multiplexing for concurrent command execution
//! and SFTP for file I/O. Maintains an idle-timeout connection.

use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;
use tracing::debug;

use crate::spec::SshConfig;
use crate::{
    CommandOutput, DirEntry, FileMetadata, FileType, Sandbox, SandboxError, SandboxId,
    SandboxResult, SandboxStatus,
};
use std::io::{Read, Seek, Write};

pub mod session;

// Re-export for registry / config building
pub use self::session::SshSandboxConfig;

/// SSH-backed sandbox implementation.
///
/// Manages a single SSH session per sandbox instance. Commands are
/// executed via `channel_session` / `exec` and file I/O uses SFTP.
/// An idle-timeout background task disconnects the session when no
/// activity has occurred within the configured window.
pub struct SSHSandbox {
    id: SandboxId,
    root_path: PathBuf,
    remote_work_dir: String,
    session: Arc<session::SshSession>,
    last_activity: Arc<StdMutex<std::time::Instant>>,
    _idle_timeout: Duration,
    idle_task: tokio::task::JoinHandle<()>,
    status: SandboxStatus,
}

impl Drop for SSHSandbox {
    fn drop(&mut self) {
        // Dropping a `JoinHandle` does NOT stop the task. Without this abort
        // the idle loop runs for the rest of the process lifetime holding its
        // own `Arc<SshSession>` clone, so a sandbox that has been evicted from
        // the manager's cache or destroyed never releases its SSH connection.
        self.idle_task.abort();
    }
}

impl SSHSandbox {
    /// Create a new SSH sandbox.
    ///
    /// `ssh_config` provides connection details; the session is lazily
    /// connected on first use (via [`start`](Sandbox::start)).
    pub fn new(name: String, ssh_config: SshConfig) -> SandboxResult<Self> {
        let remote_work_dir = ssh_config.work_dir.to_string_lossy().to_string();
        let idle_timeout = Duration::from_secs(ssh_config.idle_timeout_secs);

        let config = Arc::new(session::SshSandboxConfig {
            name,
            work_dir: remote_work_dir.clone(),
            host: ssh_config.host,
            port: ssh_config.port,
            user: ssh_config.user,
            key_path: ssh_config.key_path,
            passphrase: ssh_config.passphrase,
            known_hosts_file: ssh_config.known_hosts_file,
            host_key: ssh_config.host_key,
            idle_timeout_secs: ssh_config.idle_timeout_secs,
            connect_timeout_secs: ssh_config.connect_timeout_secs,
        });

        let session = Arc::new(session::SshSession::new(config));
        let last_activity = Arc::new(StdMutex::new(std::time::Instant::now()));

        // Background idle timeout task — shares the same `last_activity`
        // state so that every file / command operation resets the timer.
        //
        // Sleeps until the deadline rather than polling: at the default
        // 300s timeout this wakes roughly twice per window instead of 300
        // times. Aborted by `Drop`.
        let idle_task_last_activity = Arc::clone(&last_activity);
        let session_clone = Arc::clone(&session);
        let idle_dur = idle_timeout;
        let idle_task = tokio::spawn(async move {
            // Floor for the post-disconnect sleep so a configured
            // `idle_timeout_secs = 0` cannot spin. Matches the previous
            // behavior, which slept 1s per iteration unconditionally.
            const FLOOR: Duration = Duration::from_secs(1);
            loop {
                let elapsed = idle_task_last_activity
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .elapsed();

                match idle_dur.checked_sub(elapsed) {
                    // Still inside the window. Sleep the remainder, then
                    // re-check: activity during the sleep pushes the
                    // deadline out and we simply wait again.
                    Some(remaining) if !remaining.is_zero() => {
                        tokio::time::sleep(remaining).await;
                    }
                    // Idle for at least `idle_dur`.
                    _ => {
                        debug!(
                            idle_dur = ?idle_dur,
                            "SSH sandbox idle timeout reached, disconnecting"
                        );
                        let _ = session_clone.disconnect().await;
                        *idle_task_last_activity
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner) =
                            std::time::Instant::now();
                        tokio::time::sleep(FLOOR).await;
                    }
                }
            }
        });

        Ok(Self {
            id: SandboxId::new(),
            root_path: PathBuf::from(&remote_work_dir),
            remote_work_dir,
            session,
            last_activity,
            _idle_timeout: idle_timeout,
            idle_task,
            status: SandboxStatus::Running,
        })
    }

    /// Bump the activity timestamp so the idle guard doesn't fire.
    fn mark_active(&self) {
        if let Ok(mut guard) = self.last_activity.lock() {
            *guard = std::time::Instant::now();
        }
    }

    /// Map ssh2::FileStat perm bits to our FileType (detects symlinks).
    /// SFTP perm field uses Unix mode bits — S_IFLNK = 0o120000.
    fn file_type_from_stat(stat: &ssh2::FileStat) -> FileType {
        const S_IFMT: u32 = 0o170000;
        const S_IFLNK: u32 = 0o120000;
        // Check for symlink via Unix permission bits first
        if let Some(perm) = stat.perm {
            if perm & S_IFMT == S_IFLNK {
                return FileType::Symlink;
            }
        }
        if stat.is_dir() {
            FileType::Directory
        } else if stat.is_file() {
            FileType::File
        } else {
            FileType::Other
        }
    }

    /// Resolve a local filesystem path to a remote absolute path.
    /// Relative paths are appended to `remote_work_dir`.
    fn remote_path(&self, path: &Path) -> String {
        if path.is_absolute() {
            path.to_string_lossy().to_string()
        } else {
            PathBuf::from(&self.remote_work_dir)
                .join(path)
                .to_string_lossy()
                .to_string()
        }
    }
}

#[async_trait]
impl Sandbox for SSHSandbox {
    fn id(&self) -> &SandboxId {
        &self.id
    }

    fn kind(&self) -> &str {
        "ssh"
    }

    fn status(&self) -> SandboxStatus {
        self.status
    }

    fn root_path(&self) -> Option<&Path> {
        Some(&self.root_path)
    }

    fn resolve_path(&self, rel: &str) -> SandboxResult<PathBuf> {
        if rel.starts_with('/') || rel.starts_with('~') {
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

    async fn execute(&self, req: crate::CommandRequest) -> SandboxResult<CommandOutput> {
        self.mark_active();
        self.session.ensure().await?;

        let session = self.session.clone();
        tokio::task::spawn_blocking(move || session.execute_blocking(&req))
            .await
            .map_err(|e| SandboxError::Ssh(format!("join error: {e}")))?
    }

    async fn read_file(
        &self,
        path: &Path,
        offset: Option<u64>,
        limit: Option<u64>,
    ) -> SandboxResult<Vec<u8>> {
        self.mark_active();
        let sftp = self.session.sftp().await?;
        let remote_path = self.remote_path(path);

        let mut file = sftp
            .open(Path::new(&remote_path))
            .map_err(|e| SandboxError::Ssh(e.to_string()))?;

        if let Some(offset) = offset {
            file.seek(std::io::SeekFrom::Start(offset))
                .map_err(|e| SandboxError::Ssh(e.to_string()))?;
        }

        let limit = usize::try_from(limit.unwrap_or(u64::MAX)).unwrap_or(usize::MAX);
        let mut buf = Vec::new();
        let mut chunk = vec![0u8; 65536.min(limit)];

        loop {
            let n = file
                .read(&mut chunk)
                .map_err(|e| SandboxError::Ssh(e.to_string()))?;
            if n == 0 {
                break;
            }
            let slice = chunk.get(..n).unwrap_or(&[]);
            buf.extend_from_slice(slice);
            if buf.len() >= limit {
                break;
            }
        }

        Ok(buf)
    }

    async fn write_file(&self, path: &Path, content: &[u8]) -> SandboxResult<()> {
        self.mark_active();
        // Ensure parent directories exist (consistent with LocalSandbox and WasmSandbox)
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                self.create_dir_all(parent).await?;
            }
        }
        let sftp = self.session.sftp().await?;
        let remote_path = self.remote_path(path);

        let mut file = sftp
            .create(Path::new(&remote_path))
            .map_err(|e| SandboxError::Ssh(e.to_string()))?;
        file.write_all(content)
            .map_err(|e| SandboxError::Ssh(e.to_string()))?;
        Ok(())
    }

    async fn create_dir_all(&self, path: &Path) -> SandboxResult<()> {
        self.mark_active();
        let sftp = self.session.sftp().await?;
        let remote_path = self.remote_path(path);

        let clean = remote_path.trim_start_matches('/');
        let mut current = PathBuf::from("/");

        for component in clean.split('/') {
            if component.is_empty() {
                continue;
            }
            current = current.join(component);
            match sftp.mkdir(&current, 0o755) {
                Ok(_) => {}
                Err(_) => {
                    // Directory may already exist — verify by stat-ing
                    sftp.stat(&current).map_err(|e| {
                        SandboxError::Ssh(format!("mkdir {}: {}", current.display(), e))
                    })?;
                }
            }
        }

        Ok(())
    }

    async fn read_dir(&self, path: &Path) -> SandboxResult<Vec<DirEntry>> {
        self.mark_active();
        let sftp = self.session.sftp().await?;
        let remote_path = self.remote_path(path);

        let entries = sftp
            .readdir(Path::new(&remote_path))
            .map_err(|e| SandboxError::Ssh(e.to_string()))?;

        Ok(entries
            .into_iter()
            .map(|(p, stat)| {
                let name = p
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let file_type = Self::file_type_from_stat(&stat);
                DirEntry { name, file_type }
            })
            .collect())
    }

    async fn metadata(&self, path: &Path) -> SandboxResult<FileMetadata> {
        self.mark_active();
        let sftp = self.session.sftp().await?;
        let remote_path = self.remote_path(path);

        // Use lstat to avoid following symlinks (consistent with LocalSandbox)
        let stat = sftp
            .lstat(Path::new(&remote_path))
            .map_err(|e| SandboxError::Ssh(e.to_string()))?;

        let file_type = Self::file_type_from_stat(&stat);

        Ok(FileMetadata {
            size: stat.size.unwrap_or(0),
            mtime: stat.mtime.unwrap_or(0) * 1000, // seconds → ms
            file_type,
        })
    }
}

/// Provider for SSHSandbox instances.
///
/// Creates sandboxes directly from the spec's SSH fields — no out-of-band
/// config registration needed.
pub struct SSHSandboxProvider;

impl SSHSandboxProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SSHSandboxProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl crate::SandboxProvider for SSHSandboxProvider {
    fn kind(&self) -> &str {
        "ssh"
    }

    fn capabilities(&self) -> crate::SandboxCapabilities {
        crate::SandboxCapabilities {
            persistent: true,
            pausable: false,
            stoppable: false,
            destroyable: false,
        }
    }

    async fn create(
        &self,
        spec: &crate::SandboxSpec,
    ) -> crate::SandboxResult<crate::BackendSandboxRef> {
        let ssh_config = spec.config.as_ssh().ok_or_else(|| {
            SandboxError::Config(format!(
                "SSHSandboxProvider requires SSH config in spec '{}'",
                spec.name
            ))
        })?;
        let backend_id = format!(
            "{user}@{host}",
            user = ssh_config.user,
            host = ssh_config.host
        );
        let sandbox = std::sync::Arc::new(SSHSandbox::new(spec.name.clone(), ssh_config)?);
        Ok(crate::BackendSandboxRef {
            backend_id,
            sandbox,
        })
    }

    async fn get(&self, backend_id: &str) -> crate::SandboxResult<std::sync::Arc<dyn Sandbox>> {
        Err(SandboxError::NotFound(format!(
            "SSHSandbox '{backend_id}' not found in cache"
        )))
    }

    async fn list(&self) -> crate::SandboxResult<Vec<crate::SandboxInfo>> {
        Ok(vec![])
    }

    async fn start(&self, _backend_id: &str) -> crate::SandboxResult<()> {
        Ok(())
    }

    async fn pause(&self, _backend_id: &str) -> crate::SandboxResult<()> {
        Err(SandboxError::Config(
            "SSHSandbox does not support pause".to_string(),
        ))
    }

    async fn resume(&self, _backend_id: &str) -> crate::SandboxResult<()> {
        Err(SandboxError::Config(
            "SSHSandbox does not support resume".to_string(),
        ))
    }

    async fn stop(&self, _backend_id: &str) -> crate::SandboxResult<()> {
        Ok(())
    }

    async fn destroy(&self, _backend_id: &str) -> crate::SandboxResult<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> SshConfig {
        // `SSHSandbox::new` does not connect — the session is established
        // lazily on first use — so these tests need no reachable host.
        SshConfig {
            host: "127.0.0.1".to_string(),
            user: "nobody".to_string(),
            work_dir: PathBuf::from("/tmp/ssh-sandbox-test"),
            port: 22,
            key_path: None,
            passphrase: None,
            known_hosts_file: None,
            host_key: None,
            idle_timeout_secs: 300,
            connect_timeout_secs: 1,
        }
    }

    /// Regression: the idle task used to outlive the sandbox. It is spawned in
    /// `new()` and holds its own `Arc<SshSession>` clone; dropping a
    /// `JoinHandle` does not stop a tokio task, so without the `Drop` impl the
    /// session stayed alive for the rest of the process even after the sandbox
    /// was evicted from the manager's cache or destroyed.
    #[tokio::test]
    async fn dropping_sandbox_aborts_idle_task_and_releases_session() {
        let sandbox = SSHSandbox::new("drop-test".to_string(), test_config()).unwrap();
        let session = Arc::downgrade(&sandbox.session);
        assert!(
            session.upgrade().is_some(),
            "session should be alive while the sandbox is"
        );

        drop(sandbox);

        // `abort()` is not synchronous: the task is scheduled for cancellation
        // and releases its Arc when the runtime drops the future.
        for _ in 0..100 {
            if session.upgrade().is_none() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        assert!(
            session.upgrade().is_none(),
            "idle task must be aborted on drop so the SSH session is released"
        );
    }

    #[tokio::test]
    async fn idle_task_is_aborted_not_merely_detached() {
        let sandbox = SSHSandbox::new("abort-test".to_string(), test_config()).unwrap();
        let handle = sandbox.idle_task.abort_handle();
        assert!(!handle.is_finished(), "task should be running");

        drop(sandbox);

        for _ in 0..100 {
            if handle.is_finished() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(handle.is_finished(), "idle task must terminate on drop");
    }

    /// The idle task must not disconnect before the window elapses. With the
    /// old 1s polling loop this was implicit; with sleep-to-deadline it is
    /// worth pinning that a short-timeout sandbox still survives briefly.
    #[tokio::test]
    async fn idle_task_does_not_fire_before_deadline() {
        let mut cfg = test_config();
        cfg.idle_timeout_secs = 60;
        let sandbox = SSHSandbox::new("deadline-test".to_string(), cfg).unwrap();

        tokio::time::sleep(Duration::from_millis(50)).await;

        assert!(
            !sandbox.idle_task.is_finished(),
            "idle task should still be waiting well inside the window"
        );
        assert_eq!(sandbox.kind(), "ssh");
    }
}
