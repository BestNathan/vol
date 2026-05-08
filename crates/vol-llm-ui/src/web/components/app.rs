//! Root App component with state management, event loop, and routing.

use dioxus::prelude::*;
use std::time::Duration;

use crate::state::{ActiveTab, UiEvent, UiState};

use super::approval_dialog::ApprovalDialog;
use super::conversation::ConversationView;
use super::input_area::InputArea;
use super::log_viewer::LogViewer;
use super::session_dialog::SessionDialog;
use super::skills::SkillsPanel;
use super::status_bar::StatusBar;
use super::tools_panel::ToolsPanel;
use super::workspace::WorkspacePanel;

/// Shared application state provided via context.
#[derive(Clone, PartialEq)]
pub struct AppState {
    pub ui_state: Signal<UiState>,
}

impl AppState {
    /// Apply a UiEvent to the shared state.
    pub fn apply_event(&self, event: UiEvent) {
        self.ui_state.write_silent().apply(event);
    }
}

/// Root application component.
///
/// Provides the UiState signal via context and renders the full layout:
/// status bar, tools panel, tab bar, tab content, and input area.
#[component]
pub fn App() -> Element {
    // Initialize UiState with defaults
    let ui_state = use_signal(|| UiState::new("web-session".into(), "/workspace"));

    // Provide state to all child components
    use_context_provider(|| AppState { ui_state });

    rsx! {
        style { {GLOBAL_CSS} }
        div { class: "app-container",
            StatusBar {}
            div { class: "main-layout",
                ToolsPanel {}
                div { class: "right-panel",
                    TabBar {}
                    TabContent {}
                    InputArea {}
                }
            }
            SessionDialog {}
            ApprovalDialog {}
        }
    }
}

/// Tab bar component showing Conversation | Workspace | Skills | Logs.
#[component]
fn TabBar() -> Element {
    let state: AppState = use_context();

    rsx! {
        div { class: "tab-bar",
            TabButton { state: state.clone(), tab: ActiveTab::Conversation, label: "Conversation" }
            TabButton { state: state.clone(), tab: ActiveTab::Workspace, label: "Workspace" }
            TabButton { state: state.clone(), tab: ActiveTab::Skills, label: "Skills" }
            TabButton { state: state.clone(), tab: ActiveTab::Logs, label: "Logs" }
        }
    }
}

#[component]
fn TabButton(state: AppState, tab: ActiveTab, label: String) -> Element {
    let active = state.ui_state.peek().active_tab == tab;
    let tab_class = if active { "tab active" } else { "tab" };
    let mut state_clone = state.clone();

    rsx! {
        button {
            class: tab_class,
            onclick: move |_| {
                state_clone.ui_state.write_silent().active_tab = tab;
            },
            "{label}"
        }
    }
}

/// Tab content router -- renders the panel for the currently active tab.
#[component]
fn TabContent() -> Element {
    let state: AppState = use_context();
    let active = state.ui_state.peek().active_tab;

    match active {
        ActiveTab::Conversation => rsx! { ConversationView {} },
        ActiveTab::Workspace => rsx! { WorkspacePanel {} },
        ActiveTab::Skills => rsx! { SkillsPanel {} },
        ActiveTab::Logs => rsx! { LogViewer {} },
    }
}

/// Helper: format a Duration as MM:SS.
pub fn format_duration(d: Duration) -> String {
    let secs = d.as_secs();
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

/// Helper: format a tool status as a short label.
pub fn status_label(status: crate::state::ToolCallStatus) -> &'static str {
    match status {
        crate::state::ToolCallStatus::Running => "...",
        crate::state::ToolCallStatus::Success => "OK",
        crate::state::ToolCallStatus::Error => "ERR",
        crate::state::ToolCallStatus::Skipped => "SKIP",
    }
}

/// Helper: CSS class for a tool status.
pub fn status_class(status: crate::state::ToolCallStatus) -> &'static str {
    match status {
        crate::state::ToolCallStatus::Running => "status-running",
        crate::state::ToolCallStatus::Success => "status-success",
        crate::state::ToolCallStatus::Error => "status-error",
        crate::state::ToolCallStatus::Skipped => "status-skipped",
    }
}

// === Global CSS ===

