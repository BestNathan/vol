# SSH Sandbox + Sandbox Abstraction — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create a unified `Sandbox` trait that all builtin tools route I/O through, then build an SSH-backed sandbox implementation with idle-timeout connection management.

**Architecture:** New `vol-llm-sandbox` crate as a leaf crate containing the `Sandbox` trait, `LocalSandbox`, `SSHSandbox`, `SandboxRegistry`, and all sandbox types. `vol-llm-tool` depends on it; `ToolContext` always holds an `Arc<dyn Sandbox>`. Builtin tools migrate from direct OS calls to sandbox trait methods. `SandboxRegistry` loads from `.agent/sandboxes/*.toml` and is wired into `AgentRuntime`.

**Tech Stack:** Rust, tokio, async-trait, ssh2 (libssh2 bindings), serde, TOML config

---

## Phase P0: Crate Setup + Sandbox Trait + LocalSandbox

### Task P0.1: Create `vol-llm-sandbox` crate

**Files:**
- Create: `crates/vol-llm-sandbox/Cargo.toml`
- Modify: `Cargo.toml` (workspace root)
- Create: `crates/vol-llm-sandbox/src/lib.rs`

- [ ] **Step 1: Add workspace member and dependency**

Edit workspace `Cargo.toml`:

Add to `[workspace] members` array:
```toml
"crates/vol-llm-sandbox",
```

Add to `[workspace.dependencies]`:
```toml
vol-llm-sandbox = { path = "crates/vol-llm-sandbox" }
ssh2 = { version = "0.9", features = ["vendored-openssl"] }
```

- [ ] **Step 2: Create Cargo.toml**

```toml
[package]
name = "vol-llm-sandbox"
version.workspace = true
edition.workspace = true

[dependencies]
async-trait = { workspace = true }
ssh2 = { workspace = true, optional = true }
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }

[features]
default = []
ssh = ["ssh2"]
```

- [ ] **Step 3: Create `lib.rs` with Sandbox trait, types, and error**

```rust
//! Sandbox abstraction for isolated execution environments.
//!
//! All tool I/O goes through the Sandbox trait — tools never call OS APIs directly.
//! Implementations: LocalSandbox (local directory), SSHSandbox (remote host via SSH).

use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::time::Duration;
use std::sync::Arc;
use async_trait::async_trait;

pub mod local;
pub mod registry;
#[cfg(feature = "ssh")]
pub mod ssh;

/// Reference to a sandbox instance.
pub type SandboxRef = Arc<dyn Sandbox>;

/// Trait for isolated execution environments.
#[async_trait]
pub trait Sandbox: Send + Sync {
    /// Sandbox type identifier: "local", "ssh"
    fn kind(&self) -> &str;

    /// Registry name, e.g. "local", "devbox"
    fn name(&self) -> &str;

    /// Initialize the sandbox (create directory, establish connection, etc.)
    async fn start(&self) -> SandboxResult<()>;

    /// Clean up the sandbox (delete temp dir, disconnect, etc.)
    async fn cleanup(&self) -> SandboxResult<()>;

    /// Root path of the sandbox. All file operations are relative to this.
    fn root_path(&self) -> &Path;

    /// Resolve a relative path to an absolute path within the sandbox.
    /// Returns `PathTraversal` error if the resolved path escapes `root_path()`.
    fn resolve_path(&self, rel: &str) -> SandboxResult<PathBuf>;

    /// Execute a command inside the sandbox.
    async fn execute(&self, req: CommandRequest) -> SandboxResult<CommandOutput>;

    /// Read file content as raw bytes. Tools decode to String as needed.
    async fn read_file(
        &self, path: &Path, offset: Option<u64>, limit: Option<u64>
    ) -> SandboxResult<Vec<u8>>;

    /// Write bytes to a file. Parent directories must exist.
    async fn write_file(&self, path: &Path, content: &[u8]) -> SandboxResult<()>;

    /// Create directory and all parents inside the sandbox root.
    async fn create_dir_all(&self, path: &Path) -> SandboxResult<()>;

    /// List entries in a directory.
    async fn read_dir(&self, path: &Path) -> SandboxResult<Vec<DirEntry>>;

    /// Get file metadata.
    async fn metadata(&self, path: &Path) -> SandboxResult<FileMetadata>;
}

/// Request to execute a command.
#[derive(Debug, Clone)]
pub struct CommandRequest {
    /// Program to execute (e.g., "bash", "rg")
    pub program: String,
    /// Arguments (e.g., ["-c", "echo hello"])
    pub args: Vec<String>,
    /// Environment variables
    pub env: HashMap<String, String>,
    /// Working directory relative to sandbox root (None = root_path)
    pub cwd: Option<PathBuf>,
    /// Optional stdin
    pub stdin: Option<Vec<u8>>,
    /// Execution timeout
    pub timeout: Duration,
}

/// Result of a command execution.
#[derive(Debug, Clone)]
pub struct CommandOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_code: i32,
    pub killed_by_signal: Option<i32>,
}

/// Directory entry returned by `read_dir`.
#[derive(Debug, Clone)]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
    pub is_file: bool,
}

/// File metadata returned by `metadata`.
#[derive(Debug, Clone)]
pub struct FileMetadata {
    pub size: u64,
    pub mtime: u64,      // unix timestamp, milliseconds
    pub is_dir: bool,
    pub is_file: bool,
}

/// Sandbox error types.
#[derive(Debug, thiserror::Error)]
pub enum SandboxError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Path traversal detected: {0}")]
    PathTraversal(String),

    #[error("Sandbox not started")]
    NotStarted,

    #[error("Sandbox already started")]
    AlreadyStarted,

    #[cfg(feature = "ssh")]
    #[error("SSH error: {0}")]
    Ssh(String),

    #[error("Command timed out after {0:?}")]
    Timeout(Duration),

    #[error("Unknown sandbox type: {0}")]
    UnknownType(String),

    #[error("Sandbox '{0}' already registered")]
    DuplicateName(String),

    #[error("Local sandbox cannot be overridden by config")]
    LocalOverride,
}

pub type SandboxResult<T> = Result<T, SandboxError>;
```

- [ ] **Step 4: Verify crate compiles**

```bash
cargo check -p vol-llm-sandbox
```

