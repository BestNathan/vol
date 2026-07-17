// crates/vol-llm-ui/src/tui/render.rs
//
// Migrates the 9 render functions from vol-llm-tui/src/ui/* (conversation.rs,
// tools_panel.rs, input_area.rs, workspace_panel.rs, log_viewer.rs,
// skills_panel.rs, session_dialog.rs, status_bar, mod.rs), adapting from
// AppState to UiState.

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::state::{ActiveTab, ConversationEntry, ToolCallStatus, UiState};

/// Render the full UI to the frame.
#[allow(clippy::indexing_slicing)]
pub fn render_ui(frame: &mut Frame, state: &UiState) {
    let area = frame.area();

    // Status bar: 1 row at top
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(area);

    render_status_bar(frame, chunks[0], state);

    // Split remaining area: tools panel (30%) | content panel (70%)
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(chunks[1]);

    render_tools_panel(frame, main_chunks[0], state);
    render_right_panel(frame, main_chunks[1], state);
    render_session_dialog(frame, area, state);
}

#[allow(clippy::indexing_slicing)]
fn render_right_panel(frame: &mut Frame, area: Rect, state: &UiState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // tab bar
            Constraint::Min(3),    // tab content
            Constraint::Length(5), // input area
        ])
        .split(area);

    render_tab_bar(frame, chunks[0], state);

    match state.active_tab {
        ActiveTab::Conversation => render_conversation(frame, chunks[1], state),
        ActiveTab::Tools => render_tools_panel(frame, chunks[1], state),
        ActiveTab::Workspace => render_workspace(frame, chunks[1], state),
        ActiveTab::Logs => render_log_viewer(frame, chunks[1], state),
        ActiveTab::Skills => render_skills(frame, chunks[1], state),
        ActiveTab::Agents => render_agents_panel(frame, chunks[1], state),
        ActiveTab::Sessions => render_sessions_panel(frame, chunks[1], state),
        ActiveTab::Mcp => render_mcp(frame, chunks[1], state),
        ActiveTab::Tasks => render_tasks_placeholder(frame, chunks[1]),
    }

    render_input_area(frame, chunks[2], state);
}

// === Status Bar =============================================================

fn render_status_bar(frame: &mut Frame, area: Rect, state: &UiState) {
    let elapsed = if state.is_running {
        state.run_start.map(|s| s.elapsed()).unwrap_or_default()
    } else {
        state.run_elapsed
    };
    let elapsed_secs = elapsed.as_secs();
    let time_str = format!("{:02}:{:02}", elapsed_secs / 60, elapsed_secs % 60);

    let status = if state.is_running { "Running" } else { "Idle" };
    let unsafe_prefix = if state.unsafe_mode { "!! " } else { "" };
    let prefix = if state.exiting { "QUITTING · " } else { "" };

    let text = format!(
        " {}Session: {}{} │ Run: {} │ Iter: {} │ Tools: {} │ Time: {} │ {}",
        unsafe_prefix,
        prefix,
        state.session_id,
        state.run_count,
        state.iteration,
        state.tool_call_count,
        time_str,
        status,
    );

    let paragraph =
        Paragraph::new(text).style(Style::default().fg(Color::White).bg(Color::DarkGray));
    frame.render_widget(paragraph, area);
}

// === Tab Bar =================================================================

fn render_tab_bar(frame: &mut Frame, area: Rect, state: &UiState) {
    let style = |tab: ActiveTab| {
        if tab == state.active_tab {
            Style::default()
                .fg(Color::Black)
                .bg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        }
    };

    let tabs = Line::from(vec![
        Span::raw(" "),
        Span::styled(" Conversation ", style(ActiveTab::Conversation)),
        Span::raw(" "),
        Span::styled(" Sessions ", style(ActiveTab::Sessions)),
        Span::raw(" "),
        Span::styled(" Tools ", style(ActiveTab::Tools)),
        Span::raw(" "),
        Span::styled(" Workspace ", style(ActiveTab::Workspace)),
        Span::raw(" "),
        Span::styled(" Skills ", style(ActiveTab::Skills)),
        Span::raw(" "),
        Span::styled(" Logs ", style(ActiveTab::Logs)),
        Span::raw(" "),
        Span::styled(" Tasks ", style(ActiveTab::Tasks)),
        Span::raw(" "),
        Span::styled(" Agents ", style(ActiveTab::Agents)),
        Span::raw(" "),
    ]);

    let paragraph = Paragraph::new(tabs).block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(paragraph, area);
}

