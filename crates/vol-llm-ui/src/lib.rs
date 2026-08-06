/// Shared state model for all UI frontends.
///
/// # Deprecation
///
/// The Rust/Dioxus web frontend (`web` feature, `crates/vol-llm-ui/`) is **deprecated**
/// as of 2026-08-06. The active web UI is the React frontend at `frontend/`.
///
/// The TUI frontend (`tui` feature) and shared `state` module remain maintained.
pub mod state;

// TUI-only modules
#[cfg(feature = "tui")]
pub mod connection;

#[cfg(feature = "tui")]
pub mod hooks;

// TUI frontend (ratatui).
#[cfg(feature = "tui")]
pub mod tui;

// Web frontend (dioxus).
#[cfg(feature = "web")]
pub mod web;

// Re-export commonly used types at crate root.
pub use state::{
    ActiveTab, ApprovalState, ConversationEntry, LogRunSummary, SessionDialogEntry,
    SkillDisplayEntry, ToolCallEntry, ToolCallStatus, UiEvent, UiState, WorkspaceTreeNode,
};

#[cfg(feature = "tui")]
pub use connection::local::LocalConnection;
#[cfg(feature = "tui")]
pub use connection::remote::RemoteConnection;
#[cfg(feature = "tui")]
pub use connection::{AgentConnection, FileEntry, FileOperations, LogRunInfo, SessionInfo};
