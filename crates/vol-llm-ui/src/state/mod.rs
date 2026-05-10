mod workspace;

pub use workspace::scan_workspace;

#[cfg(feature = "tui")]
mod event_buffer;

#[cfg(feature = "tui")]
pub use event_buffer::EventBuffer;

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[cfg(all(feature = "web", not(feature = "tui")))]
use web_time::Instant;
#[cfg(feature = "tui")]
use std::time::Instant;

// === Unified Event Type ======================================================

/// All agent and UI events flow through this type.
/// Local mode: AgentStreamEvent → UiEvent (via EventBuffer, implemented later).
/// Remote mode: JSON-RPC notification → UiEvent (deserialized from WS).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UiEvent {
    // Agent lifecycle
    AgentStart { input: String },
    AgentComplete { response: String },
    AgentAborted { reason: String },
    AgentError { message: String },

    // Thinking
    ThinkingStart,
    ThinkingDelta { delta: String },
    ThinkingComplete,

    // Content
    ContentStart,
    ContentDelta { delta: String },
    ContentComplete { content: String },

    // Tools
    ToolCallBegin { tool_name: String, arguments: String },
    ToolCallArgumentDelta { delta: String },
    ToolCallComplete { tool_name: String, result: String, duration_ms: Option<u64> },
    ToolCallError { tool_name: String, error: String, duration_ms: Option<u64> },
    ToolCallSkipped { tool_name: String, reason: String, duration_ms: Option<u64> },

    // Iteration
    MaxIterationsReached { current: u32, max: u32 },
    IterationContinued { from_iteration: u32 },
    IterationComplete { iteration: u32, final_answer: Option<String> },

    // HITL
    ApprovalRequest { tool_name: String, reason: String, arguments: String },
    ApprovalResolved { approved: bool },
}

// === Display Types ===========================================================

#[derive(Debug, Clone)]
pub enum ToolCallStatus { Running, Success, Error, Skipped }

#[derive(Debug, Clone)]
pub struct ToolCallEntry {
    pub sequence: u32,
    pub tool_name: String,
    pub arg_preview: String,
    pub status: ToolCallStatus,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone)]
pub enum ConversationEntry {
    UserInput { text: String },
    Thinking { content: String },
    ContentStreaming { content: String },
    ToolCall { tool_name: String, arg_preview: String },
    ToolResult { tool_name: String, preview: String, success: bool },
    AgentAnswer { text: String },
    RunSummary { iterations: u32, tool_calls: u32, elapsed_ms: u128 },
    Error { message: String },
}

#[derive(Debug, Clone)]
pub struct WorkspaceTree {
    pub root: String,
    pub entries: Vec<WorkspaceEntry>,
}

#[derive(Debug, Clone)]
pub struct WorkspaceEntry {
    pub path: String,
    pub is_dir: bool,
    pub modified: bool,
    pub indent: usize,
}