// === Conversation ============================================================

fn render_conversation(frame: &mut Frame, area: Rect, state: &UiState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Conversation ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.conversation.is_empty() {
        let empty = Paragraph::new("No messages yet. Type a query and press Enter.")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(empty, inner);
        return;
    }

    let lines = build_conversation_lines(state, inner.width as usize);
    let total = lines.len();
    let visible = inner.height as usize;
    let scroll = if state.conversation_auto_scroll {
        total.saturating_sub(visible)
    } else {
        (state.conversation_scroll as usize).min(total.saturating_sub(1))
    };

    let visible_lines: Vec<Line> = lines.into_iter().skip(scroll).take(visible).collect();
    let paragraph = Paragraph::new(Text::from(visible_lines));
    frame.render_widget(paragraph, inner);
}

#[allow(clippy::indexing_slicing)]
fn wrap_line(text: &str, max_chars: usize) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max_chars || max_chars == 0 {
        return vec![text.to_string()];
    }
    let mut lines = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        let end = start + max_chars;
        if end >= chars.len() {
            lines.push(chars[start..].iter().collect());
            break;
        }
        let mut split = end;
        for i in (start..end).rev() {
            if chars[i] == ' ' {
                split = i;
                break;
            }
        }
        if split == start || chars[start..end].iter().all(|&c| c != ' ') {
            lines.push(chars[start..end].iter().collect());
            start = end;
        } else {
            lines.push(chars[start..split].iter().collect());
            start = split + 1;
        }
    }
    lines
}

