//! Shared application state mutated by agent events and read by render loop.

use ratatui_textarea::TextArea;
use std::collections::HashSet;
use std::time::Instant;

/// A single tool call entry for the tools panel.
#[derive(Debug, Clone)]
pub struct ToolCallEntry {
    pub sequence: u32,
    pub tool_name: String,
    pub arg_preview: String,
    pub status: ToolCallStatus,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone)]
pub enum ToolCallStatus {
    Running,
    Success,
    Error,
    Skipped,
}

/// A single rendered entry in the conversation view.
#[derive(Debug, Clone)]
pub enum ConversationEntry {
    UserInput {
        text: String,
    },
    Thinking {
        content: String,
    },
    ContentStreaming {
        content: String,
    },
    ToolCall {
        tool_name: String,
        arg_preview: String,
    },
    ToolResult {
        tool_name: String,
        preview: String,
        success: bool,
    },
    AgentAnswer {
        text: String,
    },
    RunSummary {
        iterations: u32,
        tool_calls: u32,
        elapsed_ms: u128,
    },
    Error {
        message: String,
    },
}

/// Snapshot of the workspace file tree.
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

/// Active tab in the right panel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ActiveTab {
    Conversation,
    Workspace,
    Skills,
    Logs,
}

impl ActiveTab {
    pub fn toggle(self) -> Self {
        match self {
            ActiveTab::Conversation => ActiveTab::Workspace,
            ActiveTab::Workspace => ActiveTab::Skills,
            ActiveTab::Skills => ActiveTab::Logs,
            ActiveTab::Logs => ActiveTab::Conversation,
        }
    }
}

/// State for the session list dialog overlay.
pub struct SessionDialog {
    pub open: bool,
    pub sessions: Vec<SessionDialogEntry>,
    pub selected: usize,
}

pub struct SessionDialogEntry {
    pub session_id: String,
    pub entry_count: usize,
    pub age_label: String,
}

impl SessionDialog {
    pub fn new() -> Self {
        Self {
            open: false,
            sessions: Vec::new(),
            selected: 0,
        }
    }
}

/// Log viewer state for the Logs tab.
pub struct LogViewer {
    pub run_logs: Vec<LogRunSummary>,
    pub selected_run: Option<String>,
    pub entries: Vec<LogLine>,
    pub scroll: u16,
    pub auto_scroll: bool,
    pub loaded: bool,
}

pub struct LogRunSummary {
    pub run_id: String,
    pub event_count: usize,
    pub last_event: String,
    pub last_event_time: String,
}

pub struct LogLine {
    pub event_type: String,
    pub summary: String,
    pub timestamp: String,
}

/// Display-friendly skill entry for the Skills tab.
#[derive(Debug, Clone)]
pub struct SkillDisplayEntry {
    pub name: String,
    pub version: String,
    pub scope: String,
    pub description: String,
}

impl LogViewer {
    pub fn new() -> Self {
        Self {
            run_logs: Vec::new(),
            selected_run: None,
            entries: Vec::new(),
            scroll: 0,
            auto_scroll: true,
            loaded: false,
        }
    }

    pub fn scan_logs(&mut self) {
        let working_dir = std::env::current_dir().unwrap_or_default();
        let project_name = working_dir
            .file_name()
            .unwrap_or(std::ffi::OsStr::new("default"))
            .to_string_lossy();
        let home = std::env::var("HOME").unwrap_or_default();
        let base = std::path::PathBuf::from(home)
            .join(".vol-coding")
            .join(project_name.as_ref());
        let logs_dir = base.join("logs");

        if !logs_dir.exists() {
            return;
        }

        let Ok(entries) = std::fs::read_dir(&logs_dir) else {
            return;
        };

        for entry in entries {
            let Ok(entry) = entry else { continue };
            let path = entry.path();
            if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            if path.parent() != Some(&logs_dir) {
                continue;
            }

            let run_id = match path.file_stem().and_then(|s| s.to_str()) {
                Some(id) => id.to_string(),
                None => continue,
            };

            let content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let lines: Vec<&str> = content.lines().collect();
            let event_count = lines.len();

            let (last_event, last_event_time) = lines
                .last()
                .and_then(|line| serde_json::from_str::<vol_llm_observability::LogEntry>(line).ok())
                .map(|e| (e.event.clone(), e.timestamp.format("%H:%M:%S").to_string()))
                .unwrap_or_else(|| ("unknown".to_string(), "—".to_string()));

            self.run_logs.push(LogRunSummary {
                run_id,
                event_count,
                last_event,
                last_event_time,
            });
        }

        self.run_logs
            .sort_by(|a, b| b.event_count.cmp(&a.event_count));
    }

