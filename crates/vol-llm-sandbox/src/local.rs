use crate::{
    CommandOutput, DirEntry, FileMetadata, FileType, Sandbox, SandboxError, SandboxResult,
};
use async_trait::async_trait;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Counter to guarantee unique temp directory names across parallel tests.
static SANDBOX_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A sandbox using a local directory as its root.
///
/// If created with `Some(path)`, the directory is caller-owned and NOT deleted on cleanup.
/// If created with `None`, a temp directory is created and IS deleted on cleanup.
pub struct LocalSandbox {
    root_path: PathBuf,
    is_temp: bool,
}

impl LocalSandbox {
    pub fn new(path: Option<PathBuf>) -> Self {
        let (root_path, is_temp) = match path {
            Some(p) => (p, false),
            None => {
                let pid = std::process::id();
                let count = SANDBOX_COUNTER.fetch_add(1, Ordering::Relaxed);
                let temp = std::env::temp_dir().join(format!("sandbox_{pid:x}_{count:x}"));
                (temp, true)
            }
        };
        Self { root_path, is_temp }
    }
}

impl Drop for LocalSandbox {
    fn drop(&mut self) {
        if self.is_temp && self.root_path.exists() {
            let _ = std::fs::remove_dir_all(&self.root_path);
        }
    }
}

#[async_trait]
impl Sandbox for LocalSandbox {
    fn kind(&self) -> &str {
        "local"
    }

    fn name(&self) -> &str {
        "local"
    }

    async fn start(&self) -> SandboxResult<()> {
        std::fs::create_dir_all(&self.root_path).map_err(SandboxError::Io)
    }

    async fn cleanup(&self) -> SandboxResult<()> {
        if self.is_temp {
            std::fs::remove_dir_all(&self.root_path).map_err(SandboxError::Io)?;
        }
        Ok(())
    }

    fn root_path(&self) -> &Path {
        &self.root_path
    }

    fn resolve_path(&self, rel: &str) -> SandboxResult<PathBuf> {
        // Accept absolute paths — join to the filesystem root and check containment.
        let resolved = if rel.starts_with('/') {
            PathBuf::from(rel)
        } else {
            self.root_path.join(rel)
        };
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
            cmd.current_dir(&cwd);
            cmd.stdin(std::process::Stdio::piped());
            cmd.stdout(std::process::Stdio::piped());
            cmd.stderr(std::process::Stdio::piped());

            #[cfg(unix)]
            {
                cmd.process_group(0);
            }

            let mut child = cmd.spawn().map_err(SandboxError::Io)?;

            // Write stdin data if provided
            if let Some(ref stdin_data) = req.stdin {
                if let Some(ref mut stdin) = child.stdin {
                    let _ = stdin.write_all(stdin_data);
                }
            }
            // Always close stdin so the child sees EOF
            drop(child.stdin.take());

            // Wait with timeout
            let timeout = req.timeout;
            let pid = child.id();
            let start = std::time::Instant::now();
            loop {
                match child.try_wait().map_err(SandboxError::Io)? {
                    Some(status) => {
                        let output = child.wait_with_output().map_err(SandboxError::Io)?;
                        #[cfg(unix)]
                        let killed_by_signal = status.signal();
                        #[cfg(not(unix))]
                        let killed_by_signal = None;
                        return Ok(CommandOutput {
                            stdout: output.stdout,
                            stderr: output.stderr,
                            exit_code: status.code().unwrap_or(-1),
                            killed_by_signal,
                        });
                    }
                    None => {
                        if start.elapsed() > timeout {
                            #[cfg(unix)]
                            {
                                use std::process::Command as KillCommand;
                                // Send SIGTERM to the process group (negative pid)
                                let _ = KillCommand::new("kill")
                                    .arg("-TERM")
                                    .arg(format!("-{pid}"))
                                    .status();
                                std::thread::sleep(Duration::from_secs(5));
                                // Send SIGKILL if still alive
                                let _ = KillCommand::new("kill")
                                    .arg("-KILL")
                                    .arg(format!("-{pid}"))
                                    .status();
                            }
                            let _ = child.wait();
                            return Err(SandboxError::Timeout(timeout));
                        }
                        std::thread::sleep(Duration::from_millis(100));
                    }
                }
            }
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
        #[allow(clippy::cast_possible_truncation)]
        let start = offset.unwrap_or(0) as usize;
        #[allow(clippy::cast_possible_truncation)]
        let end = limit.map(|l| start + l as usize).unwrap_or(content.len());
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

    #[allow(clippy::cast_possible_truncation)]
    async fn metadata(&self, path: &Path) -> SandboxResult<FileMetadata> {
        let meta = std::fs::metadata(path).map_err(SandboxError::Io)?;
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
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
    use std::time::Duration;

    async fn setup() -> LocalSandbox {
        let sb = LocalSandbox::new(None);
        sb.start().await.unwrap();
        sb
    }

    async fn teardown(sb: LocalSandbox) {
        sb.cleanup().await.unwrap();
    }

    #[tokio::test]
    async fn test_resolve_path_normal() {
        let sb = setup().await;
        let resolved = sb.resolve_path("foo/bar.txt").unwrap();
        assert!(resolved.ends_with("foo/bar.txt"));
        assert!(resolved.starts_with(sb.root_path()));
        teardown(sb).await;
    }

    #[tokio::test]
    async fn test_resolve_path_rejects_absolute() {
        let sb = setup().await;
        assert!(sb.resolve_path("/etc/passwd").is_err());
        teardown(sb).await;
    }

    #[tokio::test]
    async fn test_resolve_path_rejects_traversal() {
        let sb = setup().await;
        assert!(sb.resolve_path("../../../etc/passwd").is_err());
        teardown(sb).await;
    }

    #[tokio::test]
    async fn test_write_and_read_file() {
        let sb = setup().await;
        let path = sb.root_path().join("test.txt");
        sb.write_file(&path, b"hello world").await.unwrap();
        let content = sb.read_file(&path, None, None).await.unwrap();
        assert_eq!(content, b"hello world");
        teardown(sb).await;
    }

    #[tokio::test]
    async fn test_execute_echo() {
        let sb = setup().await;
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
        teardown(sb).await;
    }

    #[tokio::test]
    async fn test_read_dir() {
        let sb = setup().await;
        let root = sb.root_path().to_path_buf();
        sb.write_file(&root.join("a.txt"), b"a").await.unwrap();
        sb.write_file(&root.join("b.txt"), b"b").await.unwrap();
        sb.create_dir_all(&root.join("sub")).await.unwrap();

        let entries = sb.read_dir(&root).await.unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"a.txt"));
        assert!(names.contains(&"b.txt"));
        assert!(names.contains(&"sub"));

        let sub = entries.iter().find(|e| e.name == "sub").unwrap();
        assert_eq!(sub.file_type, FileType::Directory);
        teardown(sb).await;
    }

    #[tokio::test]
    async fn test_metadata() {
        let sb = setup().await;
        let path = sb.root_path().join("meta.txt");
        sb.write_file(&path, b"hello").await.unwrap();

        let meta = sb.metadata(&path).await.unwrap();
        assert_eq!(meta.size, 5);
        assert_eq!(meta.file_type, FileType::File);
        assert!(meta.mtime > 0);
        teardown(sb).await;
    }

    #[tokio::test]
    async fn test_create_dir_all() {
        let sb = setup().await;
        let deep = sb.root_path().join("a").join("b").join("c");
        sb.create_dir_all(&deep).await.unwrap();
        assert!(deep.exists());
        assert!(deep.is_dir());
        teardown(sb).await;
    }
}