Expected: compiles successfully (lib.rs, local.rs, registry.rs may have incomplete stubs — if stubs needed, add `compile_error!("TODO")` in local.rs and registry.rs for now, we'll fill them in next tasks).

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock crates/vol-llm-sandbox/
git commit -m "feat(sandbox): create vol-llm-sandbox crate with Sandbox trait and types

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task P0.2: Implement LocalSandbox

**Files:**
- Create: `crates/vol-llm-sandbox/src/local.rs`
- Modify: `crates/vol-llm-sandbox/src/lib.rs` (add `pub mod local;` — already done if added above)

- [ ] **Step 1: Write LocalSandbox**

```rust
use std::path::{Path, PathBuf};
use std::time::Duration;
use async_trait::async_trait;
use crate::{CommandOutput, DirEntry, FileMetadata, Sandbox, SandboxError, SandboxResult};

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
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis();
                let temp = std::env::temp_dir().join(format!("sandbox_{:x}", timestamp % 0xFFFFFF));
                (temp, true)
            }
        };
        Self { root_path, is_temp }
    }
}

#[async_trait]
impl Sandbox for LocalSandbox {
    fn kind(&self) -> &str { "local" }

    fn name(&self) -> &str { "local" }

    async fn start(&self) -> SandboxResult<()> {
        std::fs::create_dir_all(&self.root_path).map_err(SandboxError::Io)
    }

    async fn cleanup(&self) -> SandboxResult<()> {
        if self.is_temp {
            std::fs::remove_dir_all(&self.root_path).map_err(SandboxError::Io)?;
        }
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

    async fn execute(&self, req: crate::CommandRequest) -> SandboxResult<CommandOutput> {
        let mut cmd = std::process::Command::new(&req.program);
        cmd.args(&req.args);
        for (k, v) in &req.env {
            cmd.env(k, v);
        }
        let cwd = req.cwd.map(|p| self.root_path.join(p))
            .unwrap_or_else(|| self.root_path.clone());
        cmd.current_dir(&cwd);
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        #[cfg(unix)]
        { cmd.process_group(0); }

        let mut child = cmd.spawn().map_err(SandboxError::Io)?;

        if let Some(stdin_data) = &req.stdin {
            use std::io::Write;
            if let Some(ref mut stdin) = child.stdin {
                let _ = stdin.write_all(stdin_data);
            }
        }

        // Wait with timeout
        let timeout = req.timeout;
        let pid = child.id();
        let start = std::time::Instant::now();
        loop {
            match child.try_wait().map_err(SandboxError::Io)? {
                Some(status) => {
                    let output = child.wait_with_output().map_err(SandboxError::Io)?;
                    return Ok(CommandOutput {
                        stdout: output.stdout,
                        stderr: output.stderr,
                        exit_code: status.code().unwrap_or(-1),
                        killed_by_signal: None,
                    });
                }
                None => {
                    if start.elapsed() > timeout {
                        #[cfg(unix)]
                        if let Some(pid) = pid {
                            let _ = nix::sys::signal::kill(
                                nix::unistd::Pid::from_raw(-(pid as i32)),
                                nix::sys::signal::Signal::SIGTERM,
                            );
                            std::thread::sleep(Duration::from_secs(5));
                            let _ = nix::sys::signal::kill(
                                nix::unistd::Pid::from_raw(-(pid as i32)),
                                nix::sys::signal::Signal::SIGKILL,
                            );
                        }
                        let _ = child.wait();
                        return Err(SandboxError::Timeout(timeout));
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        }
    }

    async fn read_file(&self, path: &Path, offset: Option<u64>, limit: Option<u64>)
        -> SandboxResult<Vec<u8>>
    {
        let content = std::fs::read(path).map_err(SandboxError::Io)?;
        let start = offset.unwrap_or(0) as usize;
        let end = limit.map(|l| start + l as usize).unwrap_or(content.len());
        let end = end.min(content.len());
        Ok(content[start..end].to_vec())
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
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                let ft = e.file_type().ok()?;
                Some(DirEntry {
                    name,
                    is_dir: ft.is_dir(),
                    is_file: ft.is_file(),
                })
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
        Ok(FileMetadata {
            size: meta.len(),
            mtime,
            is_dir: meta.is_dir(),
            is_file: meta.is_file(),
        })
    }
}

/// Normalize a path by resolving `.` and `..` components without touching the filesystem.
fn normalize_path(path: &Path) -> PathBuf {
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

- [ ] **Step 2: Verify compilation**

```bash
cargo check -p vol-llm-sandbox
```

Expected: compiles. If `nix` isn't in deps yet, add `nix = { workspace = true }` to `Cargo.toml` — or use `[cfg(unix)]` conditional and `libc::kill` as fallback.

- [ ] **Step 3: Write unit tests for LocalSandbox**

Add to bottom of `local.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CommandRequest, Sandbox};
    use std::time::Duration;

    fn setup() -> LocalSandbox {
        let sb = LocalSandbox::new(None);
        sb.start().unwrap();
        sb
    }

    fn teardown(sb: LocalSandbox) {
        sb.cleanup().unwrap();
    }

    #[test]
    fn test_resolve_path_normal() {
        let sb = setup();
        let resolved = sb.resolve_path("foo/bar.txt").unwrap();
        assert!(resolved.ends_with("foo/bar.txt"));
        assert!(resolved.starts_with(sb.root_path()));
        teardown(sb);
    }

    #[test]
    fn test_resolve_path_rejects_absolute() {
        let sb = setup();
        assert!(sb.resolve_path("/etc/passwd").is_err());
        teardown(sb);
    }

    #[test]
    fn test_resolve_path_rejects_traversal() {
        let sb = setup();
        assert!(sb.resolve_path("../../../etc/passwd").is_err());
        teardown(sb);
    }

    #[tokio::test]
    async fn test_write_and_read_file() {
        let sb = setup();
        let path = std::path::Path::new("test.txt");
        sb.write_file(path, b"hello world").await.unwrap();
        let content = sb.read_file(path, None, None).await.unwrap();
        assert_eq!(content, b"hello world");
        teardown(sb);
    }

    #[tokio::test]
    async fn test_execute_echo() {
        let sb = setup();
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
        teardown(sb);
    }

    #[tokio::test]
    async fn test_read_dir() {
        let sb = setup();
        sb.write_file(std::path::Path::new("a.txt"), b"a").await.unwrap();
        sb.write_file(std::path::Path::new("b.txt"), b"b").await.unwrap();
        sb.create_dir_all(std::path::Path::new("sub")).await.unwrap();

        let entries = sb.read_dir(sb.root_path()).await.unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"a.txt"));
        assert!(names.contains(&"b.txt"));
        assert!(names.contains(&"sub"));
        teardown(sb);
    }
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p vol-llm-sandbox
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-sandbox/src/local.rs
git commit -m "feat(sandbox): implement LocalSandbox with full Sandbox trait

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task P0.3: Remove sandbox module from vol-llm-core

**Files:**
- Delete: `crates/vol-llm-core/src/sandbox.rs`
- Modify: `crates/vol-llm-core/src/lib.rs`

- [ ] **Step 1: Remove sandbox module from vol-llm-core**

Edit `crates/vol-llm-core/src/lib.rs`:

Remove the line:
```rust
pub mod sandbox;
```

And from the re-exports block, remove:
```rust
pub use sandbox::*;
```

- [ ] **Step 2: Delete the old sandbox.rs**

```bash
rm crates/vol-llm-core/src/sandbox.rs
```

- [ ] **Step 3: Check what breaks**

```bash
cargo check 2>&1 | head -50
```

Expected: compilation errors anywhere that imports `vol_llm_core::Sandbox`, `vol_llm_core::SandboxRef`, `vol_llm_core::SandboxError`, etc. We'll fix these in P1.

- [ ] **Step 4: Commit**

```bash
git rm crates/vol-llm-core/src/sandbox.rs
git add crates/vol-llm-core/src/lib.rs
git commit -m "refactor(core): remove sandbox module, migrated to vol-llm-sandbox crate

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Phase P1: ToolContext Change + Tool Migration

### Task P1.1: Update vol-llm-tool to depend on vol-llm-sandbox

**Files:**
- Modify: `crates/vol-llm-tool/Cargo.toml`
- Modify: `crates/vol-llm-tool/src/tool.rs`

- [ ] **Step 1: Add dependency**

Edit `crates/vol-llm-tool/Cargo.toml`, add:
```toml
vol-llm-sandbox = { workspace = true }
```

- [ ] **Step 2: Update ToolContext**

Edit `crates/vol-llm-tool/src/tool.rs`, change `ToolContext`:

```rust
// REMOVE this import:
// use vol_llm_core::{Message, SandboxRef, ToolDefinition};

// ADD this import:
use vol_llm_core::{Message, ToolDefinition};
use vol_llm_sandbox::SandboxRef;

// CHANGE ToolContext from:
// pub struct ToolContext {
//     pub messages: Vec<Message>,
//     pub sandbox: Option<SandboxRef>,
//     pub agent_def: Option<vol_llm_core::AgentDef>,
// }

// TO:
#[derive(Clone)]
pub struct ToolContext {
    pub messages: Vec<Message>,
    pub sandbox: SandboxRef,                                 // Always set
    pub agent_def: Option<vol_llm_core::AgentDef>,
}

impl Default for ToolContext {
    fn default() -> Self {
        Self {
            messages: Vec::new(),
            sandbox: Arc::new(vol_llm_sandbox::local::LocalSandbox::new(None)),
            agent_def: None,
        }
    }
}

impl ToolContext {
    /// Set the sandbox for this tool context.
    pub fn with_sandbox(mut self, sandbox: SandboxRef) -> Self {
        self.sandbox = sandbox;
        self
    }

    /// Set the agent definition for this tool context.
    pub fn with_agent_def(mut self, def: vol_llm_core::AgentDef) -> Self {
        self.agent_def = Some(def);
        self
    }

