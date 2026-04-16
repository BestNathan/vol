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
    UserInput { text: String },
    ThinkingStart,
    ThinkingDelta { delta: String },
    ToolCall { tool_name: String, arg_preview: String },
    ToolResult { tool_name: String, preview: String, success: bool },
    AgentAnswer { text: String },
    RunSummary { iterations: u32, tool_calls: u32, elapsed_ms: u128 },
    Error { message: String },
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
}

impl ActiveTab {
    pub fn toggle(self) -> Self {
        match self {
            ActiveTab::Conversation => ActiveTab::Workspace,
            ActiveTab::Workspace => ActiveTab::Conversation,
        }
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
    /// Last error message to display.
    pub last_error: Option<String>,
}

impl AppState {
    pub fn new(session_id: String, working_dir: &str) -> Self {
        let workspace = scan_workspace(working_dir);
        Self {
            session_id,
            run_count: 0,
            iteration: 0,
            tool_call_count: 0,
            run_start: None,
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
            last_error: None,
        }
    }

    /// Reset per-run state before starting a new agent.run().
    pub fn reset_for_run(&mut self) {
        self.iteration = 0;
        self.tool_call_count = 0;
        self.run_start = Some(Instant::now());
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
        let Ok(read_dir) = std::fs::read_dir(dir) else { return };
        let mut paths: Vec<_> = read_dir
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .collect();
        paths.sort();

        for path in paths {
            let file_name = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            if path.is_dir() {
                if skip_dirs.contains(&file_name) {
                    continue;
                }
                let rel = path.strip_prefix(root)
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
                let rel = path.strip_prefix(root)
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
