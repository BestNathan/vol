# TUI Ratatui Full Interface Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the current crossterm print-based TUI with a full ratatui terminal UI featuring a status bar, tool call panel, tabbed conversation/workspace views, multi-line input, and persistent layout.

**Architecture:** Frame-driven rendering via ratatui. Events from the coding agent update a shared `AppState` (protected by `tokio::sync::Mutex`). The main thread runs a ratatui render loop that reads `AppState` and redraws every frame. `EventBuffer` converts `AgentStreamEvent` into state mutations on `AppState`.

**Tech Stack:** ratatui 0.30, ratatui-textarea 0.8, crossterm 0.28, tokio

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `crates/vol-llm-tui/Cargo.toml` | Modify | Add ratatui, ratatui-textarea dependencies |
| `crates/vol-llm-tui/src/app.rs` | Create | `AppState` struct, initialization, accessors |
| `crates/vol-llm-tui/src/ui/mod.rs` | Create | Layout orchestration, `render_ui()`, tab switching |
| `crates/vol-llm-tui/src/ui/status_bar.rs` | Create | Status bar widget |
| `crates/vol-llm-tui/src/ui/tools_panel.rs` | Create | Tool call history panel |
| `crates/vol-llm-tui/src/ui/conversation.rs` | Create | Conversation tab widget |
| `crates/vol-llm-tui/src/ui/workspace_panel.rs` | Create | Workspace file tree widget |
| `crates/vol-llm-tui/src/ui/input.rs` | Create | Multi-line input wrapper around ratatui-textarea |
| `crates/vol-llm-tui/src/render.rs` | Rewrite | `EventBuffer` — converts `AgentStreamEvent` to `AppState` mutations |
| `crates/vol-llm-tui/src/main.rs` | Rewrite | Entry point, ratatui setup, event loop, agent spawning |

---

### Task 1: Add ratatui dependencies + AppState struct

**Files:**
- Modify: `crates/vol-llm-tui/Cargo.toml`
- Create: `crates/vol-llm-tui/src/app.rs`

- [ ] **Step 1: Add dependencies to Cargo.toml**

In `crates/vol-llm-tui/Cargo.toml`, add after `crossterm = "0.28"`:

```toml
ratatui = { version = "0.30", default-features = false, features = ["crossterm_0_28"] }
ratatui-textarea = { version = "0.8", default-features = false, features = ["crossterm_0_28"] }
```

**Version note:** `ratatui 0.30` introduced modular backends with `crossterm_0_28`/`crossterm_0_29` feature flags. `ratatui-textarea 0.8` mirrors this. Both use `default-features = false` to force crossterm 0.28 and avoid pulling in crossterm 0.29.

- [ ] **Step 2: Create AppState struct**

Create `crates/vol-llm-tui/src/app.rs`:

```rust
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
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p vol-llm-tui`
Expected: Fails because `ratatui` and `ratatui-textarea` need to be downloaded — but the struct code itself compiles.

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-tui/Cargo.toml crates/vol-llm-tui/src/app.rs
git commit -m "feat: add AppState struct and ratatui dependencies for new TUI"
```

---

### Task 2: Rewrite render.rs — EventBuffer as AppState mutator

**Files:**
- Rewrite: `crates/vol-llm-tui/src/render.rs`

- [ ] **Step 1: Replace render.rs with EventBuffer that mutates AppState**

```rust
//! Event buffer that converts AgentStreamEvent into AppState mutations.
//!
//! Instead of printing directly, this maintains state that the ratatui
//! render loop reads from AppState.

use crate::app::{AppState, ConversationEntry, ToolCallEntry, ToolCallStatus};
use std::time::Duration;
use vol_llm_core::AgentStreamEvent;

/// Stateful event buffer that tracks rendering state for deduplication.
pub struct EventBuffer {
    thinking_active: bool,
    thinking_buffer: String,
    content_buffer: String,
    current_tool_call_seq: Option<u32>,
}

impl EventBuffer {
    pub fn new() -> Self {
        Self {
            thinking_active: false,
            thinking_buffer: String::new(),
            content_buffer: String::new(),
            current_tool_call_seq: None,
        }
    }

