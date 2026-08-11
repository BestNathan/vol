//! TmpSandbox — per-sandbox temp directory at /tmp/{sub_dir}/.
//!
//! On construction, generates a random subdirectory name. Call
//! [`bind_metadata`](Sandbox::bind_metadata) with `"sub_dir"` to set
//! a custom name (e.g. the agent name for debugging) before [`start`](Sandbox::start).

use async_trait::async_trait;
use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{
    CommandOutput, DirEntry, FileMetadata, FileType, Sandbox, SandboxError, SandboxResult,
};

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn random_subdir() -> String {
    let pid = std::process::id();
    let count = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("sandbox_{pid:x}_{count:x}")
}

/// A sandbox that maps to `/tmp/{sub_dir}/`.
///
/// # Lifecycle
///
/// ```ignore
/// let sb = TmpSandbox::new();                    // root = /tmp/sandbox_3a2f_0/
/// sb.bind_metadata(&[("sub_dir", "explore")]);   // root = /tmp/explore/
/// sb.start().await?;                             // creates /tmp/explore/
/// // ... use ...
/// sb.cleanup().await?;                           // removes /tmp/explore/
/// ```
pub struct TmpSandbox {
    /// Default random subdir — used until bind_metadata sets a real one.
    default_sub_dir: String,
    // SAFETY: written once by bind_metadata (before any concurrent access),
    // then read-only. UnsafeCell needed for interior mutability with &self.
    bound_sub_dir: UnsafeCell<Option<&'static str>>,
}

// Safety: bind_metadata is called once before any concurrent access.
// After that, UnsafeCell contents are immutable.
unsafe impl Sync for TmpSandbox {}

impl Default for TmpSandbox {
    fn default() -> Self {
        Self::new()
    }
}

impl TmpSandbox {
    /// Create a TmpSandbox with a random subdirectory name.
    pub fn new() -> Self {
        Self {
            default_sub_dir: random_subdir(),
            bound_sub_dir: UnsafeCell::new(None),
        }
    }

    /// The effective subdirectory name (bound if set, otherwise default random).
    fn effective_sub_dir(&self) -> &str {
        unsafe { (*self.bound_sub_dir.get()).unwrap_or(&self.default_sub_dir) }
    }

    /// The effective root path.
    fn effective_root(&self) -> PathBuf {
        PathBuf::from("/tmp").join(self.effective_sub_dir())
    }

    fn leak_str(s: String) -> &'static str {
        Box::leak(s.into_boxed_str())
    }
}

#[async_trait]
impl Sandbox for TmpSandbox {
    fn kind(&self) -> &str {
        "tmp"
    }

    fn name(&self) -> &str {
        self.effective_sub_dir()
    }

    fn bind_metadata(&self, metadata: &HashMap<String, String>) {
        if let Some(sub_dir) = metadata.get("sub_dir") {
            // SAFETY: bind_metadata is called once before any concurrent use.
            unsafe {
                *self.bound_sub_dir.get() = Some(Self::leak_str(sub_dir.clone()));
            }
        }
    }

    async fn start(&self) -> SandboxResult<()> {
        std::fs::create_dir_all(self.effective_root()).map_err(SandboxError::Io)
    }

    async fn cleanup(&self) -> SandboxResult<()> {
        let root = self.effective_root();
        if root.exists() {
            std::fs::remove_dir_all(&root).map_err(SandboxError::Io)?;
        }
        Ok(())
    }

    fn root_path(&self) -> &Path {
        // Boxing trick: leak the effective root so we can return &Path.
        // The leaked path lives as long as the sandbox (bounded lifetime).
        let boxed: Box<Path> = self.effective_root().into_boxed_path();
        Box::leak(boxed)
    }

    fn resolve_path(&self, rel: &str) -> SandboxResult<PathBuf> {
        if rel.starts_with('/') || rel.starts_with('~') {
            return Err(SandboxError::PathTraversal(rel.to_string()));
        }
        let root = self.effective_root();
        let resolved = root.join(rel);
        let normalized = crate::normalize_path(&resolved);
        let normalized_root = crate::normalize_path(&root);
        if !normalized.starts_with(&normalized_root) {
            return Err(SandboxError::PathTraversal(rel.to_string()));
        }
        Ok(normalized)
    }

    async fn execute(&self, req: crate::CommandRequest) -> SandboxResult<CommandOutput> {
        let root = self.effective_root();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CommandRequest, Sandbox};
    use std::collections::HashMap;
    use std::time::Duration;

    fn metadata(sub_dir: &str) -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("sub_dir".to_string(), sub_dir.to_string());
        m
    }

    async fn setup(sub_dir: &str) -> TmpSandbox {
        let sb = TmpSandbox::new();
        sb.bind_metadata(&metadata(sub_dir));
        let root = sb.effective_root();
        let _ = std::fs::remove_dir_all(&root);
        sb.start().await.unwrap();
        sb
    }

    #[tokio::test]
    async fn test_random_subdir_on_construction() {
        let sb = TmpSandbox::new();
        assert_eq!(sb.kind(), "tmp");
        // Default subdir is random, should not be empty
        assert!(!sb.name().is_empty());
        assert!(sb.root_path().starts_with("/tmp/"));
    }

    #[tokio::test]
    async fn test_bind_metadata_sets_sub_dir() {
        let sb = TmpSandbox::new();
        sb.bind_metadata(&metadata("explore"));
        assert!(sb.name().contains("explore"));
        assert!(sb.root_path().to_string_lossy().contains("explore"));
    }

    #[tokio::test]
    async fn test_bind_ignores_unknown_keys() {
        let sb = TmpSandbox::new();
        let original = sb.name().to_string();
        let mut m = HashMap::new();
        m.insert("other".to_string(), "value".to_string());
        sb.bind_metadata(&m);
        assert_eq!(sb.name(), original);
    }

    #[tokio::test]
    async fn test_full_lifecycle() {
        let sb = setup("lifecycle-test").await;
        assert!(sb.root_path().exists());

        let file = sb.root_path().join("hello.txt");
        sb.write_file(&file, b"hello").await.unwrap();
        let content = sb.read_file(&file, None, None).await.unwrap();
        assert_eq!(content, b"hello");

        sb.cleanup().await.unwrap();
        assert!(!sb.root_path().exists());
    }

    #[tokio::test]
    async fn test_resolve_path() {
        let sb = setup("resolver").await;
        assert!(sb.resolve_path("/etc/passwd").is_err());
        let resolved = sb.resolve_path("sub/file.txt").unwrap();
        assert!(resolved.starts_with("/tmp/resolver"));
    }

    #[tokio::test]
    async fn test_execute_echo() {
        let sb = setup("exec-echo").await;
        let req = CommandRequest {
            program: "echo".to_string(),
            args: vec!["-n".to_string(), "hello".to_string()],
            env: Default::default(),
            cwd: None,
            stdin: None,
            timeout: Duration::from_secs(5),
        };
        let output = sb.execute(req).await.unwrap();
        assert_eq!(output.exit_code, 0);
        assert_eq!(String::from_utf8_lossy(&output.stdout), "hello");
        sb.cleanup().await.unwrap();
    }
}
