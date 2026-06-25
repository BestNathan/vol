//! Sandbox abstraction for isolated execution environments.
//!
//! All tool I/O goes through the Sandbox trait — tools never call OS APIs directly.
//! Implementations: LocalSandbox (local directory), SSHSandbox (remote host via SSH).

use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

#[cfg(feature = "firecracker")]
pub mod firecracker;
pub mod local;
pub mod registry;
#[cfg(feature = "ssh")]
pub mod ssh;
#[cfg(feature = "wasm")]
pub mod wasm;

/// Reference to a sandbox instance.
pub type SandboxRef = Arc<dyn Sandbox>;

/// Trait for isolated execution environments.
///
/// # Interior Mutability
///
/// All methods take `&self` rather than `&mut self` because `Sandbox` instances are
/// shared via `Arc<dyn Sandbox>` (`SandboxRef`). Implementations that need mutable
/// state (e.g., SSH connection pools) must use interior mutability (`Mutex`,
/// `tokio::sync::RwLock`, etc.).
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
        &self,
        path: &Path,
        offset: Option<u64>,
        limit: Option<u64>,
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
    result
}