    /// Process an event and mutate AppState accordingly.
    pub fn apply(&mut self, event: &AgentStreamEvent, state: &mut AppState) {
        match event {
            AgentStreamEvent::AgentStart { input, .. } => {
                state.reset_for_run();
                state.conversation.push(ConversationEntry::UserInput {
                    text: input.clone(),
                });
            }

            AgentStreamEvent::AgentComplete { response, .. } => {
                // Flush any pending thinking/content
                self.flush_thinking(state);
                self.flush_content(state);

                let elapsed = state.run_start
                    .map(|s| s.elapsed())
                    .unwrap_or_default();
                state.conversation.push(ConversationEntry::RunSummary {
                    iterations: state.iteration,
                    tool_calls: state.tool_call_count,
                    elapsed_ms: elapsed.as_millis(),
                });
                state.is_running = false;

                // Extract response content if available
                if let Some(resp) = response {
                    if let Some(content) = resp.get("content").and_then(|v| v.as_str()) {
                        if !content.is_empty() {
                            state.conversation.push(ConversationEntry::AgentAnswer {
                                text: content.to_string(),
                            });
                        }
                    }
                }
            }

            AgentStreamEvent::AgentAborted { reason, .. } => {
                self.flush_thinking(state);
                self.flush_content(state);
                state.conversation.push(ConversationEntry::Error {
                    message: reason.clone(),
                });
                state.is_running = false;
            }

            AgentStreamEvent::MaxIterationsReached { current_iteration, max_iterations, .. } => {
                self.flush_thinking(state);
                self.flush_content(state);
                state.conversation.push(ConversationEntry::Error {
                    message: format!(
                        "Max iterations reached ({}/{}) — waiting for user decision...",
                        current_iteration, max_iterations,
                    ),
                });
            }

            AgentStreamEvent::IterationContinued { from_iteration, .. } => {
                state.conversation.push(ConversationEntry::AgentAnswer {
                    text: format!(
                        "Continuing from iteration {} (counter reset to 0)",
                        from_iteration,
                    ),
                });
            }

            // LLM Call — meta events, not displayed
            AgentStreamEvent::LLMCallStart { .. }
            | AgentStreamEvent::LLMCallComplete { .. }
            | AgentStreamEvent::LLMCallError { .. } => {}

            // Thinking — accumulate deltas
            AgentStreamEvent::ThinkingStart { .. } => {
                self.thinking_active = true;
                self.thinking_buffer.clear();
                state.conversation.push(ConversationEntry::ThinkingStart);
            }

            AgentStreamEvent::ThinkingDelta { delta, .. } => {
                self.thinking_buffer.push_str(delta);
                state.conversation.push(ConversationEntry::ThinkingDelta {
                    delta: delta.clone(),
                });
            }

            AgentStreamEvent::ThinkingComplete { .. } => {
                self.thinking_active = false;
            }

            // Content — accumulate deltas
            AgentStreamEvent::ContentStart { .. } => {
                self.content_buffer.clear();
            }

            AgentStreamEvent::ContentDelta { delta, .. } => {
                self.content_buffer.push_str(delta);
            }

            AgentStreamEvent::ContentComplete { content, .. } => {
                if !content.is_empty() {
                    state.conversation.push(ConversationEntry::AgentAnswer {
                        text: content.clone(),
                    });
                }
            }

            // Tools
            AgentStreamEvent::ToolCallBegin { tool_name, arguments, .. } => {
                let seq = state.tool_call_count + 1;
                state.tool_call_count = seq;
                self.current_tool_call_seq = Some(seq);

                let arg_preview = extract_arg_preview(arguments);
                state.tool_calls.push(ToolCallEntry {
                    sequence: seq,
                    tool_name: tool_name.clone(),
                    arg_preview: arg_preview.clone(),
                    status: ToolCallStatus::Running,
                    duration_ms: None,
                });
                state.conversation.push(ConversationEntry::ToolCall {
                    tool_name: tool_name.clone(),
                    arg_preview,
                });
            }

            AgentStreamEvent::ToolCallComplete { tool_name, result, duration_ms, .. } => {
                self.update_tool_call_status(state, tool_name, ToolCallStatus::Success, *duration_ms);
                let preview = truncate_preview(result, 200);
                state.conversation.push(ConversationEntry::ToolResult {
                    tool_name: tool_name.clone(),
                    preview,
                    success: true,
                });

                // Track modified files
                if tool_name.contains("Write") || tool_name.contains("Edit") {
                    if let Some(path) = self.extract_file_path_from_result(result) {
                        state.modified_files.insert(path);
                    }
                }
            }

            AgentStreamEvent::ToolCallError { tool_name, error, duration_ms, .. } => {
                self.update_tool_call_status(state, tool_name, ToolCallStatus::Error, *duration_ms);
                state.conversation.push(ConversationEntry::ToolResult {
                    tool_name: tool_name.clone(),
                    preview: error.clone(),
                    success: false,
                });
            }

            AgentStreamEvent::ToolCallSkipped { tool_name, reason, duration_ms, .. } => {
                self.update_tool_call_status(state, tool_name, ToolCallStatus::Skipped, *duration_ms);
            }

            // Iteration
            AgentStreamEvent::IterationComplete { iteration, .. } => {
                state.iteration = *iteration;
                // Flush content when iteration completes
                self.flush_content(state);
            }

            // Plugin events — invisible
            AgentStreamEvent::PluginEvent { .. } => {}
        }

        // Auto-scroll conversation to bottom on new content
        if state.conversation_auto_scroll {
            state.conversation_scroll = state.conversation.len() as u16;
        }
        // Auto-scroll tools panel to bottom
        state.tools_scroll = state.tool_calls.len() as u16;
    }