fn build_conversation_lines(state: &UiState, max_width: usize) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for entry in &state.conversation {
        match entry {
            ConversationEntry::UserInput { text } => {
                let wrap = max_width.saturating_sub(4);
                lines.push(Line::from(vec![Span::styled(
                    ">>> ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )]));
                for line in text.lines() {
                    for w in wrap_line(line, wrap) {
                        lines.push(Line::from(vec![Span::styled(
                            w,
                            Style::default().fg(Color::White),
                        )]));
                    }
                }
                lines.push(Line::raw(""));
            }
            ConversationEntry::Thinking { content } => {
                lines.push(Line::from(vec![Span::styled(
                    "Thinking",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )]));
                let wrap = max_width.saturating_sub(2);
                for line in content.lines() {
                    for w in wrap_line(line, wrap) {
                        lines.push(Line::from(vec![Span::styled(
                            format!("  {w}"),
                            Style::default().fg(Color::DarkGray),
                        )]));
                    }
                }
                lines.push(Line::raw(""));
            }
            ConversationEntry::ContentStreaming { content } => {
                if content.is_empty() {
                    lines.push(Line::from(vec![Span::styled(
                        "Generating...",
                        Style::default().fg(Color::DarkGray),
                    )]));
                } else {
                    for line in content.lines() {
                        for w in wrap_line(line, max_width) {
                            lines.push(Line::from(vec![Span::styled(
                                w,
                                Style::default().fg(Color::White),
                            )]));
                        }
                    }
                }
            }
            ConversationEntry::ToolCall {
                tool_name,
                arg_preview,
                ..
            } => {
                lines.push(Line::from(vec![Span::styled(
                    format!("[{tool_name}]"),
                    Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::BOLD),
                )]));
                if !arg_preview.is_empty() {
                    for w in wrap_line(arg_preview, max_width.saturating_sub(2)) {
                        lines.push(Line::from(vec![Span::styled(
                            format!("  {w}"),
                            Style::default().fg(Color::DarkGray),
                        )]));
                    }
                }
            }
            ConversationEntry::ToolResult {
                tool_name,
                preview,
                success,
                ..
            } => {
                let status = if *success { "OK" } else { "ERR" };
                let color = if *success { Color::Green } else { Color::Red };
                lines.push(Line::from(vec![Span::styled(
                    format!("  {status} {tool_name} "),
                    Style::default().fg(color),
                )]));
                let wrap = max_width.saturating_sub(4);
                for line in preview.lines().take(6) {
                    for w in wrap_line(line, wrap) {
                        lines.push(Line::from(vec![Span::styled(
                            format!("    {w}"),
                            Style::default().fg(Color::DarkGray),
                        )]));
                    }
                }
                lines.push(Line::raw(""));
            }
            ConversationEntry::AgentAnswer { text } => {
                lines.push(Line::raw(""));
                for line in text.lines() {
                    for w in wrap_line(line, max_width) {
                        lines.push(Line::from(vec![Span::styled(
                            w,
                            Style::default().fg(Color::White),
                        )]));
                    }
                }
                lines.push(Line::raw(""));
            }
            ConversationEntry::RunSummary {
                iterations,
                tool_calls,
                elapsed_ms,
            } => {
                lines.push(Line::from(vec![Span::styled(
                    format!(
                        "Done · {} iteration{} · {} tool call{} · {}ms",
                        iterations,
                        if *iterations == 1 { "" } else { "s" },
                        tool_calls,
                        if *tool_calls == 1 { "" } else { "s" },
                        elapsed_ms
                    ),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )]));
            }
            ConversationEntry::Error { message } => {
                lines.push(Line::from(vec![Span::styled(
                    format!("Error: {message}"),
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )]));
            }
            ConversationEntry::EntryCheckpoint {
                reason,
                note,
                created_at,
            } => {
                let label = format!("[Checkpoint: {created_at}] {reason}");
                let label = note
                    .as_ref()
                    .map(|n| format!("{label} ({n})"))
                    .unwrap_or(label);
                lines.push(Line::from(vec![Span::styled(
                    label,
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::DIM),
                )]));
            }
            ConversationEntry::RunningBanner { run_id } => {
                lines.push(Line::from(vec![Span::styled(
                    format!("\u{2b24} Agent running  [{run_id}]"),
                    Style::default()
                        .fg(Color::LightBlue)
                        .add_modifier(Modifier::BOLD),
                )]));
            }
        }
    }
    lines
}

// === Tools Panel ============================================================

fn render_tools_panel(frame: &mut Frame, area: Rect, state: &UiState) {
    let title = format!(" Tools Called ({}) ", state.tool_calls.len());
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .style(Style::default().fg(Color::Blue));

    if state.tool_calls.is_empty() {
        let empty = Paragraph::new("No tool calls yet")
            .style(Style::default().fg(Color::DarkGray))
            .block(block.clone());
        frame.render_widget(empty, area);
        return;
    }

    let items: Vec<ListItem> = state
        .tool_calls
        .iter()
        .map(|entry| {
            let (status_str, status_color) = match entry.status {
                ToolCallStatus::Running => ("...", Color::Yellow),
                ToolCallStatus::Success => ("OK", Color::Green),
                ToolCallStatus::Error => ("ERR", Color::Red),
                ToolCallStatus::Skipped => ("SKIP", Color::DarkGray),
            };
            ListItem::new(vec![
                Line::from(vec![Span::styled(
                    format!("{}. [{}]  {}", entry.sequence, entry.tool_name, status_str),
                    Style::default()
                        .fg(status_color)
                        .add_modifier(Modifier::BOLD),
                )]),
                Line::from(vec![Span::styled(
                    &entry.arg_preview,
                    Style::default().fg(Color::DarkGray),
                )]),
            ])
        })
        .collect();

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

// === Input Area + Approval ==================================================

fn render_input_area(frame: &mut Frame, area: Rect, state: &UiState) {
    if area.height < 3 {
        return;
    }

    if state.approval_state.has_pending() {
        render_approval_panel(frame, area, state);
    } else {
        render_textarea_hints(frame, area, state);
    }
}

fn render_textarea_hints(frame: &mut Frame, area: Rect, state: &UiState) {
    let hint_area = Rect {
        x: area.x,
        y: area.y + area.height - 1,
        width: area.width,
        height: 1,
    };
    let text_area = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: area.height - 1,
    };

    let block = Block::default().borders(Borders::ALL).title(" Input ");
    let inner = block.inner(text_area);
    frame.render_widget(block, text_area);

    // Placeholder: "Type here" since we don't have ratatui-textarea in vol-llm-ui yet
    // The TUI bin will use ratatui-textarea for actual input
    let placeholder = Paragraph::new("Type here (input handled by TUI event loop)")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(placeholder, inner);

    let hint = if state.is_running {
        Line::from(vec![Span::styled(
            " Running... (input disabled) ",
            Style::default().fg(Color::Yellow),
        )])
    } else {
        Line::from(vec![
            Span::styled(" Enter ", Style::default().fg(Color::Blue)),
            Span::styled("Send  ", Style::default().fg(Color::DarkGray)),
            Span::styled(" Esc ", Style::default().fg(Color::Blue)),
            Span::styled("Clear", Style::default().fg(Color::DarkGray)),
        ])
    };
    frame.render_widget(Paragraph::new(hint), hint_area);
}

fn render_approval_panel(frame: &mut Frame, area: Rect, state: &UiState) {
    let tool_name = state
        .approval_state
        .tool_name
        .as_deref()
        .unwrap_or("unknown");
    let arguments = state.approval_state.arguments.as_deref().unwrap_or("");

    let block = Block::default().borders(Borders::ALL).title(" Approval ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let text = Text::from(vec![
        Line::from(vec![Span::styled(
            format!(" [!] {tool_name}"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![Span::styled(
            format!("  {}", arguments.chars().take(100).collect::<String>()),
            Style::default().fg(Color::DarkGray),
        )]),
        Line::raw(""),
        Line::from(vec![
            Span::styled(
                " [A] ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("Approve  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                " [R] ",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled("Reject  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                " [S] ",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled("Stop", Style::default().fg(Color::DarkGray)),
        ]),
    ]);

    frame.render_widget(Paragraph::new(text), inner);
}

// === Workspace Panel ========================================================

fn flatten_tree_for_tui(
    node: &crate::state::WorkspaceTreeNode,
    indent: usize,
) -> Vec<(String, bool, usize)> {
    let mut result = Vec::new();
    for child in &node.children {
        result.push((child.name.clone(), child.is_dir, indent));
        if child.is_dir {
            result.extend(flatten_tree_for_tui(child, indent + 1));
        }
    }
    result
}

fn render_workspace(frame: &mut Frame, area: Rect, state: &UiState) {
    let block = Block::default().borders(Borders::ALL).title(" Workspace ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.workspace.children.is_empty() && !state.workspace.loaded {
        let empty = Paragraph::new("Workspace directory empty or unavailable")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(empty, inner);
        return;
    }

    let entries = flatten_tree_for_tui(&state.workspace, 0);
    let lines: Vec<Line> = entries
        .iter()
        .map(|(name, is_dir, indent)| {
            let prefix = if *is_dir {
                format!("{}[DIR] {}", "  ".repeat(*indent), name)
            } else {
                format!("{}[FILE] {}", "  ".repeat(*indent), name)
            };
            let style = if *is_dir {
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            Line::from(vec![Span::styled(prefix, style)])
        })
        .collect();

    let paragraph = Paragraph::new(Text::from(lines)).scroll((state.workspace_scroll, 0));
    frame.render_widget(paragraph, inner);
}

// === Log Viewer =============================================================

fn render_log_viewer(frame: &mut Frame, area: Rect, state: &UiState) {
    if state.log_viewer_selected_run.is_some() {
        render_log_entries(frame, area, state);
    } else {
        render_run_list(frame, area, state);
    }
}

fn render_run_list(frame: &mut Frame, area: Rect, state: &UiState) {
    let mut lines = Vec::new();
    if state.log_viewer_run_logs.is_empty() {
        lines.push(Line::from(Span::styled(
            " No log files found.",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for run in &state.log_viewer_run_logs {
            let short_id = if run.run_id.len() > 12 {
                &run.run_id[..12]
            } else {
                &run.run_id
            };
            lines.push(Line::from(vec![
                Span::styled(format!(" {short_id:<14}"), Style::default().fg(Color::Gray)),
                Span::styled(
                    format!(" {:>5} events", run.event_count),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("  {}", run.last_event),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!(" ({})", run.last_event_time),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " Enter to view  Esc to go back",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph =
        Paragraph::new(lines).block(Block::default().title(" Log Runs ").borders(Borders::ALL));
    frame.render_widget(paragraph, area);
}

fn render_log_entries(frame: &mut Frame, area: Rect, state: &UiState) {
    let lines: Vec<Line> = state
        .log_viewer_entries
        .iter()
        .map(|entry| {
            let color = match entry.event_type.as_str() {
                "AgentStart" | "AgentComplete" => Color::Green,
                "ToolCallBegin" | "ToolCallComplete" => Color::Yellow,
                "ToolCallError" | "AgentAborted" => Color::Red,
                _ => Color::White,
            };
            Line::from(vec![
                Span::styled(
                    format!("[{}] ", entry.timestamp),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(entry.event_type.clone(), Style::default().fg(color)),
                Span::styled(format!(" -- {}", entry.summary), Style::default().fg(color)),
            ])
        })
        .collect();

    let run_id = state.log_viewer_selected_run.as_deref().unwrap_or("");
    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .title(format!(" Log: {run_id} "))
                .borders(Borders::ALL),
        )
        .scroll((state.log_viewer_scroll, 0));
    frame.render_widget(paragraph, area);
}

// === Skills Panel ===========================================================

fn render_skills(frame: &mut Frame, area: Rect, state: &UiState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Skills ({}) ", state.skills.len()));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.skills.is_empty() {
        let empty =
            Paragraph::new("No skills discovered").style(Style::default().fg(Color::DarkGray));
        frame.render_widget(empty, inner);
        return;
    }

    let max_width = inner.width as usize;
    let name_w = 22.min(max_width.saturating_sub(20));
    let version_w = 8.min(max_width.saturating_sub(name_w + 12));
    let scope_w = 10.min(max_width.saturating_sub(name_w + version_w + 4));
    let desc_w = max_width.saturating_sub(name_w + version_w + scope_w + 4);

    let lines: Vec<Line> = state
        .skills
        .iter()
        .map(|s| {
            let scope_color = match s.scope.as_str() {
                "User" => Color::Green,
                "Repo" => Color::Blue,
                _ => Color::Yellow,
            };
            Line::from(vec![
                Span::styled(
                    pad_or_truncate(&s.name, name_w),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" | "),
                Span::styled(
                    pad_or_truncate(&s.version, version_w),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw(" | "),
                Span::styled(
                    pad_or_truncate(&s.scope, scope_w),
                    Style::default().fg(scope_color),
                ),
                Span::raw(" | "),
                Span::styled(
                    pad_or_truncate(&s.description, desc_w),
                    Style::default().fg(Color::DarkGray),
                ),
            ])
        })
        .collect();

    frame.render_widget(Paragraph::new(Text::from(lines)), inner);
}

fn render_agents_panel(frame: &mut Frame, area: Rect, _state: &UiState) {
    let block = Block::default().borders(Borders::ALL).title(" Agents ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let placeholder = Paragraph::new("Agents panel — use the web UI to browse agents")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    frame.render_widget(placeholder, inner);
}

fn render_sessions_panel(frame: &mut Frame, area: Rect, _state: &UiState) {
    let block = Block::default()
        .title("Sessions")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let text = Text::raw("No sessions (TUI)");
    let paragraph = Paragraph::new(text)
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(paragraph, inner);
}

fn render_mcp(frame: &mut Frame, area: Rect, _state: &UiState) {
    let block = Block::default()
        .title("MCP Servers")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let text = Text::raw("MCP tab (TUI)");
    let paragraph = Paragraph::new(text)
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(paragraph, inner);
}

fn render_tasks_placeholder(frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .title("Tasks")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let text = Text::raw("Tasks tab (TUI — coming soon)");
    let paragraph = Paragraph::new(text)
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(paragraph, inner);
}

fn pad_or_truncate(s: &str, width: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= width {
        format!("{}{}", s, " ".repeat(width.saturating_sub(char_count)))
    } else {
        format!(
            "{}...",
            s.chars().take(width.saturating_sub(1)).collect::<String>()
        )
    }
}

// === Session Dialog =========================================================

#[allow(clippy::cast_possible_truncation)]
fn render_session_dialog(frame: &mut Frame, area: Rect, state: &UiState) {
    if !state.session_dialog_open {
        return;
    }

    let width = 60.min(area.width);
    let height = (state.session_dialog_sessions.len() as u16 + 6).min(area.height - 2);
    let x = (area.width - width) / 2;
    let y = (area.height - height) / 2;
    let rect = Rect::new(x, y, width, height);

    frame.render_widget(ratatui::widgets::Clear, rect);

    let mut lines = Vec::new();
    if state.session_dialog_sessions.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No saved sessions found.",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for (i, entry) in state.session_dialog_sessions.iter().enumerate() {
            let is_selected = i == state.session_dialog_selected;
            let prefix = if is_selected { "> " } else { "  " };
            let short_id = if entry.session_id.len() > 8 {
                &entry.session_id[..8]
            } else {
                &entry.session_id
            };
            let style = if is_selected {
                Style::default()
                    .fg(Color::White)
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            lines.push(Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(format!("{short_id:<10}"), style),
                Span::styled(
                    format!(" {:>4} entries", entry.entry_count),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("    {}", entry.age_label),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " [n] New  [Enter] Resume  [d] Delete  [Esc] Cancel",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .title(" Sessions (Ctrl+S to dismiss) ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow)),
    );
    frame.render_widget(paragraph, rect);
}
