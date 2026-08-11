//! TmpSandbox — per-agent sandbox rooted at `/tmp/{agent_id}/`.
//!
//! On construction the sandbox uses a placeholder path `/tmp/tmp/`.
//! Call [`bind_metadata`](Sandbox::bind_metadata) with an `"agent_id"`
//! entry to set the real path before [`start`](Sandbox::start).

use crate::{
    CommandOutput, DirEntry, FileMetadata, FileType, Sandbox, SandboxError, SandboxResult,
};
use async_trait::async_trait;
use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A sandbox that maps to `/tmp/{name}/`.
///
/// # Lifecycle
///
/// ```ignore
/// let sb = TmpSandbox::default();              // root = /tmp/tmp/
/// sb.bind_metadata(&[("agent_id", "my-agent")]); // root = /tmp/my-agent/
/// sb.start().await?;                           // creates /tmp/my-agent/
/// // ... use ...
/// sb.cleanup().await?;                         // removes /tmp/my-agent/
/// ```
pub struct TmpSandbox {
    /// Default identity — used until bind_metadata sets a real name.
    default_name: String,
    // SAFETY: written once by bind_metadata (before any concurrent access),
    // then read-only. UnsafeCell is needed for interior mutability with &self
    // while staying Sync (unlike RefCell/Cell).
    bound_name: UnsafeCell<Option<&'static str>>,

    /// Default root — used until bind_metadata sets a real path.
    default_root: PathBuf,
    // SAFETY: same contract as bound_name.
    bound_root: UnsafeCell<Option<&'static Path>>,
}

// Safety: TmpSandbox is Sync because bind_metadata is called once before any
// concurrent access, and after that the UnsafeCell contents are immutable.
// The unsafe mutations in bind_metadata happen-before all reads.
unsafe impl Sync for TmpSandbox {}

impl Default for TmpSandbox {
    fn default() -> Self {
        Self::with_default("tmp")
    }
}

impl TmpSandbox {
    /// Create a TmpSandbox with an explicit default name.
    pub fn with_default(name: &str) -> Self {
        let root = PathBuf::from("/tmp").join(name);
        Self {
            default_name: name.to_string(),
            bound_name: UnsafeCell::new(None),
            default_root: root,
            bound_root: UnsafeCell::new(None),
        }
    }

    /// The effective name (bound if set, otherwise default).
    fn effective_name(&self) -> &str {
        // SAFETY: after bind_metadata, bound_name is immutable.
        // Before bind_metadata, it's None and we return the default.
        unsafe { (*self.bound_name.get()).unwrap_or(&self.default_name) }
    }

    /// The effective root path (bound if set, otherwise default).
    fn effective_root(&self) -> &Path {
        // SAFETY: after bind_metadata, bound_root is immutable.
        unsafe { (*self.bound_root.get()).unwrap_or(&self.default_root) }
    }

    /// Leak a string and return a static reference.
    fn leak_str(s: String) -> &'static str {
        Box::leak(s.into_boxed_str())
    }

    /// Leak a PathBuf and return a static reference.
    fn leak_path(p: PathBuf) -> &'static Path {
        Box::leak(p.into_boxed_path())
    }
}

#[async_trait]
impl Sandbox for TmpSandbox {
    fn kind(&self) -> &str {
        "tmp"
    }

    fn name(&self) -> &str {
        self.effective_name()
    }

    fn bind_metadata(&self, metadata: &HashMap<String, String>) {
        if let Some(agent_id) = metadata.get("agent_id") {
            let name = Self::leak_str(agent_id.clone());
            let root = Self::leak_path(PathBuf::from("/tmp").join(agent_id));
            // SAFETY: bind_metadata is called once before any concurrent use.
            // After this point, bound_name and bound_root are immutable.
            unsafe {
                *self.bound_name.get() = Some(name);
                *self.bound_root.get() = Some(root);
            }
        }
    }

    async fn start(&self) -> SandboxResult<()> {
        std::fs::create_dir_all(self.effective_root()).map_err(SandboxError::Io)
    }

