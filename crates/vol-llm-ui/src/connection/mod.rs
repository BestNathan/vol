//! Connection abstraction for agent interaction.
//!
//! ## Architecture
//!
//! `AgentConnection` abstracts over two implementations:
//! - `LocalConnection` — in-process ReActAgent, events via EventObserver
//! - `RemoteConnection` — JSON-RPC WebSocket via jsonrpsee
//!
//! `FileOperations` is a separate trait for filesystem/log/session access.
//! In local mode this is direct filesystem access; in remote mode it goes
//! through the agent service's JSON-RPC endpoints.
//!
//! A single concrete type (e.g., `LocalConnection`) implements **both**
//! `AgentConnection` and `FileOperations`, so callers can use a single
//! struct for all operations.

use crate::state::UiEvent;
use async_trait::async_trait;
use tokio::sync::mpsc;

/// Abstract connection to an agent — local or remote.
///
/// The UI layer interacts with this trait without knowing whether the
/// agent runs in-process (local) or on a remote server (JSON-RPC over WS).
#[async_trait]
pub trait AgentConnection: Send + Sync {
    /// Submit user input. Returns a receiver for UiEvents from the agent run.
    async fn submit(&self, input: String) -> anyhow::Result<mpsc::Receiver<UiEvent>>;

    /// Request tool approval/denial.
    async fn approve_tool(
        &self,
        req_id: String,
        approved: bool,
        reason: Option<String>,
    ) -> anyhow::Result<()>;

    /// Cancel the current agent run.
    async fn cancel(&self, req_id: String) -> anyhow::Result<()>;

    /// Whether the connection is currently active.
    fn is_connected(&self) -> bool;
}

/// A file or directory entry for workspace browsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
}

/// Summary of a logged run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogRunInfo {
    pub run_id: String,
    pub timestamp: String,
    pub event_count: usize,
}

/// Summary of a saved session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionInfo {
    pub session_id: String,
    pub entry_count: usize,
    pub created_at: String,
}

/// File operations — used by both local (direct fs) and remote (JSON-RPC) modes.
#[async_trait]
pub trait FileOperations: Send + Sync {
    /// List directory contents.
    async fn list_files(&self, path: &str) -> anyhow::Result<Vec<FileEntry>>;

    /// Read a file's contents.
    async fn read_file(&self, path: &str) -> anyhow::Result<String>;

    /// List available log runs.
    async fn list_logs(&self) -> anyhow::Result<Vec<LogRunInfo>>;

    /// List available sessions.
    async fn list_sessions(&self) -> anyhow::Result<Vec<SessionInfo>>;
}

pub mod local;
pub mod remote;
