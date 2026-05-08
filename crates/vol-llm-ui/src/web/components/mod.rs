//! UI component re-exports for the Dioxus web frontend.

pub mod app;
pub mod approval_dialog;
pub mod conversation;
pub mod input_area;
pub mod log_viewer;
pub mod session_dialog;
pub mod skills;
pub mod status_bar;
pub mod tools_panel;
pub mod workspace;

pub use app::App;
pub use approval_dialog::ApprovalDialog;
pub use conversation::ConversationView;
pub use input_area::InputArea;
pub use log_viewer::LogViewer;
pub use session_dialog::SessionDialog;
pub use skills::SkillsPanel;
pub use status_bar::StatusBar;
pub use tools_panel::ToolsPanel;
pub use workspace::WorkspacePanel;