    fn flush_thinking(&mut self, state: &mut AppState) {
        if self.thinking_active && !self.thinking_buffer.is_empty() {
            // Thinking is already accumulated via deltas, nothing extra needed
            self.thinking_buffer.clear();
            self.thinking_active = false;
        }
    }

    fn flush_content(&mut self, state: &mut AppState) {
        if !self.content_buffer.is_empty() {
            // Content is handled via ContentComplete, buffer is just a fallback
            self.content_buffer.clear();
        }
    }

    fn update_tool_call_status(
        &mut self,
        state: &mut AppState,
        tool_name: &str,
        status: ToolCallStatus,
        duration_ms: Option<u64>,
    ) {
        for entry in state.tool_calls.iter_mut().rev() {
            if entry.tool_name == tool_name && matches!(entry.status, ToolCallStatus::Running) {
                entry.status = status;
                entry.duration_ms = duration_ms;
                break;
            }
        }
    }

    fn extract_file_path_from_result(&self, result: &str) -> Option<String> {
        // Try to extract file_path from JSON result
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(result) {
            if let Some(path) = parsed.get("file_path").and_then(|v| v.as_str()) {
                return Some(path.to_string());
            }
        }
        None
    }
}

/// Extract a short preview of tool arguments for display.
fn extract_arg_preview(arguments: &str) -> String {
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(arguments) {
        if let Some(cmd) = parsed.get("command").and_then(|v| v.as_str()) {
            if cmd.len() > 80 {
                return format!("Command: {}...", &cmd[..77]);
            }
            return format!("Command: {}", cmd);
        }
        if let Some(path) = parsed.get("path").and_then(|v| v.as_str()) {
            return format!("Path: {}", path);
        }
        if let Some(file_path) = parsed.get("file_path").and_then(|v| v.as_str()) {
            return format!("File: {}", file_path);
        }
        if let Some(url) = parsed.get("url").and_then(|v| v.as_str()) {
            return format!("URL: {}", url);
        }
        if arguments.len() > 80 {
            return format!("Args: {}...", &arguments[..77]);
        }
        return format!("Args: {}", arguments);
    }
    String::new()
}

