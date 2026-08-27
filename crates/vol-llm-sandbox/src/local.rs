use crate::{
    CommandOutput, DirEntry, FileMetadata, FileType, Sandbox, SandboxError, SandboxId,
    SandboxResult, SandboxStatus,
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

/// Collect all descendant PIDs of `root` via repeated `pgrep -P` walks (BFS).
/// Callers kill the returned list in reverse order to terminate leaves first.
/// Requires procps `pgrep`; returns an empty list if it is unavailable.
#[cfg(unix)]
fn collect_descendants(root: u32) -> Vec<u32> {
    let mut out = Vec::new();
    let mut frontier = vec![root];
    while let Some(parent) = frontier.pop() {
        let Ok(output) = std::process::Command::new("pgrep")
            .arg("-P")
            .arg(parent.to_string())
            .output()
        else {
            continue;
        };
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if let Ok(pid) = line.trim().parse::<u32>() {
                out.push(pid);
                frontier.push(pid);
            }
        }
    }
    out
}

/// A sandbox using a local directory as its root.
///
/// If created with `Some(path)`, the directory is caller-owned and NOT deleted on cleanup.
/// If created with `None`, a temp directory is created and IS deleted on cleanup.
pub struct LocalSandbox {
    id: SandboxId,
    root_path: PathBuf,
    is_temp: bool,
    status: SandboxStatus,
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
        Self {
            id: SandboxId::new(),
            root_path,
            is_temp,
            status: SandboxStatus::Running,
        }
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
    fn id(&self) -> &SandboxId {
        &self.id
    }

    fn kind(&self) -> &str {
        "local"
    }

    fn status(&self) -> SandboxStatus {
        self.status
    }

    fn root_path(&self) -> Option<&Path> {
        Some(&self.root_path)
    }

    fn resolve_path(&self, rel: &str) -> SandboxResult<PathBuf> {
        // Reject absolute paths — consistent with SSHSandbox and WasmSandbox.
        // Callers must convert absolute paths to relative before calling.
        // Use ToolContext::resolve_path which handles this conversion.
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
            // Ensure working directory exists (sandbox root may have been
            // cleaned up since start, e.g. by a process manager).
            let _ = std::fs::create_dir_all(&cwd);
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
                                // NOTE: no process-group kills (`kill -TERM -pgid`):
                                // sandboxes such as the Claude Code bash sandbox kill
                                // the caller's whole process tree when a group signal
                                // is actually delivered. Use positive-pid kills only.
                                // Walk the full descendant tree (pgrep -P, BFS) and
                                // TERM it deepest-first, then TERM the direct child;
                                // escalate to KILL after a 2s grace. Requires procps
                                // (pgrep) — present in both agent-server runtime images.
                                let descendants = collect_descendants(pid);
                                for d in descendants.iter().rev() {
                                    let _ = KillCommand::new("kill")
                                        .arg("-TERM")
                                        .arg(d.to_string())
                                        .status();
                                }
                                let _ = KillCommand::new("kill")
                                    .arg("-TERM")
                                    .arg(pid.to_string())
                                    .status();
                                let grace = std::time::Instant::now();
                                loop {
                                    if child.try_wait().map_err(SandboxError::Io)?.is_some() {
                                        break;
                                    }
                                    if grace.elapsed() > Duration::from_secs(2) {
                                        // Escalate to KILL, deepest-first. Then reap with
                                        // a bound — SIGKILL can be undeliverable (D-state
                                        // process), and this thread must not hang the
                                        // whole tool call forever.
                                        for d in descendants.iter().rev() {
                                            let _ = KillCommand::new("kill")
                                                .arg("-KILL")
                                                .arg(d.to_string())
                                                .status();
                                        }
                                        let _ = KillCommand::new("kill")
                                            .arg("-KILL")
                                            .arg(pid.to_string())
                                            .status();
                                        let reap = std::time::Instant::now();
                                        loop {
                                            if child.try_wait().map_err(SandboxError::Io)?.is_some()
                                            {
                                                break;
                                            }
                                            if reap.elapsed() > Duration::from_secs(5) {
                                                break;
                                            }
                                            std::thread::sleep(Duration::from_millis(50));
                                        }
                                        return Err(SandboxError::Timeout(timeout));
                                    }
                                    std::thread::sleep(Duration::from_millis(50));
                                }
                                let _ = child.wait();
                            }
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

/// Provider for LocalSandbox instances.
pub struct LocalSandboxProvider;

#[async_trait]
impl crate::SandboxProvider for LocalSandboxProvider {
    fn kind(&self) -> &str {
        "local"
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
        let work_dir = match &spec.config {
            crate::SandboxProviderConfig::Local { work_dir } => work_dir.clone(),
            _ => None,
        };
        let sandbox = std::sync::Arc::new(LocalSandbox::new(work_dir));
        let backend_id = sandbox
            .root_path()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        Ok(crate::BackendSandboxRef {
            backend_id,
            sandbox,
        })
    }

    async fn get(&self, backend_id: &str) -> crate::SandboxResult<std::sync::Arc<dyn Sandbox>> {
        let sandbox = std::sync::Arc::new(LocalSandbox::new(Some(PathBuf::from(backend_id))));
        Ok(sandbox)
    }

    async fn list(&self) -> crate::SandboxResult<Vec<crate::SandboxInfo>> {
        Ok(vec![])
    }

    async fn start(&self, _backend_id: &str) -> crate::SandboxResult<()> {
        Ok(())
    }

    async fn pause(&self, _backend_id: &str) -> crate::SandboxResult<()> {
        Err(SandboxError::Config(
            "LocalSandbox does not support pause".to_string(),
        ))
    }

    async fn resume(&self, _backend_id: &str) -> crate::SandboxResult<()> {
        Err(SandboxError::Config(
            "LocalSandbox does not support resume".to_string(),
        ))
    }

    async fn stop(&self, _backend_id: &str) -> crate::SandboxResult<()> {
        Ok(())
    }

    async fn destroy(&self, _backend_id: &str) -> crate::SandboxResult<()> {
        Ok(())
    }
}