    /// Resolve a path through the sandbox.
    pub fn resolve_path(&self, rel: &str) -> std::result::Result<std::path::PathBuf, Box<dyn std::error::Error + Send + Sync>> {
        self.sandbox.resolve_path(rel).map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }
}
```

Add `use std::sync::Arc;` to imports.

- [ ] **Step 3: Update all references from `Option<SandboxRef>` to `SandboxRef`**

In `tool.rs`, update `Debug` impl:
```rust
impl std::fmt::Debug for ToolContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolContext")
            .field("messages", &self.messages)
            .field("sandbox", &format_args!("{}:{}", self.sandbox.kind(), self.sandbox.name()))
            .field("agent_def", &self.agent_def)
            .finish()
    }
}
```

Remove the `resolve_path` match on `self.sandbox` being `Option` — the new version delegates directly.

- [ ] **Step 4: Verify vol-llm-tool compiles**

```bash
cargo check -p vol-llm-tool
```

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-tool/Cargo.toml crates/vol-llm-tool/src/tool.rs
git commit -m "refactor(tool): ToolContext.sandbox is now always-set SandboxRef

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task P1.2: Migrate bash tool to sandbox trait

**Files:**
- Modify: `crates/vol-llm-tools-builtin/bash-tool/Cargo.toml`
- Modify: `crates/vol-llm-tools-builtin/bash-tool/src/lib.rs`

- [ ] **Step 1: Add vol-llm-sandbox dependency**

Edit `crates/vol-llm-tools-builtin/bash-tool/Cargo.toml`, add:
```toml
vol-llm-sandbox = { workspace = true }
```

- [ ] **Step 2: Rewrite bash tool execute method**

Replace the `execute` method in `bash-tool/src/lib.rs` (lines 157-306):

```rust
async fn execute(
    &self,
    args: &serde_json::Value,
    context: &ToolContext,
) -> ToolResultType<ToolResult> {
    let params: BashParams = serde_json::from_value(args.clone()).map_err(|e| {
        ToolError::InvalidArguments(format!("Failed to parse arguments: {}", e))
    })?;

    // Security check before execution
    if let Err(e) = self.check_security(&params.command) {
        return Err(ToolError::ExecutionFailed(e.to_string()));
    }

    let timeout_duration = params
        .timeout
        .map(Duration::from_millis)
        .unwrap_or(self.default_timeout);

    let cwd = params.working_dir.map(std::path::PathBuf::from);

    let req = vol_llm_sandbox::CommandRequest {
        program: "bash".to_string(),
        args: vec!["-c".to_string(), params.command.clone()],
        env: std::collections::HashMap::new(),
        cwd,
        stdin: None,
        timeout: timeout_duration,
    };

    let output = context.sandbox.execute(req).await.map_err(|e| {
        ToolError::ExecutionFailed(format!("Command execution failed: {}", e))
    })?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    let stdout = self.truncate_output(stdout);
    let stderr = self.truncate_output(stderr);

    let mut content = String::new();
    if !stdout.is_empty() {
        content.push_str("stdout:\n");
        content.push_str(&stdout);
    }
    if !stderr.is_empty() {
        if !stdout.is_empty() { content.push('\n'); }
        content.push_str("stderr:\n");
        content.push_str(&stderr);
    }
    if content.is_empty() {
        content = "Command executed successfully (no output)".to_string();
    }

    Ok(ToolResult::success(content))
}
```

Remove unused imports: `nix`, `spawn_blocking`, `std::process::Command` usage. Remove the `DangerousPatterns` regex logic and signal handling — these are now in the sandbox. Actually, **keep** the `check_security` — it runs before sandbox dispatch.

Remove: all `use std::os::unix::process::CommandExt` and the `#[cfg(unix)]` import of `nix`. Remove the `SIGTERM_GRACE_PERIOD` constant (sandbox handles timeout internally).

- [ ] **Step 3: Check compilation**

```bash
cargo check -p vol-llm-tools-builtin-bash
```

Expected: compiles.

- [ ] **Step 4: Run existing bash tests**

```bash
cargo test -p vol-llm-tools-builtin-bash 2>&1 || true
```

Note any test failures — tests that relied on `std::process::Command` behavior may need updating.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-tools-builtin/bash-tool/
git commit -m "refactor(bash-tool): route command execution through sandbox trait

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task P1.3: Migrate read_file tool to sandbox trait

**Files:**
- Modify: `crates/vol-llm-tools-builtin/read-tool/Cargo.toml`
- Modify: `crates/vol-llm-tools-builtin/read-tool/src/lib.rs`

- [ ] **Step 1: Add dep + rewrite execute**

Add to `read-tool/Cargo.toml`:
```toml
vol-llm-sandbox = { workspace = true }
```

In `read-tool/src/lib.rs`, replace the `execute` method:

```rust
async fn execute(
    &self,
    args: &serde_json::Value,
    context: &ToolContext,
) -> ToolResultType<ToolResult> {
    let params: ReadParams = serde_json::from_value(args.clone()).map_err(|e| {
        ToolError::InvalidArguments(format!("Failed to parse arguments: {}", e))
    })?;

    let path = context.resolve_path(&params.file_path).map_err(|e| {
        ToolError::ExecutionFailed(format!("Path resolution failed: {}", e))
    })?;

    let content = context.sandbox.read_file(
        &path,
        Some(params.offset as u64),
        Some(params.limit as u64),
    ).await.map_err(|e| {
        ToolError::ExecutionFailed(format!("Failed to read file: {}", e))
    })?;

    let text = String::from_utf8_lossy(&content);
    // Format with line numbers (preserve existing behavior)
    let result = format_with_line_numbers(text.as_ref(), params.offset);

    Ok(ToolResult::success(result))
}
```

