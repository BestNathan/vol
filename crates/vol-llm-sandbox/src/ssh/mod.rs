//! SSH sandbox — routes all I/O to a remote host over SSH.
//!
//! Uses SSH channel multiplexing for concurrent command execution
//! and SFTP for file I/O. Maintains an idle-timeout connection.

use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;
use tracing::{debug, info};

use crate::registry::SshConfig;
use crate::{
    CommandOutput, DirEntry, FileMetadata, FileType, Sandbox, SandboxError, SandboxResult,
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
    name: String,
    root_path: PathBuf,
    remote_work_dir: String,
    session: Arc<session::SshSession>,
    last_activity: Arc<StdMutex<std::time::Instant>>,
    _idle_timeout: Duration,
    _idle_task: tokio::task::JoinHandle<()>,
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
            name: name.clone(),
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
                let elapsed = idle_task_last_activity.lock().unwrap().elapsed();
                if elapsed > idle_dur {
                    debug!(
                        idle_dur = ?idle_dur,
                        "SSH sandbox idle timeout reached, disconnecting"
                    );
                    let _ = session_clone.disconnect().await;
                    *idle_task_last_activity.lock().unwrap() = std::time::Instant::now();
                }
            }
        });

        Ok(Self {
            name,
            root_path: PathBuf::from(&remote_work_dir),
            remote_work_dir,
            session,
            last_activity,
            _idle_timeout: idle_timeout,
            _idle_task,
        })
    }

    /// Bump the activity timestamp so the idle guard doesn't fire.
    fn mark_active(&self) {
        if let Ok(mut guard) = self.last_activity.lock() {
            *guard = std::time::Instant::now();
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
    fn kind(&self) -> &str {
        "ssh"
    }

    fn name(&self) -> &str {
        &self.name
    }

    async fn start(&self) -> SandboxResult<()> {
        self.session.ensure().await?;

        // Ensure the remote work_dir exists
        let req = crate::CommandRequest {
            program: "mkdir".to_string(),
            args: vec!["-p".to_string(), self.remote_work_dir.clone()],
            env: Default::default(),
            cwd: None,
            stdin: None,
            timeout: Duration::from_secs(10),
        };

        let session = self.session.clone();
        tokio::task::spawn_blocking(move || session.execute_blocking(&req))
            .await
            .map_err(|e| SandboxError::Ssh(format!("spawn_blocking: {}", e)))?
            .map(|_| ())?;

        info!(
            "SSH sandbox '{}' ready at {}",
            self.name, self.remote_work_dir
        );
        Ok(())
    }

    async fn cleanup(&self) -> SandboxResult<()> {
        self._idle_task.abort();
        self.session.disconnect().await
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

    async fn execute(&self, req: crate::CommandRequest) -> SandboxResult<CommandOutput> {
        self.mark_active();
        self.session.ensure().await?;

        let session = self.session.clone();
        tokio::task::spawn_blocking(move || session.execute_blocking(&req))
            .await
            .map_err(|e| SandboxError::Ssh(format!("join error: {}", e)))?
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

        let limit = limit.unwrap_or(u64::MAX) as usize;
        let mut buf = Vec::new();
        let mut chunk = vec![0u8; 65536.min(limit)];

        loop {
            let n = file
                .read(&mut chunk)
                .map_err(|e| SandboxError::Ssh(e.to_string()))?;
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&chunk[..n]);
            if buf.len() >= limit {
                break;
            }
        }

        Ok(buf)
    }

    async fn write_file(&self, path: &Path, content: &[u8]) -> SandboxResult<()> {
        self.mark_active();
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
                let file_type = if stat.is_dir() {
                    FileType::Directory
                } else if stat.is_file() {
                    FileType::File
                } else {
                    FileType::Other
                };
                DirEntry { name, file_type }
            })
            .collect())
    }

    async fn metadata(&self, path: &Path) -> SandboxResult<FileMetadata> {
        self.mark_active();
        let sftp = self.session.sftp().await?;
        let remote_path = self.remote_path(path);

        let stat = sftp
            .stat(Path::new(&remote_path))
            .map_err(|e| SandboxError::Ssh(e.to_string()))?;

        let file_type = if stat.is_dir() {
            FileType::Directory
        } else if stat.is_file() {
            FileType::File
        } else {
            FileType::Other
        };

        Ok(FileMetadata {
            size: stat.size.unwrap_or(0),
            mtime: stat.mtime.unwrap_or(0) * 1000, // seconds → ms
            file_type,
        })
    }
}