fn truncate_preview(s: &str, max_chars: usize) -> String {
    let total_chars = s.chars().count();
    let chars: Vec<char> = s.chars().take(max_chars).collect();
    if chars.is_empty() {
        return String::new();
    }
    let truncated: String = chars.into_iter().collect();
    if truncated.chars().count() < total_chars {
        format!("{}...", truncated)
    } else {
        truncated
    }
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p vol-llm-tui`
Expected: Compiles successfully (app.rs + render.rs together)

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-tui/src/render.rs
git commit -m "refactor: rewrite EventBuffer to mutate AppState instead of printing"
```

---

### Task 3: Create UI widgets — status bar + tools panel

**Files:**
- Create: `crates/vol-llm-tui/src/ui/mod.rs`
- Create: `crates/vol-llm-tui/src/ui/status_bar.rs`
- Create: `crates/vol-llm-tui/src/ui/tools_panel.rs`

- [ ] **Step 1: Create ui/mod.rs**

Create `crates/vol-llm-tui/src/ui/mod.rs`:

```rust
//! UI layout orchestration.

mod status_bar;
mod tools_panel;

pub use status_bar::render_status_bar;
pub use tools_panel::render_tools_panel;

use crate::app::AppState;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Render the entire UI frame.
pub fn render_ui(f: &mut Frame, state: &AppState) {
    let area = f.area();
    if area.width < 30 || area.height < 15 {
        // Terminal too small — show a message
        f.render_widget(
            ratatui::widgets::Paragraph::new("Terminal too small. Resize to continue.")
                .alignment(ratatui::style::Styling::Center),
            area,
        );
        return;
    }

    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // status bar
            Constraint::Min(10),    // main content (tools + tabs)
            Constraint::Length(1),  // input hint
            Constraint::Length(5),  // input area
            Constraint::Length(1),  // help bar
        ])
        .split(area);

    // Status bar
    render_status_bar(f, main_chunks[0], state);

    // Main content: left (tools) + right (tab content)
    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(70),
        ])
        .split(main_chunks[1]);

    render_tools_panel(f, content_chunks[0], state);

    // Right panel: tab bar + content
    render_right_panel(f, content_chunks[1], state);

    // Input hint
    render_input_hint(f, main_chunks[2], state);

    // Input area
    render_input_area(f, main_chunks[3], state);

    // Help bar
    render_help_bar(f, main_chunks[4]);
}

fn render_right_panel(f: &mut Frame, area: Rect, state: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),  // tab bar
            Constraint::Min(1),     // tab content
        ])
        .split(area);

    render_tab_bar(f, chunks[0], state);

    match state.active_tab {
        crate::app::ActiveTab::Conversation => {
            super::conversation::render_conversation(f, chunks[1], state);
        }
        crate::app::ActiveTab::Workspace => {
            super::workspace_panel::render_workspace(f, chunks[1], state);
        }
    }
}

fn render_tab_bar(f: &mut Frame, area: Rect, state: &AppState) {
    use ratatui::style::{Color, Modifier, Style};
    use ratatui::widgets::Paragraph;
    use ratatui::text::{Line, Span};

    let conv_style = if matches!(state.active_tab, crate::app::ActiveTab::Conversation) {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGrey)
    };
    let ws_style = if matches!(state.active_tab, crate::app::ActiveTab::Workspace) {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGrey)
    };

    let line = Line::from(vec![
        Span::styled(" [1] Conversation ", conv_style),
        Span::styled("  [2] Workspace  ", ws_style),
    ]);

    f.render_widget(Paragraph::new(line), area);
}

fn render_input_hint(f: &mut Frame, area: Rect, state: &AppState) {
    use ratatui::style::{Color, Style};
    use ratatui::widgets::Paragraph;

    let text = if state.is_running {
        " Agent is running...  (input will be sent after completion)"
    } else {
        " Ctrl+Enter: Send  │  Esc: Clear"
    };
    f.render_widget(
        Paragraph::new(text).style(Style::default().fg(Color::DarkGrey)),
        area,
    );
}

fn render_input_area(f: &mut Frame, area: Rect, state: &AppState) {
    use ratatui::widgets::{Block, Borders};
    use ratatui::style::{Color, Style};

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Input (Ctrl+Enter to send)")
        .style(Style::default().fg(Color::Cyan));

    // Render the textarea
    let mut textarea = state.input.clone();
    textarea.set_block(block);
    f.render_widget(&textarea, area);
}

fn render_help_bar(f: &mut Frame, area: Rect) {
    use ratatui::style::{Color, Style};
    use ratatui::widgets::Paragraph;

    f.render_widget(
        Paragraph::new(" Tab:Switch  PgUp/PgDn:Scroll  Ctrl+Enter:Send  Esc:Clear  /quit:Exit")
            .style(Style::default().fg(Color::DarkGrey)),
        area,
    );
}
```

- [ ] **Step 2: Create ui/status_bar.rs**

Create `crates/vol-llm-tui/src/ui/status_bar.rs`:

```rust
//! Status bar widget — shows session ID, iteration, tool count, elapsed time.

use crate::app::AppState;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

pub fn render_status_bar(f: &mut Frame, area: Rect, state: &AppState) {
    let elapsed = state.run_start
        .map(|s| s.elapsed())
        .unwrap_or_default();

    let status_indicator = if state.is_running {
        Span::styled(" ◉ Running ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
    } else {
        Span::styled(" ○ Idle ", Style::default().fg(Color::DarkGrey))
    };

    let session = Span::styled(
        format!(" Session: {} ", state.session_id),
        Style::default().fg(Color::Cyan),
    );

    let stats = Span::styled(
        format!(" │ Iter: {} │ Tools: {} │ Time: {:.0}s │ ",
            state.iteration,
            state.tool_call_count,
            elapsed.as_secs_f64(),
        ),
        Style::default().fg(Color::White),
    );

    let run_count = Span::styled(
        format!("Runs: {} ", state.run_count),
        Style::default().fg(Color::Yellow),
    );

    let line = Line::from(vec![
        status_indicator,
        session,
        stats,
        run_count,
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::Cyan));

    f.render_widget(Paragraph::new(line).block(block), area);
}
```

- [ ] **Step 3: Create ui/tools_panel.rs**

Create `crates/vol-llm-tui/src/ui/tools_panel.rs`:

```rust
//! Tools call history panel.

use crate::app::{AppState, ToolCallStatus};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

pub fn render_tools_panel(f: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Tools ({}) ", state.tool_calls.len()));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if state.tool_calls.is_empty() {
        f.render_widget(
            Paragraph::new("No tool calls yet").style(Style::default().fg(Color::DarkGrey)),
            inner,
        );
        return;
    }

    let mut lines = Vec::new();
    for entry in &state.tool_calls {
        let (status_style, status_text) = match entry.status {
            ToolCallStatus::Running => (Style::default().fg(Color::Yellow), "..."),
            ToolCallStatus::Success => (Style::default().fg(Color::Green), &format!("{}ms", entry.duration_ms.unwrap_or(0))),
            ToolCallStatus::Error => (Style::default().fg(Color::Red), "ERR"),
            ToolCallStatus::Skipped => (Style::default().fg(Color::DarkGrey), "SKIP"),
        };

        let title = Span::styled(
            format!("{}. [{}]", entry.sequence, entry.tool_name),
            Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD),
        );
        let duration = Span::styled(status_text.to_string(), status_style);

        lines.push(Line::from(vec![title, Span::raw("    "), duration]));
        lines.push(Line::from(vec![
            Span::styled(format!("   {}", entry.arg_preview), Style::default().fg(Color::DarkGrey)),
        ]));
        lines.push(Line::from("")); // blank separator
    }

    let paragraph = Paragraph::new(lines);
    f.render_widget(paragraph, inner);
}
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p vol-llm-tui`
Expected: Compiles successfully

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-tui/src/ui/mod.rs crates/vol-llm-tui/src/ui/status_bar.rs crates/vol-llm-tui/src/ui/tools_panel.rs
git commit -m "feat: create UI layout, status bar, and tools panel widgets"
```

---

### Task 4: Create conversation and workspace tab widgets

**Files:**
- Create: `crates/vol-llm-tui/src/ui/conversation.rs`
- Create: `crates/vol-llm-tui/src/ui/workspace_panel.rs`
- Modify: `crates/vol-llm-tui/src/ui/mod.rs` (add imports)

- [ ] **Step 1: Create ui/conversation.rs**

Create `crates/vol-llm-tui/src/ui/conversation.rs`:

```rust
//! Conversation history tab widget.

use crate::app::{AppState, ConversationEntry};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

pub fn render_conversation(f: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Conversation ");

    let inner = block.inner(area);
    f.render_widget(block, area);

    if state.conversation.is_empty() {
        f.render_widget(
            Paragraph::new("Send a message to start...").style(Style::default().fg(Color::DarkGrey)),
            inner,
        );
        return;
    }

    let mut lines = Vec::new();
    for entry in &state.conversation {
        match entry {
            ConversationEntry::UserInput { text } => {
                lines.push(Line::from(""));
                lines.push(Line::from(vec![
                    Span::styled(">>> ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                    Span::styled(text.clone(), Style::default().fg(Color::Cyan)),
                ]));
            }
            ConversationEntry::ThinkingStart => {
                lines.push(Line::from(vec![
                    Span::styled("Thinking...", Style::default().fg(Color::Yellow)),
                ]));
            }
            ConversationEntry::ThinkingDelta { delta } => {
                lines.push(Line::from(vec![
                    Span::styled(delta.clone(), Style::default().fg(Color::DarkGrey)),
                ]));
            }
            ConversationEntry::ToolCall { tool_name, arg_preview } => {
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("[{}] ", tool_name),
                        Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(arg_preview.clone(), Style::default().fg(Color::DarkGrey)),
                ]));
            }
            ConversationEntry::ToolResult { tool_name, preview, success } => {
                let style = if *success {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::Red)
                };
                let status = if *success { "OK" } else { "ERR" };
                lines.push(Line::from(vec![
                    Span::styled(format!("  {} {}", status, tool_name), style),
                ]));
                for line in preview.lines().take(6) {
                    lines.push(Line::from(vec![
                        Span::styled(format!("    {}", line), Style::default().fg(Color::DarkGrey)),
                    ]));
                }
            }
            ConversationEntry::AgentAnswer { text } => {
                lines.push(Line::from(""));
                for line in text.lines() {
                    lines.push(Line::from(vec![
                        Span::styled(line.to_string(), Style::default().fg(Color::White)),
                    ]));
                }
            }
            ConversationEntry::RunSummary { iterations, tool_calls, elapsed_ms } => {
                lines.push(Line::from(""));
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("Done · {} iteration{} · {} tool call{} · {}ms",
                            iterations,
                            if *iterations == 1 { "" } else { "s" },
                            tool_calls,
                            if *tool_calls == 1 { "" } else { "s" },
                            elapsed_ms,
                        ),
                        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                    ),
                ]));
            }
            ConversationEntry::Error { message } => {
                lines.push(Line::from(vec![
                    Span::styled(message.clone(), Style::default().fg(Color::Red)),
                ]));
            }
        }
    }

    let paragraph = Paragraph::new(lines);
    f.render_widget(paragraph, inner);
}
```

- [ ] **Step 2: Create ui/workspace_panel.rs**

Create `crates/vol-llm-tui/src/ui/workspace_panel.rs`:

```rust
//! Workspace file tree tab widget.