const GLOBAL_CSS: &str = r#"
* { margin: 0; padding: 0; box-sizing: border-box; }
body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; font-size: 14px; color: #e0e0e0; background: #1a1a2e; }
.app-container { display: flex; flex-direction: column; height: 100vh; width: 100vw; overflow: hidden; }
.status-bar { display: flex; align-items: center; padding: 4px 12px; background: #2d2d44; color: #e0e0e0; font-size: 12px; font-family: monospace; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; flex-shrink: 0; }
.status-running { color: #f0c040; }
.status-idle { color: #80c080; }
.unsafe-mode { color: #ff4040; font-weight: bold; }
.main-layout { display: flex; flex: 1; overflow: hidden; }
.tools-panel { width: 30%; min-width: 200px; max-width: 400px; border-right: 1px solid #333355; display: flex; flex-direction: column; overflow: hidden; flex-shrink: 0; }
.tools-panel-header { padding: 6px 10px; background: #252540; border-bottom: 1px solid #333355; font-weight: bold; font-size: 12px; color: #80a0ff; flex-shrink: 0; }
.tools-panel-list { flex: 1; overflow-y: auto; padding: 4px 0; }
.tool-item { padding: 6px 10px; border-bottom: 1px solid #2a2a44; }
.tool-item-name { font-weight: bold; font-size: 13px; }
.tool-item-arg { font-size: 11px; color: #888; margin-top: 2px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.tool-item-status { font-size: 11px; margin-left: 8px; padding: 1px 6px; border-radius: 3px; }
.status-running { background: #665500; color: #f0c040; }
.status-success { background: #224422; color: #80c080; }
.status-error { background: #442222; color: #ff6060; }
.status-skipped { background: #333333; color: #888; }
.right-panel { flex: 1; display: flex; flex-direction: column; overflow: hidden; }
.tab-bar { display: flex; background: #252540; border-bottom: 1px solid #333355; flex-shrink: 0; }
.tab { padding: 6px 16px; background: transparent; border: none; color: #888; cursor: pointer; font-size: 13px; border-bottom: 2px solid transparent; }
.tab:hover { color: #ccc; background: #2a2a44; }
.tab.active { color: #e0e0e0; background: #1a1a2e; border-bottom: 2px solid #80a0ff; }
.conversation { flex: 1; overflow-y: auto; padding: 10px; }
.conversation-empty { display: flex; align-items: center; justify-content: center; height: 100%; color: #666; }
.msg { margin-bottom: 10px; padding: 8px 10px; border-radius: 6px; max-width: 100%; word-wrap: break-word; white-space: pre-wrap; }
.msg-user { background: #1a2a44; border-left: 3px solid #4080ff; }
.msg-user-prefix { color: #4080ff; font-weight: bold; }
.msg-thinking { background: #2a2a20; border-left: 3px solid #c0c040; }
.msg-thinking-prefix { color: #c0c040; font-weight: bold; }
.msg-thinking-content { color: #888; margin-top: 4px; padding-left: 4px; }
.msg-streaming { color: #ccc; }
.msg-tool { background: #1a2a3a; border-left: 3px solid #4080c0; }
.msg-tool-name { color: #4080c0; font-weight: bold; }
.msg-tool-arg { color: #888; font-size: 12px; margin-top: 2px; padding-left: 4px; }
.msg-tool-result { background: #1a2a1a; border-left: 3px solid #40c040; }
.msg-tool-result-error { background: #2a1a1a; border-left: 3px solid #c04040; }
.msg-tool-result-prefix { font-weight: bold; }
.msg-tool-result-content { color: #888; font-size: 12px; margin-top: 4px; padding-left: 4px; max-height: 120px; overflow-y: auto; font-family: monospace; }
.msg-answer { color: #e0e0e0; line-height: 1.5; }
.msg-summary { color: #80c080; font-weight: bold; padding: 6px 0; }
.msg-error { color: #ff6060; font-weight: bold; background: #2a1a1a; border-left: 3px solid #c04040; }
.input-area { border-top: 1px solid #333355; padding: 8px 10px; background: #252540; flex-shrink: 0; }
.input-row { display: flex; gap: 8px; }
.input-area textarea { flex: 1; background: #1a1a2e; color: #e0e0e0; border: 1px solid #444466; border-radius: 4px; padding: 6px 8px; font-size: 14px; font-family: inherit; resize: none; min-height: 40px; max-height: 120px; outline: none; }
.input-area textarea:focus { border-color: #80a0ff; }
.input-area textarea:disabled { opacity: 0.5; }
.input-area button { padding: 6px 16px; background: #4060c0; color: #e0e0e0; border: none; border-radius: 4px; cursor: pointer; font-size: 14px; align-self: flex-end; }
.input-area button:hover { background: #5070d0; }
.input-area button:disabled { background: #333355; cursor: not-allowed; }
.input-hint { margin-top: 4px; font-size: 11px; color: #666; }
.input-hint-key { color: #80a0ff; font-weight: bold; }
.input-hint-running { color: #f0c040; }
.workspace-panel { flex: 1; overflow-y: auto; padding: 10px; }
.workspace-empty { display: flex; align-items: center; justify-content: center; height: 100%; color: #666; }
.workspace-entry { padding: 2px 0; font-family: monospace; font-size: 13px; }
.workspace-dir { color: #6090ff; font-weight: bold; }
.workspace-file { color: #e0e0e0; }
.workspace-file-modified { color: #c0c040; }
.skills-panel { flex: 1; overflow-y: auto; padding: 10px; }
.skills-empty { display: flex; align-items: center; justify-content: center; height: 100%; color: #666; }
.skills-table { width: 100%; border-collapse: collapse; }
.skills-table th { text-align: left; padding: 4px 8px; border-bottom: 1px solid #333355; font-size: 12px; color: #888; }
.skills-table td { padding: 4px 8px; font-size: 13px; border-bottom: 1px solid #2a2a44; }
.log-viewer { flex: 1; overflow-y: auto; padding: 10px; }
.log-run-list { font-family: monospace; font-size: 13px; }
.log-run-item { padding: 3px 0; color: #ccc; }
.log-run-item-id { color: #c0c0c0; }
.log-run-item-count { color: #888; }
.log-entry { font-family: monospace; font-size: 12px; padding: 2px 0; white-space: nowrap; }
.log-entry-time { color: #666; }
.log-entry-type { font-weight: bold; }
.log-empty { display: flex; align-items: center; justify-content: center; height: 100%; color: #666; }
.modal-overlay { position: fixed; top: 0; left: 0; right: 0; bottom: 0; background: rgba(0, 0, 0, 0.6); display: flex; align-items: center; justify-content: center; z-index: 100; }
.modal-content { background: #252540; border: 1px solid #444466; border-radius: 8px; padding: 16px; min-width: 400px; max-width: 600px; max-height: 80vh; overflow-y: auto; }
.modal-title { font-size: 16px; font-weight: bold; color: #e0e0e0; margin-bottom: 12px; border-bottom: 1px solid #333355; padding-bottom: 8px; }
.modal-empty { color: #888; padding: 10px 0; }
.modal-session-item { padding: 6px 8px; border-bottom: 1px solid #2a2a44; display: flex; align-items: center; gap: 8px; }
.modal-session-item.selected { background: #2a2a44; }
.modal-session-id { font-family: monospace; color: #e0e0e0; font-weight: bold; }
.modal-session-meta { color: #888; font-size: 12px; }
.modal-actions { margin-top: 12px; display: flex; gap: 8px; padding-top: 8px; border-top: 1px solid #333355; }
.modal-actions button { padding: 6px 12px; border: none; border-radius: 4px; cursor: pointer; font-size: 13px; }
.btn-new { background: #4060c0; color: #e0e0e0; }
.btn-resume { background: #408040; color: #e0e0e0; }
.btn-delete { background: #804040; color: #e0e0e0; }
.btn-cancel { background: #555; color: #e0e0e0; }
.btn-approve { background: #408040; color: #e0e0e0; }
.btn-reject { background: #804040; color: #e0e0e0; }
.btn-stop { background: #662020; color: #e0e0e0; }
.approval-tool-name { color: #f0c040; font-weight: bold; font-size: 15px; }
.approval-reason { color: #ccc; margin: 6px 0; }
.approval-args { font-family: monospace; font-size: 12px; color: #888; background: #1a1a2e; padding: 6px 8px; border-radius: 4px; margin: 8px 0; max-height: 100px; overflow-y: auto; white-space: pre-wrap; }
"#;
