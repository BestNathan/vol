//! Sandbox abstraction for isolated code execution.

use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Sandbox error type
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
}

/// Result type for sandbox operations
pub type SandboxResult<T> = Result<T, SandboxError>;

/// Sandbox trait — abstract interface for isolated execution environments.
///
/// Implementations: LocalSandbox (local directory), DockerSandbox (container),
/// SSHSandbox (remote host), etc.
pub trait Sandbox: Send + Sync {
    /// Sandbox type identifier (for logging/debugging)
    fn kind(&self) -> &str;

    /// Start the sandbox (create directory, establish connection, etc.)
    fn start(&self) -> SandboxResult<()>;

    /// Clean up the sandbox (delete temp directory, disconnect, etc.)
    fn cleanup(&self) -> SandboxResult<()>;

    /// Root path of the sandbox
    fn root_path(&self) -> &Path;

    /// Resolve a relative path to an absolute path within the sandbox.
    /// Returns an error if the resolved path escapes the sandbox root.
    fn resolve_path(&self, rel: &str) -> SandboxResult<PathBuf>;
}

/// Type alias for convenience
pub type SandboxRef = Arc<dyn Sandbox>;