use crate::app::AppState;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

pub fn render_workspace(f: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Workspace ");

    let inner = block.inner(area);
    f.render_widget(block, area);

    if state.workspace.entries.is_empty() {
        f.render_widget(
            Paragraph::new("Workspace scan failed or directory empty").style(Style::default().fg(Color::DarkGrey)),
            inner,
        );
        return;
    }

    let mut lines = Vec::new();
    for entry in &state.workspace.entries {
        let indent = "  ".repeat(entry.indent);
        let icon = if entry.is_dir { "📂 " } else { "📄 " };

        let mut spans = vec![
            Span::styled(indent.clone(), Style::default()),
            Span::styled(icon, Style::default().fg(Color::Blue)),
            Span::styled(
                entry.path.split('/').last().unwrap_or(&entry.path).to_string(),
                Style::default().fg(Color::White),
            ),
        ];

        // Show modification marker
        if !entry.is_dir && state.modified_files.contains(&entry.path) {
            spans.push(Span::styled(
                "  ✎ M",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ));
        }

        lines.push(Line::from(spans));
    }

    let paragraph = Paragraph::new(lines);
    f.render_widget(paragraph, inner);
}
```

- [ ] **Step 3: Update ui/mod.rs to import new modules**

Add after `mod tools_panel;`:

```rust
mod conversation;
mod workspace_panel;

