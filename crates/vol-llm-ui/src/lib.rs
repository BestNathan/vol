/// Shared state model for all UI frontends.
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
    UiState, UiEvent, ConversationEntry, ToolCallEntry, ToolCallStatus,
    WorkspaceTree, WorkspaceEntry, ActiveTab, ApprovalState,
    SkillDisplayEntry, LogRunSummary, SessionDialogEntry,
};

#[cfg(feature = "tui")]
pub use connection::{AgentConnection, FileOperations, FileEntry, LogRunInfo, SessionInfo};
#[cfg(feature = "tui")]
pub use connection::local::LocalConnection;
#[cfg(feature = "tui")]
pub use connection::remote::RemoteConnection;
