//! Conversation tab widget — scrollable message view.

use crate::app::{AppState, ConversationEntry};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

/// Render the conversation panel.
pub fn render_conversation(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Conversation ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.conversation.is_empty() {
        let empty = Paragraph::new("No messages yet. Type a query and press Ctrl+Enter.")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(empty, inner);
        return;
    }

    let lines = build_conversation_lines(state, inner.width as usize);
    let total_lines = lines.len();
    let visible_height = inner.height as usize;

    // Calculate scroll offset
    let scroll = if state.conversation_auto_scroll {
        // Auto-scroll: show bottom of content
        total_lines.saturating_sub(visible_height)
    } else {
        // Manual scroll: use stored offset, clamped to content bounds
        (state.conversation_scroll as usize).min(total_lines.saturating_sub(1))
    };

    // Slice only the visible lines to prevent overflow beyond widget bounds
    let visible_lines: Vec<Line> = lines
        .into_iter()
        .skip(scroll)
        .take(visible_height)
        .collect();

    let text = Text::from(visible_lines);
    let paragraph = Paragraph::new(text);
    frame.render_widget(paragraph, inner);
}

/// Wrap a line into multiple lines at `max_chars` boundary, breaking at word boundaries.
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
        // Find last space within range
        let mut split = end;
        for i in (start..end).rev() {
            if chars[i] == ' ' {
                split = i;
                break;
            }
        }
        if split == start || chars[start..end].iter().all(|&c| c != ' ') {
            // No space found, hard break
            lines.push(chars[start..end].iter().collect());
            start = end;
        } else {
            lines.push(chars[start..split].iter().collect());
            start = split + 1; // skip the space
        }
    }
    lines
}

fn build_conversation_lines<'a>(state: &'a AppState, max_width: usize) -> Vec<Line<'a>> {
    let mut lines = Vec::new();

    for entry in &state.conversation {
        match entry {
            ConversationEntry::UserInput { text } => {
                let wrap = max_width.saturating_sub(4); // ">>> "
                lines.push(Line::from(vec![Span::styled(
                    ">>> ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )]));
                for line in text.lines() {
                    for wrapped in wrap_line(line, wrap) {
                        lines.push(Line::from(vec![Span::styled(
                            wrapped,
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
                    for wrapped in wrap_line(line, wrap) {
                        lines.push(Line::from(vec![Span::styled(
                            format!("  {wrapped}"),
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
                        for wrapped in wrap_line(line, max_width) {
                            lines.push(Line::from(vec![Span::styled(
                                wrapped,
                                Style::default().fg(Color::White),
                            )]));
                        }
                    }
                }
            }
            ConversationEntry::ToolCall {
                tool_name,
                arg_preview,
            } => {
                lines.push(Line::from(vec![Span::styled(
                    format!("[{tool_name}]"),
                    Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::BOLD),
                )]));
                if !arg_preview.is_empty() {
                    for wrapped in wrap_line(arg_preview, max_width.saturating_sub(2)) {
                        lines.push(Line::from(vec![Span::styled(
                            format!("  {wrapped}"),
                            Style::default().fg(Color::DarkGray),
                        )]));
                    }
                }
            }
            ConversationEntry::ToolResult {
                tool_name,
                preview,
                success,
            } => {
                let status = if *success { "OK" } else { "ERR" };
                let color = if *success { Color::Green } else { Color::Red };
                lines.push(Line::from(vec![Span::styled(
                    format!("  {status} {tool_name} "),
                    Style::default().fg(color),
                )]));
                let wrap = max_width.saturating_sub(4);
                for line in preview.lines().take(6) {
                    for wrapped in wrap_line(line, wrap) {
                        lines.push(Line::from(vec![Span::styled(
                            format!("    {wrapped}"),
                            Style::default().fg(Color::DarkGray),
                        )]));
                    }
                }
                lines.push(Line::raw(""));
            }
            ConversationEntry::AgentAnswer { text } => {
                lines.push(Line::raw(""));
                for line in text.lines() {
                    for wrapped in wrap_line(line, max_width) {
                        lines.push(Line::from(vec![Span::styled(
                            wrapped,
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
                        elapsed_ms,
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
        }
    }

    lines
}
