use crate::{Sandbox, SandboxCapabilities, SandboxResult, SandboxSpec};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Reference to a backend-specific sandbox instance.
#[derive(Clone)]
pub struct BackendSandboxRef {
    pub backend_id: String,
    pub sandbox: Arc<dyn Sandbox>,
}

impl std::fmt::Debug for BackendSandboxRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BackendSandboxRef")
            .field("backend_id", &self.backend_id)
            .field("sandbox", &"<dyn Sandbox>")
            .finish()
    }
}

/// Information about a sandbox instance returned by list operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxInfo {
    pub id: String,
    pub profile: String,
    pub kind: String,
    pub status: String,
    pub root_path: Option<String>,
    pub capabilities: SandboxCapabilities,
}

/// Backend lifecycle adapter trait.
///
/// Each sandbox backend (local, tmp, ssh, docker, kubernetes, etc.) implements
/// this trait to provide lifecycle management for its sandbox instances.
#[async_trait]
pub trait SandboxProvider: Send + Sync {
    /// Returns the kind identifier for this provider (e.g., "local", "ssh").
    fn kind(&self) -> &str;

    /// Returns the capabilities supported by this provider.
    fn capabilities(&self) -> SandboxCapabilities;

    /// Create a new sandbox instance from a spec.
    ///
    /// Returns a BackendSandboxRef containing the backend-specific identifier
    /// and the sandbox handle.
    async fn create(&self, spec: &SandboxSpec) -> SandboxResult<BackendSandboxRef>;

    /// Get an existing sandbox instance by its backend-specific identifier.
    async fn get(&self, backend_id: &str) -> SandboxResult<Arc<dyn Sandbox>>;

    /// List all sandbox instances managed by this provider.
    async fn list(&self) -> SandboxResult<Vec<SandboxInfo>>;

    /// Start a sandbox instance (transition from Created/Stopped to Running).
    async fn start(&self, backend_id: &str) -> SandboxResult<()>;

    /// Pause a running sandbox instance (transition from Running to Paused).
    async fn pause(&self, backend_id: &str) -> SandboxResult<()>;

    /// Resume a paused sandbox instance (transition from Paused to Running).
    async fn resume(&self, backend_id: &str) -> SandboxResult<()>;

    /// Stop a sandbox instance (transition from Running/Paused to Stopped).
    async fn stop(&self, backend_id: &str) -> SandboxResult<()>;

    /// Destroy a sandbox instance and release its resources.
    async fn destroy(&self, backend_id: &str) -> SandboxResult<()>;
}
