/// Shared state model for all UI frontends.
pub mod state;

/// Connection abstraction layer (local + remote).
pub mod connection;

/// Async hooks for agent interaction.
pub mod hooks;

// Re-export commonly used types at crate root.
pub use state::{
    UiState, UiEvent, ConversationEntry, ToolCallEntry, ToolCallStatus,
    WorkspaceTree, WorkspaceEntry, ActiveTab, ApprovalState,
    SkillDisplayEntry, LogLine, LogRunSummary, SessionDialogEntry,
};

pub use connection::{AgentConnection, FileOperations, FileEntry, LogRunInfo, SessionInfo};