#[derive(Debug, Clone)]
pub struct SkillDisplayEntry {
    pub name: String,
    pub version: String,
    pub scope: String,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct OpenFileTab {
    pub path: String,
    pub content: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ActiveTab { Conversation, Tools, Workspace, Skills, Logs }

impl ActiveTab {
    pub fn toggle(self) -> Self {
        match self {
            ActiveTab::Conversation => ActiveTab::Tools,
            ActiveTab::Tools => ActiveTab::Workspace,
            ActiveTab::Workspace => ActiveTab::Skills,
            ActiveTab::Skills => ActiveTab::Logs,
            ActiveTab::Logs => ActiveTab::Conversation,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SessionDialogEntry {
    pub session_id: String,
    pub entry_count: usize,
    pub age_label: String,
}

// === ApprovalState ===========================================================

pub struct ApprovalState {
    pub tool_name: Option<String>,
    pub reason: Option<String>,
    pub arguments: Option<String>,
    pub response: Option<(bool, Option<String>)>,
}

impl ApprovalState {
    pub fn new() -> Self {
        Self {
            tool_name: None,
            reason: None,
            arguments: None,
            response: None,
        }
    }

    pub fn has_pending(&self) -> bool {
        self.tool_name.is_some()
    }

    pub fn clear(&mut self) {
        self.tool_name = None;
        self.reason = None;
        self.arguments = None;
        self.response = None;
    }
}

// === Log Types ===============================================================

#[derive(Debug, Clone)]
pub struct LogLine {
    pub event_type: String,
    pub summary: String,
    pub timestamp: String,
}

#[derive(Debug, Clone)]
pub struct LogRunSummary {
    pub run_id: String,
    pub event_count: usize,
    pub last_event: String,
    pub last_event_time: String,
}

// === UiState =================================================================

pub struct UiState {
    pub session_id: String,
    pub run_count: u32,
    pub iteration: u32,
    pub tool_call_count: u32,
    pub run_start: Option<Instant>,
    pub run_elapsed: std::time::Duration,
    pub is_running: bool,
    pub exiting: bool,
    pub conversation: Vec<ConversationEntry>,
    pub tool_calls: Vec<ToolCallEntry>,
    pub workspace: WorkspaceTree,
    pub modified_files: HashSet<String>,
    pub active_tab: ActiveTab,
    pub conversation_scroll: u16,
    pub workspace_scroll: u16,
    pub tools_scroll: u16,
    pub conversation_auto_scroll: bool,
    pub approval_state: ApprovalState,
    pub session_dialog_open: bool,
    pub session_dialog_sessions: Vec<SessionDialogEntry>,
    pub session_dialog_selected: usize,
    pub log_viewer_selected_run: Option<String>,
    pub log_viewer_entries: Vec<LogLine>,
    pub log_viewer_scroll: u16,
    pub log_viewer_auto_scroll: bool,
    pub log_viewer_run_logs: Vec<LogRunSummary>,
    pub skills: Vec<SkillDisplayEntry>,
    pub unsafe_mode: bool,
    pub last_error: Option<String>,
    pub ws_url: String,
    pub ws_connected: bool,
    pub ws_last_error: Option<String>,
    pub open_files: Vec<OpenFileTab>,
    pub selected_file_tab: Option<usize>,
    pub collapsed_dirs: HashSet<String>,
    pub expanded_tool_calls: HashSet<usize>,
}

impl UiState {
    pub fn new(session_id: String, working_dir: &str, url: &str) -> Self {
        Self {
            session_id,
            run_count: 0,
            iteration: 0,
            tool_call_count: 0,
            run_start: None,
            run_elapsed: std::time::Duration::ZERO,
            is_running: false,
            exiting: false,
            conversation: Vec::new(),
            tool_calls: Vec::new(),
            workspace: WorkspaceTree {
                root: working_dir.to_string(),
                entries: Vec::new(),
            },
            modified_files: HashSet::new(),
            active_tab: ActiveTab::Conversation,
            conversation_scroll: 0,
            workspace_scroll: 0,
            tools_scroll: 0,
            conversation_auto_scroll: true,
            approval_state: ApprovalState::new(),
            session_dialog_open: false,
            session_dialog_sessions: Vec::new(),
            session_dialog_selected: 0,
            log_viewer_selected_run: None,
            log_viewer_entries: Vec::new(),
            log_viewer_scroll: 0,
            log_viewer_auto_scroll: true,
            log_viewer_run_logs: Vec::new(),
            skills: Vec::new(),
            unsafe_mode: false,
            last_error: None,
            ws_url: url.to_string(),
            ws_connected: false,
            ws_last_error: None,
            open_files: Vec::new(),
            selected_file_tab: None,
            collapsed_dirs: HashSet::new(),
            expanded_tool_calls: HashSet::new(),
        }
    }

    pub fn reset_for_run(&mut self) {
        self.iteration = 0;
        self.tool_call_count = 0;
        self.run_start = Some(Instant::now());
        self.run_elapsed = std::time::Duration::ZERO;
        self.tool_calls.clear();
        self.modified_files.clear();
        self.tools_scroll = 0;
        self.run_count += 1;
    }

    /// Apply a UiEvent to mutate state.
    pub fn apply(&mut self, event: UiEvent) {
        match event {
            UiEvent::AgentStart { input } => {
                self.reset_for_run();
                self.is_running = true;
                self.conversation.push(ConversationEntry::UserInput { text: input });
            }
            UiEvent::AgentComplete { response } => {
                self.flush_pending_content();
                let elapsed = self.run_start.map(|s| s.elapsed()).unwrap_or_default();
                self.run_elapsed = elapsed;
                self.conversation.push(ConversationEntry::RunSummary {
                    iterations: self.iteration,
                    tool_calls: self.tool_call_count,
                    elapsed_ms: elapsed.as_millis(),
                });
                if !response.is_empty() {
                    self.conversation.push(ConversationEntry::AgentAnswer { text: response });
                }
                self.is_running = false;
            }
            UiEvent::AgentAborted { reason } => {
                self.flush_pending_content();
                let elapsed = self.run_start.map(|s| s.elapsed()).unwrap_or_default();
                self.run_elapsed = elapsed;
                self.conversation.push(ConversationEntry::Error { message: reason });
                self.is_running = false;
            }
            UiEvent::AgentError { message } => {
                self.flush_pending_content();
                let elapsed = self.run_start.map(|s| s.elapsed()).unwrap_or_default();
                self.run_elapsed = elapsed;
                self.conversation.push(ConversationEntry::Error { message });
                self.is_running = false;
            }
            UiEvent::ThinkingStart => {
                self.conversation.push(ConversationEntry::Thinking { content: String::new() });
            }
            UiEvent::ThinkingDelta { delta } => {
                if let Some(ConversationEntry::Thinking { content }) = self.conversation.last_mut() {
                    content.push_str(&delta);
                }
            }
            UiEvent::ThinkingComplete => {
                // No-op — thinking content already streamed via deltas
            }
            UiEvent::ContentStart => {
                self.conversation.push(ConversationEntry::ContentStreaming { content: String::new() });
            }
            UiEvent::ContentDelta { delta } => {
                if let Some(ConversationEntry::ContentStreaming { content }) = self.conversation.last_mut() {
                    content.push_str(&delta);
                }
            }
            UiEvent::ContentComplete { content } => {
                if let Some(ConversationEntry::ContentStreaming { .. }) = self.conversation.last() {
                    let entry = self.conversation.last_mut().unwrap();
                    *entry = ConversationEntry::AgentAnswer { text: content };
                } else if !content.is_empty() {
                    self.conversation.push(ConversationEntry::AgentAnswer { text: content });
                }
            }
            UiEvent::ToolCallBegin { tool_name, arguments } => {
                let seq = self.tool_call_count + 1;
                self.tool_call_count = seq;
                let preview = extract_arg_preview(&arguments);
                self.tool_calls.push(ToolCallEntry {
                    sequence: seq,
                    tool_name: tool_name.clone(),
                    arg_preview: preview.clone(),
                    status: ToolCallStatus::Running,
                    duration_ms: None,
                });
                self.conversation.push(ConversationEntry::ToolCall {
                    tool_name,
                    arg_preview: preview,
                });
            }
            UiEvent::ToolCallArgumentDelta { delta: _ } => {
                // Invisible in UI
            }
            UiEvent::ToolCallComplete { tool_name, result, duration_ms } => {
                self.update_tool_call_status(&tool_name, ToolCallStatus::Success, duration_ms);
                let preview = truncate_preview(&result, 200);
                self.conversation.push(ConversationEntry::ToolResult {
                    tool_name,
                    preview,
                    success: true,
                });
            }
            UiEvent::ToolCallError { tool_name, error, duration_ms } => {
                self.update_tool_call_status(&tool_name, ToolCallStatus::Error, duration_ms);
                self.conversation.push(ConversationEntry::ToolResult {
                    tool_name,
                    preview: error,
                    success: false,
                });
            }
            UiEvent::ToolCallSkipped { tool_name, reason, duration_ms } => {
                self.update_tool_call_status(&tool_name, ToolCallStatus::Skipped, duration_ms);
                self.conversation.push(ConversationEntry::ToolResult {
                    tool_name,
                    preview: reason,
                    success: false,
                });
            }
            UiEvent::MaxIterationsReached { current, max } => {
                self.conversation.push(ConversationEntry::Error {
                    message: format!("Max iterations reached ({}/{}) — waiting for user decision...", current, max),
                });
            }
            UiEvent::IterationContinued { from_iteration } => {
                self.iteration = from_iteration;
                self.conversation.push(ConversationEntry::AgentAnswer {
                    text: format!("Continuing from iteration {} (counter reset to 0)", from_iteration),
                });
            }
            UiEvent::IterationComplete { iteration, final_answer } => {
                self.iteration = iteration;
                if let Some(answer) = final_answer {
                    self.conversation.push(ConversationEntry::AgentAnswer { text: answer });
                }
            }
            UiEvent::ApprovalRequest { tool_name, reason, arguments } => {
                self.approval_state.tool_name = Some(tool_name);
                self.approval_state.reason = Some(reason);
                self.approval_state.arguments = Some(arguments);
            }
            UiEvent::ApprovalResolved { approved: _ } => {
                self.approval_state.clear();
            }
        }

        // Auto-scroll
        if self.conversation_auto_scroll {
            self.conversation_scroll = 0;
        }
        self.tools_scroll = self.tool_calls.len() as u16;
    }

    fn flush_pending_content(&mut self) {
        if let Some(ConversationEntry::ContentStreaming { content }) = self.conversation.last() {
            let text = content.clone();
            if !text.is_empty() {
                let entry = self.conversation.last_mut().unwrap();
                *entry = ConversationEntry::AgentAnswer { text };
            }
        }
    }

    fn update_tool_call_status(&mut self, tool_name: &str, status: ToolCallStatus, duration_ms: Option<u64>) {
        // Match the most recent running entry for this tool by sequence (last-written wins).
        for entry in self.tool_calls.iter_mut().rev() {
            if entry.tool_name == tool_name && matches!(entry.status, ToolCallStatus::Running) {
                entry.status = status;
                entry.duration_ms = duration_ms;
                break;
            }
        }
    }

    /// Compute current elapsed time (works mid-run and after completion).
    pub fn elapsed(&self) -> std::time::Duration {
        if self.is_running {
            self.run_start.map(|s| s.elapsed()).unwrap_or(self.run_elapsed)
        } else {
            self.run_elapsed
        }
    }
}

// === Helpers =================================================================

fn extract_arg_preview(arguments: &str) -> String {
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(arguments) {
        if let Some(cmd) = parsed.get("command").and_then(|v| v.as_str()) {
            if cmd.chars().count() > 80 {
                return format!("Command: {}...", cmd.chars().take(77).collect::<String>());
            }
            return format!("Command: {}", cmd);
        }
        if let Some(path) = parsed.get("path").and_then(|v| v.as_str()) {
            return format!("Path: {}", path);
        }
        if let Some(file_path) = parsed.get("file_path").and_then(|v| v.as_str()) {
            return format!("File: {}", file_path);
        }
        if arguments.chars().count() > 80 {
            return format!("Args: {}...", arguments.chars().take(77).collect::<String>());
        }
        return format!("Args: {}", arguments);
    }
    String::new()
}

fn truncate_preview(s: &str, max_chars: usize) -> String {
    let total_chars = s.chars().count();
    if total_chars <= max_chars {
        return s.to_string();
    }
    let truncated: String = s.chars().take(max_chars).collect();
    format!("{}...", truncated)
}

// === Tests ===================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ui_event_agent_start_serializes() {
        let event = UiEvent::AgentStart { input: "hello".into() };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""type":"agent_start""#));
        assert!(json.contains(r#""input":"hello""#));
    }

    #[test]
    fn test_ui_event_tool_call_begin_serializes() {
        let event = UiEvent::ToolCallBegin {
            tool_name: "bash".into(),
            arguments: r#"{"cmd":"ls"}"#.into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""type":"tool_call_begin""#));
        assert!(json.contains(r#""tool_name":"bash""#));
    }

    #[test]
    fn test_ui_event_deserializes_from_remote() {
        let json = r#"{"type":"content_complete","content":"The answer is 42."}"#;
        let event: UiEvent = serde_json::from_str(json).unwrap();
        match event {
            UiEvent::ContentComplete { content } => assert_eq!(content, "The answer is 42."),
            _ => panic!("Expected ContentComplete"),
        }
    }

    #[test]
    fn test_ui_state_new_initializes() {
        let state = UiState::new("test-session".into(), "/tmp/test", "ws://localhost:3001/ws");
        assert_eq!(state.session_id, "test-session");
        assert_eq!(state.run_count, 0);
        assert!(state.conversation.is_empty());
        assert!(state.tool_calls.is_empty());
        assert!(!state.is_running);
    }

    #[test]
    fn test_ui_state_apply_agent_start() {
        let mut state = UiState::new("sess-1".into(), ".", "local");
        state.apply(UiEvent::AgentStart { input: "fix the bug".into() });
        assert!(state.is_running);
        assert_eq!(state.run_count, 1);
        assert_eq!(state.conversation.len(), 1);
        match &state.conversation[0] {
            ConversationEntry::UserInput { text } => assert_eq!(text, "fix the bug"),
            _ => panic!("Expected UserInput"),
        }
    }

    #[test]
    fn test_ui_state_apply_thinking_deltas() {
        let mut state = UiState::new("sess-1".into(), ".", "local");
        state.apply(UiEvent::ThinkingStart);
        state.apply(UiEvent::ThinkingDelta { delta: "Let me ".into() });
        state.apply(UiEvent::ThinkingDelta { delta: "think...".into() });
        state.apply(UiEvent::ThinkingComplete);
        assert_eq!(state.conversation.len(), 1);
        match &state.conversation[0] {
            ConversationEntry::Thinking { content } => assert_eq!(content, "Let me think..."),
            _ => panic!("Expected Thinking"),
        }
    }

    #[test]
    fn test_ui_state_apply_tool_call_lifecycle() {
        let mut state = UiState::new("sess-1".into(), ".", "local");
        state.apply(UiEvent::ToolCallBegin {
            tool_name: "bash".into(),
            arguments: r#"{"command":"ls"}"#.into(),
        });
        assert_eq!(state.tool_calls.len(), 1);
        assert_eq!(state.tool_call_count, 1);
        assert_eq!(state.conversation.len(), 1);

        state.apply(UiEvent::ToolCallComplete {
            tool_name: "bash".into(),
            result: "file.txt".into(),
            duration_ms: Some(42),
        });
        match &state.tool_calls[0].status {
            ToolCallStatus::Success => (),
            _ => panic!("Expected Success"),
        }
        assert_eq!(state.tool_calls[0].duration_ms, Some(42));
    }

    #[test]
    fn test_ui_state_approval_flow() {
        let mut state = UiState::new("sess-1".into(), ".", "local");
        state.apply(UiEvent::ApprovalRequest {
            tool_name: "write".into(),
            reason: "modifying file".into(),
            arguments: r#"{"path":"test.rs"}"#.into(),
        });
        assert!(state.approval_state.has_pending());
        assert_eq!(state.approval_state.tool_name, Some("write".into()));

        state.apply(UiEvent::ApprovalResolved { approved: true });
        assert!(!state.approval_state.has_pending());
    }

    #[test]
    fn test_active_tab_toggle() {
        use ActiveTab::*;
        assert_eq!(Conversation.toggle(), Tools);
        assert_eq!(Tools.toggle(), Workspace);
        assert_eq!(Workspace.toggle(), Skills);
        assert_eq!(Skills.toggle(), Logs);
        assert_eq!(Logs.toggle(), Conversation);
    }

    #[test]
    fn test_extract_arg_preview() {
        // JSON with command
        let preview = extract_arg_preview(r#"{"command":"ls -la"}"#);
        assert_eq!(preview, "Command: ls -la");

        // JSON with path
        let preview = extract_arg_preview(r#"{"path":"/tmp/test.txt"}"#);
        assert_eq!(preview, "Path: /tmp/test.txt");

        // JSON with file_path
        let preview = extract_arg_preview(r#"{"file_path":"src/main.rs"}"#);
        assert_eq!(preview, "File: src/main.rs");

        // Non-JSON or parse failure
        let preview = extract_arg_preview("not json");
        assert_eq!(preview, "");
    }
}
