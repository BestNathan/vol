//! Sandbox abstraction for isolated execution environments.
//!
//! All tool I/O goes through the Sandbox trait — tools never call OS APIs directly.
//! Implementations: LocalSandbox (local directory), SSHSandbox (remote host via SSH).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use ulid::Ulid;

#[cfg(feature = "firecracker")]
pub mod firecracker;
pub mod local;
pub mod manager;
pub mod provider;
pub mod registry;
pub mod spec;
#[cfg(feature = "ssh")]
pub mod ssh;
pub mod store;
pub mod tmp;
#[cfg(feature = "wasm")]
pub mod wasm;

pub use manager::SandboxManager;
pub use provider::{BackendSandboxRef, SandboxInfo, SandboxProvider};
pub use spec::{SandboxProviderConfig, SandboxSpec};
pub use store::{InMemorySandboxStore, SandboxFilter, SandboxRecord, SandboxStore};

/// Stable instance identifier, distinct from profile name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SandboxId(String);

impl SandboxId {
    pub fn new() -> Self {
        Self(format!("sb_{}", Ulid::new()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SandboxId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for SandboxId {
    fn default() -> Self {
        Self::new()
    }
}

/// Explicit lifecycle states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxStatus {
    Creating,
    Created,
    Starting,
    Running,
    Pausing,
    Paused,
    Stopping,
    Stopped,
    Destroying,
    Destroyed,
    Failed,
}

/// Discoverable backend capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxCapabilities {
    pub persistent: bool,
    pub pausable: bool,
    pub stoppable: bool,
    pub destroyable: bool,
}

/// Reference to a sandbox instance.
pub type SandboxRef = Arc<dyn Sandbox>;

/// Trait for isolated execution environments.
///
/// All tool I/O MUST go through sandbox methods — tools never call OS APIs directly.
/// Implementations: LocalSandbox (local directory), SSHSandbox (remote via SSH),
/// FirecrackerSandbox (microVM), WasmSandbox (WASI runtime).
///
/// # Path handling contract
///
/// - **`root_path()`**: returns the absolute root of the sandbox filesystem.
///   All sandbox file operations are scoped to this directory.
/// - **`resolve_path(rel)`**: validates that `rel` (a **relative** path string
///   from tool arguments) is safe and resolves it to an absolute path within
///   `root_path()`. MUST return `PathTraversal` if the resolved path escapes
///   the root. Implementations MUST reject absolute paths (they start with `/`).
///   Tools that receive absolute user input must convert it before calling
///   `resolve_path` — use `ToolContext::resolve_path` which handles this.
/// - **`read_file / write_file / create_dir_all / read_dir / metadata`**: take an
///   already-resolved absolute path (output of `resolve_path`). The caller is
///   responsible for calling `resolve_path` first to validate.
///
/// # Interior Mutability
///
/// All methods take `&self` rather than `&mut self` because `Sandbox` instances are
/// shared via `Arc<dyn Sandbox>` (`SandboxRef`). Implementations that need mutable
/// state (e.g., SSH connection pools) must use interior mutability (`Mutex`,
/// `tokio::sync::RwLock`, etc.).
#[async_trait]
pub trait Sandbox: Send + Sync {
    /// Stable instance identifier.
    fn id(&self) -> &SandboxId;

    /// Sandbox type identifier: "local", "ssh", "firecracker", "wasm".
    fn kind(&self) -> &str;

    /// Current lifecycle status.
    fn status(&self) -> SandboxStatus;

    /// Absolute root path of the sandbox. All file operations are scoped to this.
    /// Returns None if the sandbox is not yet initialized or has been destroyed.
    fn root_path(&self) -> Option<&Path>;

    /// Validate a **relative** path and resolve it to an absolute path within
    /// `root_path()`. Returns `PathTraversal` if the resolved path escapes the root.
    ///
    /// # Contract (all implementations MUST follow)
    ///
    /// - Accepts: relative paths (`"foo/bar.txt"`, `"./foo"`, `"."`)
    /// - Rejects: absolute paths (anything starting with `/` or `~`)
    /// - Rejects: paths containing `..` that escape `root_path()`
    /// - Normalizes: `.` and redundant separators are resolved
    ///
    /// Tools calling this should use `ToolContext::resolve_path` which
    /// handles absolute→relative conversion transparently.
    fn resolve_path(&self, rel: &str) -> SandboxResult<PathBuf>;

    /// Execute a command inside the sandbox. `req.program` is the binary name,
    /// `req.args` are the arguments, `req.env` are environment variables.
    /// The command runs with `root_path()` as its working directory.
    async fn execute(&self, req: CommandRequest) -> SandboxResult<CommandOutput>;

    /// Read file content as raw bytes at the given absolute path.
    /// The caller MUST validate the path via `resolve_path` first.
    /// `offset` and `limit` are byte-level, applied after reading.
    async fn read_file(
        &self,
        path: &Path,
        offset: Option<u64>,
        limit: Option<u64>,
    ) -> SandboxResult<Vec<u8>>;

    /// Write bytes to a file at the given absolute path.
    /// The caller MUST validate the path via `resolve_path` first.
    /// Parent directories are created automatically if they don't exist.
    async fn write_file(&self, path: &Path, content: &[u8]) -> SandboxResult<()>;

    /// Create directory and all parents at the given absolute path.
    /// The caller MUST validate the path via `resolve_path` first.
    async fn create_dir_all(&self, path: &Path) -> SandboxResult<()>;

    /// List entries in a directory at the given absolute path.
    /// The caller MUST validate the path via `resolve_path` first.
    async fn read_dir(&self, path: &Path) -> SandboxResult<Vec<DirEntry>>;

    /// Get file metadata at the given absolute path.
    /// The caller MUST validate the path via `resolve_path` first.
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

/// The type of a filesystem entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileType {
    File,
    Directory,
    Symlink,
    Other,
}

/// Directory entry returned by `read_dir`.
#[derive(Debug, Clone)]
pub struct DirEntry {
    pub name: String,
    pub file_type: FileType,
}

/// File metadata returned by `metadata`.
#[derive(Debug, Clone)]
pub struct FileMetadata {
    pub size: u64,
    pub mtime: u64, // unix timestamp, milliseconds
    pub file_type: FileType,
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

    #[cfg(feature = "firecracker")]
    #[error("Firecracker error: {0}")]
    Firecracker(String),

    #[cfg(feature = "wasm")]
    #[error("Wasm error: {0}")]
    Wasm(String),

    #[error("Command timed out after {0:?}")]
    Timeout(Duration),

    #[error("Unknown sandbox type: {0}")]
    UnknownType(String),

    #[error("Sandbox '{0}' already registered")]
    DuplicateName(String),

    #[error("Local sandbox cannot be overridden by config")]
    LocalOverride,

    #[error("Config error: {0}")]
    Config(String),

    #[error("Sandbox not found: {0}")]
    NotFound(String),

    #[error("Invalid state transition: {from:?} -> {to:?}")]
    InvalidTransition {
        from: SandboxStatus,
        to: SandboxStatus,
    },
}

pub type SandboxResult<T> = Result<T, SandboxError>;

/// Normalize a path by resolving `.` and `..` components without touching the filesystem.
pub fn normalize_path(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                result.pop();
            }
            std::path::Component::CurDir => {}
            _ => result.push(component),
        }
    }
    // Guard: an all-CurDir path like "." normalizes to empty, which breaks
    // read_dir/read_file. Fall back to "." so relative resolution works.
    if result.as_os_str().is_empty() && !path.as_os_str().is_empty() {
        result.push(".");
    }
    result
}