    pub fn load_run(&mut self, run_id: &str) {
        let working_dir = std::env::current_dir().unwrap_or_default();
        let project_name = working_dir
            .file_name()
            .unwrap_or(std::ffi::OsStr::new("default"))
            .to_string_lossy();
        let home = std::env::var("HOME").unwrap_or_default();
        let base = std::path::PathBuf::from(home)
            .join(".vol-coding")
            .join(project_name.as_ref());
        let log_path = base.join("logs").join(format!("{}.jsonl", run_id));

        let content = match std::fs::read_to_string(&log_path) {
            Ok(c) => c,
            Err(_) => return,
        };

        self.entries = content
            .lines()
            .filter_map(|line| serde_json::from_str::<vol_llm_observability::LogEntry>(line).ok())
            .map(|entry| LogLine {
                event_type: entry.event.clone(),
                summary: entry.format_event_summary(),
                timestamp: entry.timestamp.format("%H:%M:%S").to_string(),
            })
            .collect();
    }
}

/// Shared application state.
pub struct AppState {
    /// Session ID displayed in status bar.
    pub session_id: String,
    /// Number of agent.run() calls in this TUI session.
    pub run_count: u32,
    /// Current iteration count in the active run.
    pub iteration: u32,
    /// Total tool calls in the active run.
    pub tool_call_count: u32,
    /// When the current run started.
    pub run_start: Option<Instant>,
    /// Frozen elapsed time from the last completed run (used when idle).
    pub run_elapsed: std::time::Duration,
    /// Whether an agent run is in progress.
    pub is_running: bool,
    /// Whether the app is in the process of exiting.
    pub exiting: bool,
    /// Conversation history entries for the right panel.
    pub conversation: Vec<ConversationEntry>,
    /// Tool call entries for the left panel.
    pub tool_calls: Vec<ToolCallEntry>,
    /// Workspace file tree.
    pub workspace: WorkspaceTree,
    /// Set of files modified by WriteTool/EditTool in current run.
    pub modified_files: HashSet<String>,
    /// Currently active tab.
    pub active_tab: ActiveTab,
    /// Multi-line input buffer.
    pub input: TextArea<'static>,
    /// Scroll offset for conversation panel.
    pub conversation_scroll: u16,
    /// Scroll offset for workspace panel.
    pub workspace_scroll: u16,
    /// Scroll offset for tools panel (auto-scrolls to bottom).
    pub tools_scroll: u16,
    /// Whether conversation auto-scroll is enabled.
    pub conversation_auto_scroll: bool,
    /// Whether unsafe mode is active (auto-approve all tool approvals).
    pub unsafe_mode: bool,
    /// Approval state shared with the TUI approval handler.
    pub approval_state: crate::approval::ApprovalState,
    /// Session list dialog state.
    pub session_dialog: SessionDialog,
    /// Last error message to display.
    pub last_error: Option<String>,
    /// Log viewer state for the Logs tab.
    pub log_viewer: LogViewer,
    /// Discovered skills for the Skills tab.
    pub skills: Vec<SkillDisplayEntry>,
}

impl AppState {
    pub fn new(session_id: String, working_dir: &str, skills: Vec<SkillDisplayEntry>) -> Self {
        let workspace = scan_workspace(working_dir);
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
            workspace,
            modified_files: HashSet::new(),
            active_tab: ActiveTab::Conversation,
            input: TextArea::default(),
            conversation_scroll: 0,
            workspace_scroll: 0,
            tools_scroll: 0,
            conversation_auto_scroll: true,
            unsafe_mode: false,
            approval_state: crate::approval::ApprovalState::new(false),
            session_dialog: SessionDialog::new(),
            last_error: None,
            log_viewer: LogViewer::new(),
            skills,
        }
    }

    /// Reset per-run state before starting a new agent.run().
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
}

/// Scan the working directory for files, skipping ignored directories.
fn scan_workspace(root: &str) -> WorkspaceTree {
    let skip_dirs = &[".git", "target", "node_modules"];
    let mut entries = Vec::new();

    fn walk(
        dir: &std::path::Path,
        root: &str,
        entries: &mut Vec<WorkspaceEntry>,
        skip_dirs: &[&str],
        indent: usize,
    ) {
        let Ok(read_dir) = std::fs::read_dir(dir) else {
            return;
        };
        let mut paths: Vec<_> = read_dir.filter_map(|e| e.ok()).map(|e| e.path()).collect();
        paths.sort();

        for path in paths {
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if path.is_dir() {
                if skip_dirs.contains(&file_name) {
                    continue;
                }
                let rel = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();
                entries.push(WorkspaceEntry {
                    path: rel.clone(),
                    is_dir: true,
                    modified: false,
                    indent,
                });
                walk(&path, root, entries, skip_dirs, indent + 1);
            } else {
                let rel = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();
                entries.push(WorkspaceEntry {
                    path: rel,
                    is_dir: false,
                    modified: false,
                    indent,
                });
            }
        }
    }

    let root_path = std::path::Path::new(root);
    if root_path.is_dir() {
        walk(root_path, root, &mut entries, skip_dirs, 0);
    }

    WorkspaceTree {
        root: root.to_string(),
        entries,
    }
}
