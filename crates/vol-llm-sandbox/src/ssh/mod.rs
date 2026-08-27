//! SSH sandbox — routes all I/O to a remote host over SSH.
//!
//! Uses SSH channel multiplexing for concurrent command execution
//! and SFTP for file I/O. Maintains an idle-timeout connection.

use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;
use tracing::debug;

use crate::registry::SshConfig;
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
    _idle_task: tokio::task::JoinHandle<()>,
    status: SandboxStatus,
}

impl SSHSandbox {
    /// Create a new SSH sandbox.
    ///
    /// `ssh_config` provides connection details; the session is lazily
    /// connected on first use (via [`start`](Sandbox::start)).
    pub fn new(
        name: String,
        work_dir: Option<String>,
        ssh_config: SshConfig,
    ) -> SandboxResult<Self> {
        let remote_work_dir = work_dir.unwrap_or_else(|| "/tmp/sandbox".to_string());
        let idle_timeout = Duration::from_secs(ssh_config.idle_timeout_secs);

        let config = Arc::new(session::SshSandboxConfig {
            name,
            work_dir: remote_work_dir.clone(),
            host: ssh_config.host,
            port: ssh_config.port,
            user: ssh_config.user,
            identity_file: ssh_config.identity_file,
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
        let idle_task_last_activity = Arc::clone(&last_activity);
        let session_clone = Arc::clone(&session);
        let idle_dur = idle_timeout;
        let _idle_task = tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(1)).await;
                let elapsed = idle_task_last_activity
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .elapsed();
                if elapsed > idle_dur {
                    debug!(
                        idle_dur = ?idle_dur,
                        "SSH sandbox idle timeout reached, disconnecting"
                    );
                    let _ = session_clone.disconnect().await;
                    *idle_task_last_activity
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner) =
                        std::time::Instant::now();
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
            _idle_task,
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
pub struct SSHSandboxProvider {
    configs: std::sync::Mutex<Vec<crate::registry::SshConfig>>,
}

impl SSHSandboxProvider {
    pub fn new() -> Self {
        Self {
            configs: std::sync::Mutex::new(Vec::new()),
        }
    }

    pub fn add_config(&self, config: crate::registry::SshConfig) {
        if let Ok(mut configs) = self.configs.lock() {
            configs.push(config);
        }
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
        let ssh_config =
            match &spec.config {
                crate::SandboxProviderConfig::Ssh { .. } => {
                    // For now, use the first registered config
                    let configs = self.configs.lock().map_err(|_| {
                        SandboxError::Config("Failed to lock SSH configs".to_string())
                    })?;
                    configs.first().cloned().ok_or_else(|| {
                        SandboxError::Config("No SSH config registered".to_string())
                    })?
                }
                _ => {
                    return Err(SandboxError::Config(
                        "SSHSandboxProvider requires SSH config".to_string(),
                    ))
                }
            };
        let work_dir = match &spec.config {
            crate::SandboxProviderConfig::Ssh { work_dir, .. } => {
                Some(work_dir.to_string_lossy().to_string())
            }
            _ => None,
        };
        let sandbox = std::sync::Arc::new(SSHSandbox::new(
            spec.name.clone(),
            work_dir,
            ssh_config.clone(),
        )?);
        let backend_id = format!(
            "{user}@{host}",
            user = ssh_config.user,
            host = ssh_config.host
        );
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