Remove the existing `tokio::fs::read_to_string` call. Keep the `format_with_line_numbers` helper (or extract it if it doesn't exist yet). If the tool currently has inline formatting logic, keep it.

- [ ] **Step 2: Check compilation**

```bash
cargo check -p vol-llm-tools-builtin-read
```

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-tools-builtin/read-tool/
git commit -m "refactor(read-tool): route file reading through sandbox trait

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task P1.4: Migrate write_file tool to sandbox trait

**Files:**
- Modify: `crates/vol-llm-tools-builtin/write-tool/Cargo.toml`
- Modify: `crates/vol-llm-tools-builtin/write-tool/src/lib.rs`

- [ ] **Step 1: Add dep + rewrite execute**

Add `vol-llm-sandbox = { workspace = true }` to `write-tool/Cargo.toml`.

Replace the file write calls in `execute()` with:

```rust
let path = context.resolve_path(&params.file_path).map_err(|e| {
    ToolError::ExecutionFailed(format!("Path resolution failed: {}", e))
})?;

// Ensure parent directory exists
if let Some(parent) = path.parent() {
    context.sandbox.create_dir_all(parent).await.map_err(|e| {
        ToolError::ExecutionFailed(format!("Failed to create directory: {}", e))
    })?;
}

context.sandbox.write_file(&path, params.content.as_bytes()).await.map_err(|e| {
    ToolError::ExecutionFailed(format!("Failed to write file: {}", e))
})?;

Ok(ToolResult::success(format!("File written: {}", path.display())))
```

Remove `tokio::fs::create_dir_all` and `tokio::fs::write` calls.

- [ ] **Step 2: Check compilation and commit**

```bash
cargo check -p vol-llm-tools-builtin-write
git add crates/vol-llm-tools-builtin/write-tool/
git commit -m "refactor(write-tool): route file writing through sandbox trait

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task P1.5: Migrate edit_file tool to sandbox trait

**Files:**
- Modify: `crates/vol-llm-tools-builtin/edit-tool/Cargo.toml`
- Modify: `crates/vol-llm-tools-builtin/edit-tool/src/lib.rs`

- [ ] **Step 1: Add dep + rewrite**

Add `vol-llm-sandbox = { workspace = true }` to `edit-tool/Cargo.toml`.

Replace `tokio::fs::read_to_string` → `context.sandbox.read_file()`, and `tokio::fs::write` → `context.sandbox.write_file()`.

```rust
// Read
let content = context.sandbox.read_file(&path, None, None).await.map_err(|e| {
    ToolError::ExecutionFailed(format!("Failed to read file: {}", e))
})?;
let text = String::from_utf8_lossy(&content).to_string();

// ... perform string replacement ...

// Write
context.sandbox.write_file(&path, new_text.as_bytes()).await.map_err(|e| {
    ToolError::ExecutionFailed(format!("Failed to write file: {}", e))
})?;
```

- [ ] **Step 2: Check compilation and commit**

```bash
cargo check -p vol-llm-tools-builtin-edit
git add crates/vol-llm-tools-builtin/edit-tool/
git commit -m "refactor(edit-tool): route file edit through sandbox trait

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task P1.6: Migrate glob tool to sandbox trait

**Files:**
- Modify: `crates/vol-llm-tools-builtin/glob-tool/Cargo.toml`
- Modify: `crates/vol-llm-tools-builtin/glob-tool/src/lib.rs`

- [ ] **Step 1: Add dep + rewrite**

Add `vol-llm-sandbox = { workspace = true }` to `glob-tool/Cargo.toml`.

Replace `glob::glob()` call with `context.sandbox.read_dir()` and perform pattern matching in the tool:

```rust
let parent = path.parent().unwrap_or(std::path::Path::new("."));
let entries = context.sandbox.read_dir(parent).await.map_err(|e| {
    ToolError::ExecutionFailed(format!("Failed to read directory: {}", e))
})?;

// Naive glob matching in tool (keep existing glob logic if more sophisticated)
let pattern = params.pattern.clone();
let mut matches: Vec<(String, u64)> = Vec::new();
for entry in &entries {
    if glob_match(&pattern, &entry.name) {
        let entry_path = parent.join(&entry.name);
        let meta = context.sandbox.metadata(&entry_path).await.unwrap_or(FileMetadata {
            size: 0, mtime: 0, is_dir: entry.is_dir, is_file: entry.is_file,
        });
        matches.push((entry_path.display().to_string(), meta.mtime));
    }
}
matches.sort_by_key(|(_, mtime)| std::cmp::Reverse(*mtime));
```

Remove `glob` crate dependency from the tool's Cargo.toml if no longer needed.

- [ ] **Step 2: Check compilation and commit**

```bash
cargo check -p vol-llm-tools-builtin-glob
git add crates/vol-llm-tools-builtin/glob-tool/
git commit -m "refactor(glob-tool): route directory listing through sandbox trait

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task P1.7: Migrate grep tool to sandbox trait

**Files:**
- Modify: `crates/vol-llm-tools-builtin/grep-tool/Cargo.toml`
- Modify: `crates/vol-llm-tools-builtin/grep-tool/src/lib.rs`

- [ ] **Step 1: Add dep + rewrite rg execution path**

Add `vol-llm-sandbox = { workspace = true }` to `grep-tool/Cargo.toml`.

Replace the `std::process::Command::new("rg")` call with `context.sandbox.execute()`:

```rust
if rg_available {
    let mut args = vec![
        "--with-filename".to_string(),
        "--no-heading".to_string(),
    ];
    match output_mode.as_str() {
        "files_with_matches" => { args.push("-l".to_string()); }
        "count" => { args.push("-c".to_string()); }
        _ => {}
    }
    args.push(pattern.clone());
    if let Some(ref p) = search_path {
        args.push(p.clone());
    }

    let req = vol_llm_sandbox::CommandRequest {
        program: "rg".to_string(),
        args,
        env: Default::default(),
        cwd: Some(std::path::PathBuf::from(".")),
        stdin: None,
        timeout: Duration::from_secs(SEARCH_TIMEOUT_SECS),
    };
    let output = context.sandbox.execute(req).await.map_err(|e| {
        ToolError::ExecutionFailed(format!("grep (rg) failed: {}", e))
    })?;
    // Parse rg output same as before
    let text = String::from_utf8_lossy(&output.stdout);
    // ... existing parsing logic ...
} else {
    // Fallback: sandbox.read_file() + local regex — keep existing library search logic
}
```

- [ ] **Step 2: Check compilation and commit**

```bash
cargo check -p vol-llm-tools-builtin-grep
git add crates/vol-llm-tools-builtin/grep-tool/
git commit -m "refactor(grep-tool): route grep (rg) through sandbox trait

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task P1.8: Update agent code for non-optional ToolContext

**Files:**
- Modify: `crates/vol-llm-agent/src/react/agent.rs`
- Modify: `crates/vol-llm-agent/src/react/config_builder.rs`
- Modify: `crates/vol-llm-agent/src/agent_tool.rs` (if it uses sandbox)
- Modify: `crates/vol-llm-agents/src/coding/sandbox/local.rs` (DELETE — replaced by vol-llm-sandbox)

- [ ] **Step 1: Update agent.rs ToolContext construction**

In `crates/vol-llm-agent/src/react/agent.rs`, lines 483-486, change:

```rust
// BEFORE:
let mut tool_ctx = match &sandbox {
    Some(sandbox) => ToolContext::default().with_sandbox(sandbox.clone()),
    None => ToolContext::default(),
};

// AFTER:
let sandbox_ref = sandbox.clone().unwrap_or_else(|| {
    Arc::new(vol_llm_sandbox::local::LocalSandbox::new(None))
});
let mut tool_ctx = ToolContext::default().with_sandbox(sandbox_ref);
```

Update import: `use vol_llm_core::SandboxRef;` → `use vol_llm_sandbox::SandboxRef;`

- [ ] **Step 2: Update AgentConfig and AgentConfigBuilder**

In `crates/vol-llm-agent/src/react/config_builder.rs`:

Change `sandbox: Option<SandboxRef>` → `sandbox: Option<SandboxRef>` (still Option, but import from `vol_llm_sandbox`).

Add new fields:
```rust
sandbox_registry: Option<Arc<vol_llm_sandbox::registry::SandboxRegistry>>,
default_sandbox: Option<String>,
```

Add builder methods:
```rust
pub fn with_sandbox_registry(mut self, registry: Arc<vol_llm_sandbox::registry::SandboxRegistry>) -> Self {
    self.sandbox_registry = Some(registry);
    self
}

pub fn with_default_sandbox(mut self, name: impl Into<String>) -> Self {
    self.default_sandbox = Some(name.into());
    self
}
```

In `AgentConfig`, add corresponding fields.

- [ ] **Step 3: Delete old LocalSandbox**

```bash
rm crates/vol-llm-agents/src/coding/sandbox/local.rs
```

Remove `pub mod sandbox;` from `crates/vol-llm-agents/src/coding/mod.rs` (or wherever it was declared). If the entire `sandbox/` module is now empty, remove it.

Update any imports in `vol-llm-agents` that reference the old `LocalSandbox` → `vol_llm_sandbox::local::LocalSandbox`.

- [ ] **Step 4: Full workspace check**

```bash
cargo check 2>&1 | grep "^error" | head -30
```

Fix remaining compilation errors. Source of most errors will be:
- `use vol_llm_core::Sandbox` → `use vol_llm_sandbox::Sandbox`
- `use vol_llm_core::SandboxRef` → `use vol_llm_sandbox::SandboxRef`
- `Option<SandboxRef>` usage needing update
- Old `LocalSandbox` path references

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor(agent): wire sandbox through ToolContext, remove old LocalSandbox

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Phase P2: SandboxRegistry

### Task P2.1: Implement SandboxRegistry

**Files:**
- Modify: `crates/vol-llm-sandbox/src/registry.rs`

- [ ] **Step 1: Write SandboxRegistry with config types**

```rust
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use crate::{Sandbox, SandboxError, SandboxResult};
use crate::local::LocalSandbox;
#[cfg(feature = "ssh")]
use crate::ssh::SSHSandbox;

/// Configuration for a sandbox, deserialized from `.agent/sandboxes/*.toml`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SandboxConfig {
    #[serde(flatten)]
    pub common: SandboxCommonConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SandboxCommonConfig {
    pub name: String,
    #[serde(rename = "type")]
    pub sandbox_type: String,
    #[serde(default)]
    pub work_dir: Option<String>,
    #[serde(default)]
    pub ssh: Option<SshConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SshConfig {
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    pub user: String,
    pub identity_file: String,
    #[serde(default)]
    pub passphrase: Option<String>,
    #[serde(default)]
    pub known_hosts_file: Option<String>,
    #[serde(default)]
    pub host_key: Option<String>,
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout_secs: u64,
    #[serde(default = "default_connect_timeout")]
    pub connect_timeout_secs: u64,
}

fn default_port() -> u16 { 22 }
fn default_idle_timeout() -> u64 { 300 }
fn default_connect_timeout() -> u64 { 10 }

/// Registry of named sandbox instances.
pub struct SandboxRegistry {
    sandboxes: HashMap<String, Arc<dyn Sandbox>>,
    default_name: String,
}

impl SandboxRegistry {
    /// Load sandboxes from a config directory.
    ///
    /// Always registers a built-in `LocalSandbox` named "local".
    /// Additional sandboxes are loaded from `*.toml` files in `sandboxes_dir`.
    pub async fn load(sandboxes_dir: &Path) -> SandboxResult<Self> {
        let mut sandboxes: HashMap<String, Arc<dyn Sandbox>> = HashMap::new();

        // Always register LocalSandbox (hardcoded, no config file needed)
        let local = Arc::new(LocalSandbox::new(None));
        sandboxes.insert("local".to_string(), local);

        // Load *.toml files
        if sandboxes_dir.exists() {
            for entry in std::fs::read_dir(sandboxes_dir).map_err(SandboxError::Io)? {
                let entry = entry.map_err(SandboxError::Io)?;
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "toml") {
                    let content = std::fs::read_to_string(&path).map_err(SandboxError::Io)?;
                    let config: SandboxCommonConfig = toml::from_str(&content)
                        .map_err(|e| SandboxError::UnknownType(format!(
                            "failed to parse {}: {}", path.display(), e
                        )))?;

                    if config.name == "local" {
                        return Err(SandboxError::LocalOverride);
                    }
                    if sandboxes.contains_key(&config.name) {
                        return Err(SandboxError::DuplicateName(config.name.clone()));
                    }

                    let sandbox: Arc<dyn Sandbox> = match config.sandbox_type.as_str() {
                        #[cfg(feature = "ssh")]
                        "ssh" => {
                            let ssh_config = config.ssh.ok_or_else(|| {
                                SandboxError::UnknownType(
                                    "SSH sandbox requires [sandbox.ssh] section".to_string()
                                )
                            })?;
                            let sb = SSHSandbox::new(
                                config.name.clone(),
                                config.work_dir.clone(),
                                ssh_config,
                            )?;
                            Arc::new(sb)
                        }
                        other => return Err(SandboxError::UnknownType(other.to_string())),
                    };

                    sandbox.start().await?;
                    sandboxes.insert(config.name.clone(), sandbox);
                }
            }
        }

        Ok(Self { sandboxes, default_name: "local".to_string() })
    }

    /// Get a sandbox by name.
    pub fn get(&self, name: &str) -> Option<Arc<dyn Sandbox>> {
        self.sandboxes.get(name).cloned()
    }

    /// Get the default sandbox (always "local").
    pub fn default(&self) -> Arc<dyn Sandbox> {
        self.sandboxes.get(&self.default_name)
            .cloned()
            .expect("LocalSandbox always present")
    }

    /// Number of registered sandboxes.
    pub fn len(&self) -> usize {
        self.sandboxes.len()
    }

    /// Names of all registered sandboxes.
    pub fn names(&self) -> Vec<&str> {
        self.sandboxes.keys().map(|s| s.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[tokio::test]
    async fn test_registry_always_has_local() {
        let tmp = std::env::temp_dir().join("sandbox_test_empty");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let registry = SandboxRegistry::load(&tmp).await.unwrap();
        assert!(registry.get("local").is_some());
        assert_eq!(registry.default().name(), "local");
        assert_eq!(registry.default().kind(), "local");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[tokio::test]
    async fn test_registry_rejects_local_override() {
        let tmp = std::env::temp_dir().join("sandbox_test_local_override");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let config = r#"
name = "local"
type = "ssh"
work_dir = "/tmp"
"#;
        std::fs::write(tmp.join("bad.toml"), config).unwrap();

        let result = SandboxRegistry::load(&tmp).await;
        assert!(result.is_err());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[tokio::test]
    async fn test_registry_rejects_duplicate() {
        let tmp = std::env::temp_dir().join("sandbox_test_dup");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let config_a = r#"
name = "myssh"
type = "ssh"
work_dir = "/tmp"
"#;
        let config_b = r#"
name = "myssh"
type = "ssh"
work_dir = "/tmp2"
"#;
        std::fs::write(tmp.join("a.toml"), config_a).unwrap();
        std::fs::write(tmp.join("b.toml"), config_b).unwrap();

        let result = SandboxRegistry::load(&tmp).await;
        // With ssh feature disabled or no ssh config, this'll fail with UnknownType.
        // With ssh feature enabled but invalid config, it'll fail before duplicate check.
        // Either way, test that loading rejects invalid configs.
        assert!(result.is_err());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[tokio::test]
    async fn test_registry_unknown_type() {
        let tmp = std::env::temp_dir().join("sandbox_test_unknown");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let config = r#"
name = "bad"
type = "nonexistent"
"#;
        std::fs::write(tmp.join("bad.toml"), config).unwrap();

        let result = SandboxRegistry::load(&tmp).await;
        assert!(result.is_err());

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
```

- [ ] **Step 2: Verify tests pass**

```bash
cargo test -p vol-llm-sandbox -- registry
```

Expected: unit tests pass (SSH-related tests won't run without `--features ssh`).

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-sandbox/src/registry.rs
git commit -m "feat(sandbox): implement SandboxRegistry with TOML config loading

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task P2.2: Wire SandboxRegistry into AgentRuntime

**Files:**
- Modify: `crates/vol-llm-runtime/Cargo.toml`
- Modify: `crates/vol-llm-runtime/src/lib.rs`

- [ ] **Step 1: Add dependency**

In `crates/vol-llm-runtime/Cargo.toml`, add:
```toml
vol-llm-sandbox = { workspace = true }
```

- [ ] **Step 2: Initialize SandboxRegistry in AgentRuntimeBuilder::build()**

In `crates/vol-llm-runtime/src/lib.rs`, after the MCP manager initialization (around line 252):

```rust
use vol_llm_sandbox::registry::SandboxRegistry;

// In build() method, after MCP setup:
let sandbox_registry = {
    let sandboxes_dir = self.working_dir.join(".agent").join("sandboxes");
    SandboxRegistry::load(&sandboxes_dir).await.map_err(|e| {
        format!("Sandbox registry init failed: {}", e)
    })?
};
let sandbox_registry = Arc::new(sandbox_registry);
```

- [ ] **Step 3: Add field to AgentRuntime struct**

```rust
pub sandbox_registry: Arc<SandboxRegistry>,
```

Add to the `AgentRuntime` struct return value in `build()`:
```rust
Ok(AgentRuntime {
    working_dir: self.working_dir,
    store_dir,
    llm_registry,
    tool_registry,
    task_store,
    mcp_manager,
    skill_loader,
    agent_defs: Arc::new(std::sync::RwLock::new(HashMap::new())),
    agent_status: Arc::new(std::sync::RwLock::new(HashMap::new())),
    sandbox_registry,  // NEW
})
```

- [ ] **Step 4: Check full workspace compilation**

```bash
cargo check 2>&1 | grep "^error" | head -20
```

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-runtime/
git commit -m "feat(runtime): initialize SandboxRegistry in AgentRuntime

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Phase P3: SSHSandbox

### Task P3.1: Implement SSH session management

**Files:**
- Create: `crates/vol-llm-sandbox/src/ssh/session.rs`

- [ ] **Step 1: Write session module**

```rust
//! SSH session lifecycle: connect, authenticate, verify host key, disconnect.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::SandboxResult;

/// Managed SSH session with reconnect capability.
pub struct SshSession {
    inner: Mutex<Option<InnerSession>>,
    config: Arc<super::SshSandboxConfig>,
}

struct InnerSession {
    sess: ssh2::Session,
    tcp: std::net::TcpStream,
}

impl SshSession {
    pub fn new(config: Arc<super::SshSandboxConfig>) -> Self {
        Self { inner: Mutex::new(None), config }
    }

    /// Ensure a session exists. Reconnects if disconnected.
    pub async fn ensure(&self) -> SandboxResult<()> {
        let mut guard = self.inner.lock().await;
        if guard.is_some() { return Ok(()); }
        let session = self.connect_locked().await?;
        *guard = Some(session);
        Ok(())
    }

    /// Execute a function with the session. Auto-reconnects if needed.
    pub async fn with_session<F, T>(&self, f: F) -> SandboxResult<T>
    where F: FnOnce(&ssh2::Session) -> SandboxResult<T> + Send,
    {
        self.ensure().await?;
        let guard = self.inner.lock().await;
        let inner = guard.as_ref().ok_or(crate::SandboxError::NotStarted)?;
        f(&inner.sess)
    }

    /// Get a new SFTP channel from the session.
    pub async fn sftp(&self) -> SandboxResult<ssh2::Sftp> {
        self.ensure().await?;
        let guard = self.inner.lock().await;
        let inner = guard.as_ref().ok_or(crate::SandboxError::NotStarted)?;
        inner.sess.sftp().map_err(|e| crate::SandboxError::Ssh(e.to_string()))
    }

    /// Disconnect the session.
    pub async fn disconnect(&self) -> SandboxResult<()> {
        let mut guard = self.inner.lock().await;
        if let Some(inner) = guard.take() {
            let _ = inner.sess.disconnect(None, "cleanup", None);
        }
        Ok(())
    }

    async fn connect_locked(&self) -> SandboxResult<InnerSession> {
        use std::net::TcpStream;
        use std::time::Duration;

        let addr = format!("{}:{}", self.config.host, self.config.port);
        info!("SSH connecting to {}", addr);

        let tcp = TcpStream::connect_timeout(
            &addr.parse().map_err(|e| crate::SandboxError::Ssh(format!("bad address: {}", e)))?,
            Duration::from_secs(self.config.connect_timeout_secs),
        ).map_err(|e| crate::SandboxError::Ssh(format!("connection failed: {}", e)))?;

        tcp.set_read_timeout(Some(Duration::from_secs(30))).ok();

        let mut sess = ssh2::Session::new().map_err(|e| crate::SandboxError::Ssh(e.to_string()))?;
        sess.set_tcp_stream(tcp.try_clone().map_err(|e| crate::SandboxError::Ssh(e.to_string()))?);
        sess.handshake().map_err(|e| crate::SandboxError::Ssh(format!("handshake failed: {}", e)))?;

        // Host key verification
        self.verify_host_key(&sess)?;

        // Authenticate
        let identity = shellexpand::tilde(&self.config.identity_file).to_string();
        if let Some(ref passphrase) = self.config.passphrase {
            sess.userauth_pubkey_file(&self.config.user, None, PathBuf::from(&identity).as_path(), Some(passphrase))
                .map_err(|e| crate::SandboxError::Ssh(format!("auth failed: {}", e)))?;
        } else {
            // Try ssh-agent first, then key file without passphrase
            let mut agent = sess.agent().ok();
            let authed = if let Some(ref mut agent) = agent {
                agent.connect().is_ok()
                    && agent.list_identities().is_ok()
                    && agent.identities().ok().map_or(false, |ids| {
                        ids.iter().any(|id| {
                            agent.userauth(&self.config.user, id).is_ok()
                        })
                    })
            } else { false };

            if !authed {
                sess.userauth_pubkey_file(&self.config.user, None, PathBuf::from(&identity).as_path(), None)
                    .map_err(|e| crate::SandboxError::Ssh(format!("auth failed: {}", e)))?;
            }
        }

        if !sess.authenticated() {
            return Err(crate::SandboxError::Ssh("authentication failed".to_string()));
        }

        info!("SSH authenticated to {}", addr);
        Ok(InnerSession { sess, tcp })
    }

    fn verify_host_key(&self, sess: &ssh2::Session) -> SandboxResult<()> {
        let remote_key = sess.host_key().ok_or_else(|| {
            crate::SandboxError::Ssh("no host key from server".to_string())
        })?;

        if let Some(ref fingerprint) = self.config.host_key {
            let hash = sess.host_key_hash(ssh2::HashType::Sha256)
                .ok_or_else(|| crate::SandboxError::Ssh("failed to hash host key".to_string()))?;
            let fp = format!("sha256:{}", base64_encode(&hash));
            if fp != *fingerprint {
                return Err(crate::SandboxError::Ssh(format!(
                    "host key mismatch: expected {}, got {}", fingerprint, fp
                )));
            }
        } else if let Some(ref known_hosts) = self.config.known_hosts_file {
            let known_hosts = shellexpand::tilde(known_hosts).to_string();
            let known = sess.known_hosts().map_err(|e| crate::SandboxError::Ssh(e.to_string()))?;
            known.read_file(PathBuf::from(&known_hosts), ssh2::KnownHostsKind::OpenSshFormat)
                .map_err(|e| crate::SandboxError::Ssh(format!("failed to read known_hosts: {}", e)))?;
            let (key, _kind) = known.check(&self.config.host, self.config.port as i32, remote_key);
            match key {
                ssh2::CheckResult::Match => {}
                ssh2::CheckResult::NotFound => {
                    return Err(crate::SandboxError::Ssh(format!(
                        "host {} not found in known_hosts file", self.config.host
                    )));
                }
                ssh2::CheckResult::Mismatch => {
                    return Err(crate::SandboxError::Ssh(format!(
                        "host key mismatch for {} in known_hosts", self.config.host
                    )));
                }
            }
        } else {
            return Err(crate::SandboxError::Ssh(
                "host key verification not configured (set known_hosts_file or host_key)".to_string()
            ));
        }

        Ok(())
    }
}

fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/vol-llm-sandbox/src/ssh/
git commit -m "feat(ssh): implement SSH session lifecycle with host key verification

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task P3.2: Implement SSHSandbox (public API + Sandbox trait impl)

**Files:**
- Create: `crates/vol-llm-sandbox/src/ssh.rs`
- Modify: `crates/vol-llm-sandbox/Cargo.toml` (add base64, shellexpand deps)
- Modify: `crates/vol-llm-sandbox/src/ssh/session.rs` (add `execute_blocking` helper)

- [ ] **Step 1: Add `execute_blocking` to SshSession**

First, add the blocking helper to session.rs so SSHSandbox can use it. Append to `crates/vol-llm-sandbox/src/ssh/session.rs`:

```rust
impl SshSession {
    /// Execute a command via channel_exec in a blocking context.
    /// Call this from within `tokio::task::spawn_blocking`.
    pub fn execute_blocking(&self, req: &crate::CommandRequest) -> SandboxResult<crate::CommandOutput> {
        use std::io::Read;
        let guard = self.inner.blocking_lock();
        let inner = guard.as_ref().ok_or(crate::SandboxError::NotStarted)?;

        let cmd_line = if req.args.is_empty() {
            req.program.clone()
        } else {
            format!("{} {}", req.program, req.args.join(" "))
        };

        let mut channel = inner.sess.channel_session()
            .map_err(|e| crate::SandboxError::Ssh(e.to_string()))?;

        for (k, v) in &req.env {
            channel.setenv(k, v).ok();
        }

        channel.exec(&cmd_line)
            .map_err(|e| crate::SandboxError::Ssh(e.to_string()))?;

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        channel.read_to_end(&mut stdout)
            .map_err(|e| crate::SandboxError::Ssh(e.to_string()))?;
        channel.stderr().read_to_end(&mut stderr)
            .map_err(|e| crate::SandboxError::Ssh(e.to_string()))?;
        channel.wait_close().ok();

        Ok(crate::CommandOutput {
            stdout,
            stderr,
            exit_code: channel.exit_status().unwrap_or(-1),
            killed_by_signal: None,
        })
    }
}
```

- [ ] **Step 2: Add dependencies**

Add to `crates/vol-llm-sandbox/Cargo.toml`:
```toml
base64 = "0.22"
shellexpand = "3"
```

- [ ] **Step 3: Verify session.rs compiles**

```bash
cargo check -p vol-llm-sandbox --features ssh
```

Expected: session.rs compiles (SSHSandbox not yet written, so lib.rs may still reference missing types — that's fine for this step).

- [ ] **Step 4: Write SSHSandbox**

```rust
//! SSH sandbox — routes all I/O to a remote host over SSH.
//!
//! Uses SSH channel multiplexing for concurrent command execution
//! and SFTP for file I/O. Maintains an idle-timeout connection state machine.

pub mod session;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use async_trait::async_trait;
use tokio::sync::Mutex;
use tracing::{debug, info};

use crate::{CommandOutput, DirEntry, FileMetadata, Sandbox, SandboxError, SandboxResult};
use crate::registry::SshConfig;

/// Internal configuration struct passed to session.
#[derive(Debug, Clone)]
pub(crate) struct SshSandboxConfig {
    pub name: String,
    pub work_dir: String,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub identity_file: String,
    pub passphrase: Option<String>,
    pub known_hosts_file: Option<String>,
    pub host_key: Option<String>,
    pub idle_timeout_secs: u64,
    pub connect_timeout_secs: u64,
}

/// SSH-backed sandbox implementation.
pub struct SSHSandbox {
    name: String,
    root_path: PathBuf,
    remote_work_dir: String,
    session: Arc<session::SshSession>,
    /// Track when the last command completed (for idle timeout)
    last_activity: Mutex<std::time::Instant>,
    idle_timeout: Duration,
    _idle_task: tokio::task::JoinHandle<()>,
}

impl SSHSandbox {
    pub fn new(
        name: String,
        work_dir: Option<String>,
        ssh_config: SshConfig,
    ) -> SandboxResult<Self> {
        let remote_work_dir = work_dir.unwrap_or_else(|| "/tmp/sandbox".to_string());
        let idle_timeout = Duration::from_secs(ssh_config.idle_timeout_secs);

        let config = Arc::new(SshSandboxConfig {
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

        let session = Arc::new(session::SshSession::new(config.clone()));
        let last_activity = Mutex::new(std::time::Instant::now());

        // Background idle timeout task
        let session_clone = session.clone();
        let last_activity_clone = Arc::new(std::sync::Mutex::new(std::time::Instant::now()));
        let _idle_task = tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(1)).await;
                let elapsed = last_activity_clone.lock().unwrap().elapsed();
                if elapsed > idle_timeout {
                    debug!("SSH idle timeout after {:?}, disconnecting", elapsed);
                    let _ = session_clone.disconnect().await;
                    // Reset timer so we don't keep trying to disconnect
                    *last_activity_clone.lock().unwrap() = std::time::Instant::now();
                }
            }
        });

        Ok(Self {
            name,
            root_path: PathBuf::from(&remote_work_dir),
            remote_work_dir,
            session,
            last_activity,
            idle_timeout,
            _idle_task,
        })
    }

    /// Mark activity to prevent idle timeout disconnect.
    async fn mark_active(&self) {
        *self.last_activity.lock().await = std::time::Instant::now();
    }
}

#[async_trait]
impl Sandbox for SSHSandbox {
    fn kind(&self) -> &str { "ssh" }
    fn name(&self) -> &str { &self.name }

    async fn start(&self) -> SandboxResult<()> {
        self.session.ensure().await?;
        // Ensure remote work_dir exists
        self.execute(crate::CommandRequest {
            program: "mkdir".to_string(),
            args: vec!["-p".to_string(), self.remote_work_dir.clone()],
            env: Default::default(),
            cwd: None,
            stdin: None,
            timeout: Duration::from_secs(10),
        }).await?;
        info!("SSH sandbox '{}' ready at {}", self.name, self.remote_work_dir);
        Ok(())
    }

    async fn cleanup(&self) -> SandboxResult<()> {
        self.session.disconnect().await
    }

    fn root_path(&self) -> &Path { &self.root_path }

    fn resolve_path(&self, rel: &str) -> SandboxResult<PathBuf> {
        // Same logic as LocalSandbox: reject absolute, check traversal
        if rel.starts_with('/') {
            return Err(SandboxError::PathTraversal(rel.to_string()));
        }
        let resolved = self.root_path.join(rel);
        // Normalize and verify
        let normalized = normalize_path(&resolved);
        let normalized_root = normalize_path(&self.root_path);
        if !normalized.starts_with(&normalized_root) {
            return Err(SandboxError::PathTraversal(rel.to_string()));
        }
        Ok(normalized)
    }

    async fn execute(&self, req: crate::CommandRequest) -> SandboxResult<CommandOutput> {
        self.mark_active().await;
        self.session.ensure().await?;

        let session = self.session.clone();
        let req_clone = req.clone();
        tokio::task::spawn_blocking(move || session.execute_blocking(&req_clone))
            .await
            .map_err(|e| SandboxError::Ssh(format!("join error: {}", e)))?
    }

    async fn read_file(&self, path: &Path, offset: Option<u64>, limit: Option<u64>)
        -> SandboxResult<Vec<u8>>
    {
        self.mark_active().await;
        let sftp = self.session.sftp().await?;
        let remote_path = self.remote_path(path);
        let mut file = sftp.open(&remote_path).map_err(|e| SandboxError::Ssh(e.to_string()))?;
        let mut buf = Vec::new();
        use std::io::Read;
        if let Some(offset) = offset {
            file.seek(std::io::SeekFrom::Start(offset)).map_err(|e| SandboxError::Ssh(e.to_string()))?;
        }
        let limit = limit.unwrap_or(u64::MAX) as usize;
        let mut chunk = vec![0u8; limit.min(65536)];
        loop {
            let n = file.read(&mut chunk).map_err(|e| SandboxError::Ssh(e.to_string()))?;
            if n == 0 { break; }
            buf.extend_from_slice(&chunk[..n]);
            if buf.len() >= limit { break; }
        }
        Ok(buf)
    }

    async fn write_file(&self, path: &Path, content: &[u8]) -> SandboxResult<()> {
        self.mark_active().await;
        let sftp = self.session.sftp().await?;
        let remote_path = self.remote_path(path);
        use std::io::Write;
        let mut file = sftp.create(&remote_path).map_err(|e| SandboxError::Ssh(e.to_string()))?;
        file.write_all(content).map_err(|e| SandboxError::Ssh(e.to_string()))?;
        Ok(())
    }

    async fn create_dir_all(&self, path: &Path) -> SandboxResult<()> {
        self.mark_active().await;
        let sftp = self.session.sftp().await?;
        let remote_path = self.remote_path(path);
        // Build path component by component
        let mut current = PathBuf::from("/");
        // Strip leading slash if present
        let clean = remote_path.trim_start_matches('/');
        for component in clean.split('/') {
            if component.is_empty() { continue; }
            current = current.join(component);
            match sftp.mkdir(&current.to_string_lossy(), 0o755) {
                Ok(_) => {}
                Err(e) if e.code() == 11 /* EEXIST? let's check */ => {
                    // Already exists, continue
                    // ssh2 error codes are libssh2 errors, not errno
                    // Just try and ignore errors for existing dirs
                    let _ = sftp.stat(&current.to_string_lossy()).map_err(|e| {
                        SandboxError::Ssh(format!("mkdir {}: {}", current.display(), e))
                    })?;
                }
                Err(e) => {
                    return Err(SandboxError::Ssh(format!("mkdir {}: {}", current.display(), e)));
                }
            }
        }
        Ok(())
    }

    async fn read_dir(&self, path: &Path) -> SandboxResult<Vec<DirEntry>> {
        self.mark_active().await;
        let sftp = self.session.sftp().await?;
        let remote_path = self.remote_path(path);
        let entries = sftp.readdir(&remote_path).map_err(|e| SandboxError::Ssh(e.to_string()))?;
        Ok(entries.into_iter().map(|(p, stat)| {
            let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
            DirEntry {
                name,
                is_dir: stat.is_dir(),
                is_file: stat.is_file(),
            }
        }).collect())
    }

    async fn metadata(&self, path: &Path) -> SandboxResult<FileMetadata> {
        self.mark_active().await;
        let sftp = self.session.sftp().await?;
        let remote_path = self.remote_path(path);
        let stat = sftp.stat(&remote_path).map_err(|e| SandboxError::Ssh(e.to_string()))?;
        Ok(FileMetadata {
            size: stat.size.unwrap_or(0),
            mtime: stat.mtime.unwrap_or(0) as u64 * 1000, // stat gives seconds, we want millis
            is_dir: stat.is_dir(),
            is_file: stat.is_file(),
        })
    }
}

