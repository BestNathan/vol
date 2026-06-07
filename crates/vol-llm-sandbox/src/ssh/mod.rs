//! SSH sandbox implementation.
//!
//! Sub-modules:
//! - `session`: SSH session lifecycle (connect, auth, host key verification, disconnect)

use std::path::{Path, PathBuf};
use async_trait::async_trait;

use crate::registry::SshConfig;
use crate::{Sandbox, SandboxError, SandboxResult, CommandRequest, CommandOutput, DirEntry, FileMetadata};

pub mod session;

/// SSH sandbox — P3.2 will implement the full Sandbox trait.
/// For now, this is a stub that returns Err for every operation
/// except `new()`, `kind()`, and `name()`.
pub struct SSHSandbox {
    _name: String,
    _work_dir: Option<String>,
    _config: SshConfig,
}

impl SSHSandbox {
    pub(crate) fn new(name: String, work_dir: Option<String>, config: SshConfig) -> SandboxResult<Self> {
        Ok(Self { _name: name, _work_dir: work_dir, _config: config })
    }
}

#[async_trait]
impl Sandbox for SSHSandbox {
    fn kind(&self) -> &str { "ssh" }
    fn name(&self) -> &str { &self._name }
    fn root_path(&self) -> &Path { Path::new("") }

    async fn start(&self) -> SandboxResult<()> {
        Err(SandboxError::UnknownType("SSH sandbox not yet implemented (P3.2)".into()))
    }
    async fn cleanup(&self) -> SandboxResult<()> { Ok(()) }
    fn resolve_path(&self, _rel: &str) -> SandboxResult<PathBuf> {
        Err(SandboxError::UnknownType("SSH sandbox not yet implemented (P3.2)".into()))
    }
    async fn execute(&self, _req: CommandRequest) -> SandboxResult<CommandOutput> {
        Err(SandboxError::UnknownType("SSH sandbox not yet implemented (P3.2)".into()))
    }
    async fn read_file(&self, _path: &Path, _offset: Option<u64>, _limit: Option<u64>) -> SandboxResult<Vec<u8>> {
        Err(SandboxError::UnknownType("SSH sandbox not yet implemented (P3.2)".into()))
    }
    async fn write_file(&self, _path: &Path, _content: &[u8]) -> SandboxResult<()> {
        Err(SandboxError::UnknownType("SSH sandbox not yet implemented (P3.2)".into()))
    }
    async fn create_dir_all(&self, _path: &Path) -> SandboxResult<()> {
        Err(SandboxError::UnknownType("SSH sandbox not yet implemented (P3.2)".into()))
    }
    async fn read_dir(&self, _path: &Path) -> SandboxResult<Vec<DirEntry>> {
        Err(SandboxError::UnknownType("SSH sandbox not yet implemented (P3.2)".into()))
    }
    async fn metadata(&self, _path: &Path) -> SandboxResult<FileMetadata> {
        Err(SandboxError::UnknownType("SSH sandbox not yet implemented (P3.2)".into()))
    }
}