    async fn cleanup(&self) -> SandboxResult<()> {
        let root = self.effective_root();
        if root.exists() {
            std::fs::remove_dir_all(root).map_err(SandboxError::Io)?;
        }
        Ok(())
    }

    fn root_path(&self) -> &Path {
        self.effective_root()
    }

    fn resolve_path(&self, rel: &str) -> SandboxResult<PathBuf> {
        if rel.starts_with('/') || rel.starts_with('~') {
            return Err(SandboxError::PathTraversal(rel.to_string()));
        }
        let resolved = self.effective_root().join(rel);
        let normalized = crate::normalize_path(&resolved);
        let normalized_root = crate::normalize_path(self.effective_root());
        if !normalized.starts_with(&normalized_root) {
            return Err(SandboxError::PathTraversal(rel.to_string()));
        }
        Ok(normalized)
    }

    async fn execute(&self, req: crate::CommandRequest) -> SandboxResult<CommandOutput> {
        let root = self.effective_root().to_path_buf();
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

    fn agent_metadata(id: &str) -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("agent_id".to_string(), id.to_string());
        m
    }

    async fn setup(id: &str) -> TmpSandbox {
        let sb = TmpSandbox::default();
        sb.bind_metadata(&agent_metadata(id));
        // Clean up from previous runs
        let root = sb.effective_root().to_path_buf();
        let _ = std::fs::remove_dir_all(&root);
        sb.start().await.unwrap();
        sb
    }

    #[tokio::test]
    async fn test_default_without_bind() {
        let sb = TmpSandbox::default();
        assert_eq!(sb.kind(), "tmp");
        assert_eq!(sb.name(), "tmp");
        assert_eq!(sb.root_path(), Path::new("/tmp/tmp"));
    }

    #[tokio::test]
    async fn test_bind_sets_agent_path() {
        let sb = TmpSandbox::default();
        sb.bind_metadata(&agent_metadata("my-agent"));
        assert_eq!(sb.name(), "my-agent");
        assert_eq!(sb.root_path(), Path::new("/tmp/my-agent"));
    }

    #[tokio::test]
    async fn test_bind_ignores_unknown_keys() {
        let sb = TmpSandbox::default();
        let mut m = HashMap::new();
        m.insert("other_key".to_string(), "value".to_string());
        sb.bind_metadata(&m);
        // Still uses default
        assert_eq!(sb.name(), "tmp");
    }

    #[tokio::test]
    async fn test_full_lifecycle() {
        let sb = TmpSandbox::default();
        sb.bind_metadata(&agent_metadata("lifecycle-test"));
        assert_eq!(sb.name(), "lifecycle-test");

        // Start
        sb.start().await.unwrap();
        assert!(sb.root_path().exists());

        // Write + read
        let file = sb.root_path().join("hello.txt");
        sb.write_file(&file, b"hello tmp").await.unwrap();
        let content = sb.read_file(&file, None, None).await.unwrap();
        assert_eq!(content, b"hello tmp");

        // Cleanup
        sb.cleanup().await.unwrap();
        assert!(!sb.root_path().exists());
    }

    #[tokio::test]
    async fn test_resolve_path_rejects_absolute() {
        let sb = TmpSandbox::default();
        sb.bind_metadata(&agent_metadata("resolver"));
        assert!(sb.resolve_path("/etc/passwd").is_err());
        assert!(sb.resolve_path("~/foo").is_err());
        let resolved = sb.resolve_path("sub/file.txt").unwrap();
        assert!(resolved.starts_with("/tmp/resolver"));
        assert!(resolved.ends_with("sub/file.txt"));
    }

    #[tokio::test]
    async fn test_execute_echo() {
        let sb = setup("exec-echo").await;
        let req = CommandRequest {
            program: "echo".to_string(),
            args: vec!["-n".to_string(), "hello from tmp".to_string()],
            env: Default::default(),
            cwd: None,
            stdin: None,
            timeout: Duration::from_secs(5),
        };
        let output = sb.execute(req).await.unwrap();
        assert_eq!(output.exit_code, 0);
        assert_eq!(String::from_utf8_lossy(&output.stdout), "hello from tmp");
        sb.cleanup().await.unwrap();
    }
}