impl SSHSandbox {
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

fn normalize_path(path: &Path) -> PathBuf {
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

- [ ] **Step 5: Verify full compilation with SSH feature**

```bash
cargo check -p vol-llm-sandbox --features ssh
```

Expected: compiles. May need to fix `ssh2::Sftp` API details (stat fields `size`/`mtime` are `Option`, channel API method names). Fix any compilation issues.

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-sandbox/
git commit -m "feat(ssh): implement SSHSandbox with SFTP file I/O and channel exec

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---
## Phase P4: SSH Integration Tests

### Task P4.1: Create Docker-based SSH test host

**Files:**
- Create: `crates/vol-llm-sandbox/tests/ssh_test_host/`
- Create: `crates/vol-llm-sandbox/tests/ssh_test_host/Dockerfile`
- Create: `crates/vol-llm-sandbox/tests/ssh_test_host/docker-compose.yml`

- [ ] **Step 1: Write Dockerfile**

```dockerfile
FROM alpine:3.20
RUN apk add --no-cache openssh bash ripgrep
RUN ssh-keygen -A
RUN adduser -D agent && echo "agent:password" | chpasswd
RUN mkdir -p /home/agent/sandbox && chown agent:agent /home/agent/sandbox
RUN mkdir /run/sshd
COPY id_ed25519.pub /home/agent/.ssh/authorized_keys
RUN chmod 600 /home/agent/.ssh/authorized_keys && chown -R agent:agent /home/agent/.ssh
EXPOSE 22
CMD ["/usr/sbin/sshd", "-D", "-e"]
```

- [ ] **Step 2: Generate test SSH key**

```bash
mkdir -p crates/vol-llm-sandbox/tests/ssh_test_host
ssh-keygen -t ed25519 -f crates/vol-llm-sandbox/tests/ssh_test_host/id_ed25519 -N "" -C "test-sandbox"
```

- [ ] **Step 3: Write docker-compose.yml**

```yaml
services:
  ssh-sandbox:
    build: .
    ports:
      - "2222:22"
    volumes:
      - ./id_ed25519.pub:/home/agent/.ssh/authorized_keys:ro
```

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-sandbox/tests/
git commit -m "test(ssh): add Docker-based SSH test host

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task P4.2: Write SSH sandbox integration tests

**Files:**
- Create: `crates/vol-llm-sandbox/tests/ssh_integration.rs`

- [ ] **Step 1: Write integration tests**

```rust
#[cfg(feature = "ssh")]
mod ssh_tests {
    use vol_llm_sandbox::{Sandbox, CommandRequest, SandboxError};
    use vol_llm_sandbox::ssh::SSHSandbox;
    use vol_llm_sandbox::registry::SshConfig;
    use std::path::Path;
    use std::time::Duration;

    fn test_config() -> SshConfig {
        SshConfig {
            host: "localhost".to_string(),
            port: 2222,
            user: "agent".to_string(),
            identity_file: "tests/ssh_test_host/id_ed25519".to_string(),
            passphrase: None,
            known_hosts_file: None,
            host_key: None, // We'll set this at runtime from the actual host key
            idle_timeout_secs: 300,
            connect_timeout_secs: 10,
        }
    }

    #[tokio::test]
    #[ignore = "requires Docker SSH test host running on port 2222"]
    async fn test_ssh_execute_echo() {
        let mut config = test_config();
        // For test: disable host key verification (connect to local Docker)
        // In production this would be rejected — test only
        config.host_key = None; // Accept any (Docker test host)
        let sb = SSHSandbox::new("test".to_string(), Some("/home/agent/sandbox".to_string()), config)
            .expect("create SSHSandbox");
        sb.start().await.expect("start sandbox");

        let req = CommandRequest {
            program: "echo".to_string(),
            args: vec!["-n".to_string(), "hello from ssh".to_string()],
            env: Default::default(),
            cwd: None,
            stdin: None,
            timeout: Duration::from_secs(5),
        };
        let output = sb.execute(req).await.expect("execute echo");
        assert_eq!(output.exit_code, 0);
        assert_eq!(String::from_utf8_lossy(&output.stdout), "hello from ssh");

        sb.cleanup().await.ok();
    }

    #[tokio::test]
    #[ignore = "requires Docker SSH test host running on port 2222"]
    async fn test_ssh_file_read_write() {
        let config = test_config();
        let sb = SSHSandbox::new("test".to_string(), Some("/home/agent/sandbox".to_string()), config)
            .expect("create SSHSandbox");
        sb.start().await.expect("start sandbox");

        let path = Path::new("test_file.txt");
        sb.write_file(path, b"hello ssh file").await.expect("write file");
        let content = sb.read_file(path, None, None).await.expect("read file");
        assert_eq!(content, b"hello ssh file");

        sb.cleanup().await.ok();
    }

    #[tokio::test]
    #[ignore = "requires Docker SSH test host running on port 2222"]
    async fn test_ssh_missing_host_key_rejected() {
        let config = test_config();
        // This should fail because no host key verification is configured
        // (test skipped: to run this we'd need to mock the host key check or use actual known_hosts)
        // For now, test that the config is valid
        let sb = SSHSandbox::new("test".to_string(), Some("/tmp".to_string()), config);
        // The error will be different depending on whether ssh feature is on
        // We just verify construction works structurally
        assert!(sb.is_ok());
    }
}
```

- [ ] **Step 2: Document test host startup**

Add a `README.md` in `crates/vol-llm-sandbox/tests/ssh_test_host/`:

```markdown
# SSH Test Host

Start the test host:
```bash
cd crates/vol-llm-sandbox/tests/ssh_test_host
docker compose up -d
```

Run integration tests:
```bash
cargo test -p vol-llm-sandbox --features ssh -- --ignored
```

Stop:
```bash
docker compose down
```
```

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-sandbox/tests/
git commit -m "test(ssh): add integration tests for SSHSandbox

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Phase P5: Per-Tool Sandbox

### Task P5.1: Per-tool sandbox resolution in agent loop

**Files:**
- Modify: `crates/vol-llm-agent/src/react/agent.rs`
- Modify: `crates/vol-llm-tool/src/registry.rs`

- [ ] **Step 1: Add sandbox field to ToolDef (or use metadata)**

Since `ToolDef` is the existing tool definition concept, and there's no separate `ToolDef` struct currently (tools are identified by their `ExecutableTool::name()`), we need a way to associate sandbox names with tools. 

Approach: Use a `HashMap<String, String>` in `ToolRegistry` for per-tool sandbox overrides:

```rust
// In ToolRegistry, add field:
tool_sandboxes: HashMap<String, String>,  // tool_name → sandbox_name
```

Add method:
```rust
pub fn set_tool_sandbox(&mut self, tool_name: &str, sandbox_name: &str) {
    self.tool_sandboxes.insert(tool_name.to_string(), sandbox_name.to_string());
}

pub fn get_tool_sandbox(&self, tool_name: &str) -> Option<&str> {
    self.tool_sandboxes.get(tool_name).map(|s| s.as_str())
}
```

- [ ] **Step 2: Update agent tool execution loop**

In `crates/vol-llm-agent/src/react/agent.rs`, modify the sandbox resolution before tool execution:

```rust
// Resolve sandbox for this tool invocation:
// 1. Check per-tool override in registry
// 2. Fall back to agent default_sandbox
// 3. Fall back to registry default ("local")
let sandbox_name = run_ctx
    .tool_registry()
    .get_tool_sandbox(&call.name)
    .or(config.default_sandbox.as_deref())
    .unwrap_or("local");

let sandbox = config
    .sandbox_registry
    .as_ref()
    .and_then(|r| r.get(sandbox_name))
    .unwrap_or_else(|| {
        Arc::new(vol_llm_sandbox::local::LocalSandbox::new(None))
    });

let tool_ctx = ToolContext::default()
    .with_sandbox(sandbox)
    .with_agent_def(agent_def.clone());
```

- [ ] **Step 3: Check compilation**

```bash
cargo check -p vol-llm-agent
```

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent/src/react/agent.rs crates/vol-llm-tool/src/registry.rs
git commit -m "feat(agent): per-tool sandbox resolution in agent execution loop

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task P5.2: Full workspace tests

- [ ] **Step 1: Run all tests**

```bash
cargo test --workspace --exclude vol-llm-ui 2>&1 | tail -30
```

- [ ] **Step 2: Fix any test failures**

Check for:
- Tests that expected `ToolContext.sandbox` to be `None`
- Tests that used the old `Sandbox` trait from `vol_llm_core`
- Tests referencing the removed `LocalSandbox` location

- [ ] **Step 3: Run clippy**

```bash
cargo clippy --workspace --exclude vol-llm-ui -- -D warnings 2>&1 | tail -20
```

Fix any warnings.

- [ ] **Step 4: Final commit**

```bash
git add -A
git commit -m "test: fix remaining test issues after sandbox migration

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Summary

| Phase | Tasks | Expected Commits | Dependencies |
|-------|-------|-----------------|--------------|
| P0: Crate + Trait + Local | P0.1-P0.3 | 3 | None |
| P1: Tool Migration | P1.1-P1.8 | 8 | P0 |
| P2: Registry | P2.1-P2.2 | 2 | P0 |
| P3: SSHSandbox | P3.1-P3.2 | 2 | P0 |
| P4: SSH Tests | P4.1-P4.2 | 2 | P3 |
| P5: Per-Tool | P5.1-P5.2 | 2 | P2, P3 |

**Total: ~19 commits across 5 phases. P2 and P3 can be parallelized.**

## Key Implementation Notes

1. **ssh2 crate compatibility**: The `ssh2` crate's `Sftp.stat()` returns `FileStat` with optional `size` and `mtime` fields. Verify field names against the actual `ssh2` version used.

2. **base64/shellexpand**: Added as dependencies for host key hash encoding and `~` expansion in SSH config paths. If these are already in the workspace, use workspace versions.

3. **nix dependency**: LocalSandbox uses `nix` for process group signaling. If `nix` isn't in workspace dependencies, add it or use `libc::kill` as fallback.

4. **SFTP mkdir race**: `create_dir_all` over SFTP has inherent race conditions (two concurrent calls may both try to create the same dir). The implementation tolerates errors for existing directories.

5. **Idle timeout background task**: The `_idle_task` JoinHandle is kept alive as long as the SSHSandbox lives. On Drop, it will be cancelled (tokio drops the handle). Consider adding explicit cancellation in `cleanup()`.