pub use conversation::render_conversation;
pub use workspace_panel::render_workspace;
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p vol-llm-tui`
Expected: Compiles successfully

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-tui/src/ui/conversation.rs crates/vol-llm-tui/src/ui/workspace_panel.rs crates/vol-llm-tui/src/ui/mod.rs
git commit -m "feat: add conversation and workspace tab widgets"
```

---

### Task 5: Rewrite main.rs — ratatui event loop + agent spawning

**Files:**
- Rewrite: `crates/vol-llm-tui/src/main.rs`

- [ ] **Step 1: Rewrite main.rs**

Replace the entire contents of `crates/vol-llm-tui/src/main.rs` with:

```rust
//! vol-llm-tui: Interactive ratatui TUI for the coding agent.
//!
//! Provides a full terminal UI with status bar, tool call panel,
//! tabbed conversation/workspace views, multi-line input, and persistent layout.

mod app;
mod render;
mod ui;

use std::io::{self, stdout};
use std::sync::Arc;
use std::time::Duration;

use app::AppState;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use render::EventBuffer;
use vol_llm_agents::coding::{CodingAgent, CodingAgentConfig, EventObserver, ObserverError};
use vol_llm_core::AgentStreamEvent;
use vol_llm_tool::{ToolConfig, ProxyConfig};
use vol_session::FileMessageStore;

/// Observer that forwards events to EventBuffer for AppState mutation.
struct RatatuiObserver {
    buffer: tokio::sync::Mutex<EventBuffer>,
    state: Arc<tokio::sync::Mutex<AppState>>,
}

#[async_trait::async_trait]
impl EventObserver for RatatuiObserver {
    async fn on_event(&self, event: &AgentStreamEvent) -> Result<(), ObserverError> {
        let mut buf = self.buffer.lock().await;
        let mut state = self.state.lock().await;
        buf.apply(event, &mut state);
        Ok(())
    }

    async fn on_complete(&self) -> Result<(), ObserverError> {
        Ok(())
    }
}

impl RatatuiObserver {
    fn new(state: Arc<tokio::sync::Mutex<AppState>>) -> Self {
        Self {
            buffer: tokio::sync::Mutex::new(EventBuffer::new()),
            state,
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Verify API key
    let _api_key = std::env::var("ANTHROPIC_AUTH_TOKEN")
        .expect("ANTHROPIC_AUTH_TOKEN must be set");

    // Create persistent session
    let session: Arc<vol_llm_agents::coding::Session> = create_session()?;
    let session_id = session.id().to_string();

    // Setup terminal with panic recovery
    setup_terminal()?;
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic| {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen);
        original_hook(panic);
    }));

    // Create shared state
    let working_dir = std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let state = Arc::new(tokio::sync::Mutex::new(
        AppState::new(session_id, &working_dir),
    ));

    // Create ratatui terminal
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    // Main event loop
    let result = run_event_loop(&mut terminal, state, session).await;

    // Cleanup
    cleanup_terminal()?;
    std::panic::take_hook(); // Reset panic hook

    result
}

fn create_session() -> Result<Arc<vol_llm_agents::coding::Session>, Box<dyn std::error::Error>> {
    let session_dir = std::env::current_dir()
        .unwrap_or_default()
        .join(".vol-sessions");

    if let Err(e) = std::fs::create_dir_all(&session_dir) {
        eprintln!("Warning: cannot create session dir: {}", e);
        eprintln!("Using in-memory session (no history persistence)");
        use vol_session::InMemoryMessageStore;
        use vol_llm_agent::session::InMemorySessionStore;
        return Ok(Arc::new(vol_llm_agents::coding::Session::new(
            "tui_memory".to_string(),
            Arc::new(InMemorySessionStore::new()),
            Arc::new(InMemoryMessageStore::new()),
        )));
    }

    let session_id = format!("tui_{}", chrono::Utc::now().format("%Y%m%d_%H%M%S%.3f"));
    let message_store = Arc::new(FileMessageStore::new(&session_dir, &session_id));
    let session_store = Arc::new(vol_session::InMemorySessionStore::new());
    Ok(Arc::new(vol_llm_agents::coding::Session::new(
        session_id.clone(),
        session_store,
        message_store,
    )))
}

fn setup_terminal() -> io::Result<()> {
    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;
    Ok(())
}

fn cleanup_terminal() -> io::Result<()> {
    execute!(stdout(), LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}

async fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: Arc<tokio::sync::Mutex<AppState>>,
    session: Arc<vol_llm_agents::coding::Session>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut render_interval = tokio::time::interval(Duration::from_millis(33)); // ~30fps

    loop {
        tokio::select! {
            // Render tick
            _ = render_interval.tick() => {
                let state = state.lock().await;
                terminal.draw(|f| ui::render_ui(f, &state))?;
            }

            // Event handling (non-blocking poll)
            event = poll_event() => {
                match event {
                    Some(Event::Key(key)) => {
                        let mut state = state.lock().await;
                        match handle_key(key, &mut state) {
                            KeyAction::Exit => return Ok(()),
                            KeyAction::Send(input) => {
                                drop(state);
                                spawn_agent(input, state.clone(), session.clone());
                            }
                            KeyAction::None => {}
                        }
                    }
                    Some(Event::Resize(_, _)) => {
                        // Terminal resized — next render will adjust
                    }
                    None => {
                        // No event, just continue (render tick will fire)
                    }
                    _ => {}
                }
            }
        }
    }
}

enum KeyAction {
    Exit,
    Send(String),
    None,
}

fn handle_key(key: KeyEvent, state: &mut AppState) -> KeyAction {
    match (key.modifiers, key.code) {
        // Ctrl+Enter: send input
        (KeyModifiers::CONTROL, KeyCode::Enter) => {
            if state.is_running {
                return KeyAction::None;
            }
            let input = state.input.lines().join("\n").trim().to_string();
            if input.is_empty() {
                return KeyAction::None;
            }
            state.input.lines_mut().clear();
            KeyAction::Send(input)
        }

        // Escape: clear input
        (_, KeyCode::Esc) => {
            state.input.lines_mut().clear();
            KeyAction::None
        }

        // Tab: switch tabs
        (_, KeyCode::Tab) => {
            state.active_tab = state.active_tab.toggle();
            KeyAction::None
        }

        // PageUp/PageDown: scroll conversation
        (_, KeyCode::PageUp) => {
            if state.conversation_scroll > 0 {
                state.conversation_scroll -= 1;
                state.conversation_auto_scroll = false;
            }
            KeyAction::None
        }
        (_, KeyCode::PageDown) => {
            state.conversation_scroll += 1;
            state.conversation_auto_scroll = false;
            KeyAction::None
        }

        // Ctrl+1/2: direct tab switch
        (KeyModifiers::CONTROL, KeyCode::Char('1')) => {
            state.active_tab = app::ActiveTab::Conversation;
            KeyAction::None
        }
        (KeyModifiers::CONTROL, KeyCode::Char('2')) => {
            state.active_tab = app::ActiveTab::Workspace;
            KeyAction::None
        }

        // All other keys: pass to textarea (handled below)
        _ => {
            state.input.input(key);
            KeyAction::None
        }
    }
}

/// Poll for a crossterm event with a short timeout.
async fn poll_event() -> Option<Event> {
    // Use tokio::task::spawn_blocking to avoid blocking the async runtime
    tokio::task::spawn_blocking(|| {
        if event::poll(Duration::from_millis(100)).unwrap_or(false) {
            event::read().ok()
        } else {
            None
        }
    }).await.unwrap_or(None)
}

fn spawn_agent(
    input: String,
    state: Arc<tokio::sync::Mutex<AppState>>,
    session: Arc<vol_llm_agents::coding::Session>,
) {
    tokio::spawn(async move {
        // Set running flag
        {
            let mut state = state.lock().await;
            state.is_running = true;
        }

        // Configure tools
        let mut tool_config = ToolConfig::new();
        if let Ok(tavily_key) = std::env::var("TAVILY_API_KEY") {
            tool_config.set("web_search", vol_llm_tools_builtin::WebSearchConfig {
                provider: "tavily".to_string(),
                api_key: tavily_key,
                proxy: ProxyConfig::default(),
            });
        }
        if let Ok(max_len) = std::env::var("WEB_FETCH_MAX_LENGTH") {
            tool_config.set("web_fetch", vol_llm_tools_builtin::WebFetchConfig {
                max_content_length: max_len.parse().ok(),
                proxy: ProxyConfig::default(),
            });
        }

        let working_dir = std::env::current_dir().unwrap_or_default();
        let config = CodingAgentConfig {
            max_iterations: 10,
            working_dir,
            hitl_enabled: true,
            verbose: false,
            html_report_path: None,
            session: Some(session.clone()),
            tool_config,
            ..Default::default()
        };

        let agent = match CodingAgent::new(config).await {
            Ok(a) => a,
            Err(e) => {
                let mut state = state.lock().await;
                state.conversation.push(app::ConversationEntry::Error {
                    message: format!("Error creating agent: {}", e),
                });
                state.is_running = false;
                return;
            }
        };

        let observer = RatatuiObserver::new(state.clone());
        let agent = agent.with_observer(observer);

        match agent.run(&input).await {
            Ok(_response) => {
                // All events handled via observer
            }
            Err(e) => {
                let mut state = state.lock().await;
                state.conversation.push(app::ConversationEntry::Error {
                    message: format!("Error: {}", e),
                });
                state.is_running = false;
            }
        }
    });
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p vol-llm-tui`
Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-tui/src/main.rs
git commit -m "feat: rewrite main.rs with ratatui event loop, agent spawning, and full UI"
```

---

### Task 6: Full workspace verification

**Files:** No changes — just verification

- [ ] **Step 1: Full workspace check**

Run: `cargo check --workspace`
Expected: All crates compile

- [ ] **Step 2: Run all tests**

Run: `cargo test --workspace --lib`
Expected: All tests pass (except pre-existing vol-llm-provider failure)

- [ ] **Step 3: Manual test**

Run: `source .env && cargo run -p vol-llm-tui`

Verify:
1. Status bar shows session ID, idle state
2. Left panel shows "No tool calls yet"
3. Right panel shows Conversation tab with "Send a message to start..."
4. Type text in input area, press Ctrl+Enter to send
5. Events stream in real-time during agent run
6. Tool calls appear in left panel
7. Press Tab to switch to Workspace tab, see file tree
8. After agent completes, status shows "Idle", input re-enabled
9. Press /quit or Ctrl+C to exit — terminal restores normally

- [ ] **Step 4: Commit** (if any fixes needed from manual test)

---

## Summary of Changes

| File | Change | Lines |
|------|--------|-------|
| `crates/vol-llm-tui/Cargo.toml` | Add ratatui, ratatui-textarea deps | +2 |
| `crates/vol-llm-tui/src/app.rs` | **New** — AppState, ToolCallEntry, ConversationEntry, WorkspaceTree | ~150 |
| `crates/vol-llm-tui/src/ui/mod.rs` | **New** — Layout orchestration, tab bar, input hint, help bar | ~120 |
| `crates/vol-llm-tui/src/ui/status_bar.rs` | **New** — Status bar widget | ~40 |
| `crates/vol-llm-tui/src/ui/tools_panel.rs` | **New** — Tool call history panel | ~60 |
| `crates/vol-llm-tui/src/ui/conversation.rs` | **New** — Conversation tab widget | ~80 |
| `crates/vol-llm-tui/src/ui/workspace_panel.rs` | **New** — Workspace file tree widget | ~60 |
| `crates/vol-llm-tui/src/render.rs` | **Rewrite** — EventBuffer mutates AppState instead of printing | ~180 |
| `crates/vol-llm-tui/src/main.rs` | **Rewrite** — Ratatui terminal setup, event loop, agent spawning | ~250 |

**Key behavioral changes:**
1. Terminal switches to alternate screen + raw mode (ratatui)
2. Events update shared AppState via EventBuffer, not direct printing
3. 30fps render loop redraws entire UI from AppState every frame
4. Agent runs in background tokio task, UI remains responsive during runs
5. Multi-line input via ratatui-textarea (Ctrl+Enter to send)
6. Tab switching between Conversation and Workspace views
7. File tree shows modification markers from WriteTool/EditTool
