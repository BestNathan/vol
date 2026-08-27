//! TmpSandbox — per-sandbox temp directory at /tmp/{sub_dir}/.
//!
//! Creates a temporary directory for sandbox execution. The directory is created
//! on construction and can be cleaned up by the provider.

use crate::{
    CommandOutput, DirEntry, FileMetadata, FileType, Sandbox, SandboxError, SandboxId,
    SandboxResult, SandboxStatus,
};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn random_subdir() -> String {
    let pid = std::process::id();
    let count = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("sandbox_{pid:x}_{count:x}")
}

/// A sandbox that maps to `/tmp/{sub_dir}/`.
///
/// Creates a temporary directory at construction time. The directory is not
/// automatically cleaned up — the provider is responsible for calling cleanup.
pub struct TmpSandbox {
    id: SandboxId,
    root_path: PathBuf,
    status: SandboxStatus,
}

impl Default for TmpSandbox {
    fn default() -> Self {
        Self::new()
    }
}

impl TmpSandbox {
    /// Create a TmpSandbox with a random subdirectory name.
    pub fn new() -> Self {
        let sub_dir = random_subdir();
        let root_path = PathBuf::from("/tmp").join(sub_dir);
        Self {
            id: SandboxId::new(),
            root_path,
            status: SandboxStatus::Running,
        }
    }

    /// Create a TmpSandbox with a specific subdirectory name.
    pub fn with_sub_dir(sub_dir: &str) -> Self {
        let root_path = PathBuf::from("/tmp").join(sub_dir);
        Self {
            id: SandboxId::new(),
            root_path,
            status: SandboxStatus::Running,
        }
    }

    /// Clean up the sandbox directory.
    pub fn cleanup(&self) -> SandboxResult<()> {
        if self.root_path.exists() {
            std::fs::remove_dir_all(&self.root_path).map_err(SandboxError::Io)?;
        }
        Ok(())
    }
}

#[async_trait]
impl Sandbox for TmpSandbox {
    fn id(&self) -> &SandboxId {
        &self.id
    }

    fn kind(&self) -> &str {
        "tmp"
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
        let root = self.root_path.clone();
        tokio::task::spawn_blocking(move || {
            let mut cmd = std::process::Command::new(&req.program);
            cmd.args(&req.args);
            for (k, v) in &req.env {
                cmd.env(k, v);
            }
            let cwd = req
                .cwd
                .map(|p| root.join(p))
                .unwrap_or_else(|| root.clone());
            let _ = std::fs::create_dir_all(&cwd);
            cmd.current_dir(&cwd);
            cmd.stdin(std::process::Stdio::piped());
            cmd.stdout(std::process::Stdio::piped());
            cmd.stderr(std::process::Stdio::piped());

            #[cfg(unix)]
            {
                use std::os::unix::process::CommandExt;
                cmd.process_group(0);
            }

            let mut child = cmd.spawn().map_err(SandboxError::Io)?;
            drop(child.stdin.take());

            let output = child.wait_with_output().map_err(SandboxError::Io)?;
            #[cfg(unix)]
            let killed_by_signal = {
                use std::os::unix::process::ExitStatusExt;
                output.status.signal()
            };
            #[cfg(not(unix))]
            let killed_by_signal = None;

            Ok(CommandOutput {
                stdout: output.stdout,
                stderr: output.stderr,
                exit_code: output.status.code().unwrap_or(-1),
                killed_by_signal,
            })
        })
        .await
        .map_err(|e| SandboxError::Io(std::io::Error::other(e.to_string())))?
    }

    async fn read_file(
        &self,
        path: &Path,
        offset: Option<u64>,
        limit: Option<u64>,
    ) -> SandboxResult<Vec<u8>> {
        let content = std::fs::read(path).map_err(SandboxError::Io)?;
        let start = usize::try_from(offset.unwrap_or(0)).unwrap_or(usize::MAX);
        let end = limit
            .and_then(|l| usize::try_from(l).ok().map(|l| start.saturating_add(l)))
            .unwrap_or(content.len());
        let end = end.min(content.len());
        let slice = content.get(start..end).unwrap_or(&[]);
        Ok(slice.to_vec())
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
            .filter_map(std::result::Result::ok)
            .map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                let file_type = e
                    .file_type()
                    .map(|ft| {
                        if ft.is_dir() {
                            FileType::Directory
                        } else if ft.is_file() {
                            FileType::File
                        } else if ft.is_symlink() {
                            FileType::Symlink
                        } else {
                            FileType::Other
                        }
                    })
                    .unwrap_or(FileType::Other);
                DirEntry { name, file_type }
            })
            .collect();
        Ok(entries)
    }

    async fn metadata(&self, path: &Path) -> SandboxResult<FileMetadata> {
        let meta = std::fs::metadata(path).map_err(SandboxError::Io)?;
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .and_then(|d| u64::try_from(d.as_millis()).ok())
            .unwrap_or(0);
        let file_type = if meta.is_dir() {
            FileType::Directory
        } else if meta.is_file() {
            FileType::File
        } else if meta.is_symlink() {
            FileType::Symlink
        } else {
            FileType::Other
        };
        Ok(FileMetadata {
            size: meta.len(),
            mtime,
            file_type,
        })
    }
}
