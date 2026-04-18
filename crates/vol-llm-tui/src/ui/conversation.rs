//! Conversation tab widget — scrollable message view.

use crate::app::{AppState, ConversationEntry};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::text::{Line, Span, Text};

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

    let lines = build_conversation_lines(state);
    let total_lines = lines.len() as u16;
    let visible_height = inner.height;
    let scroll = if state.conversation_scroll == u16::MAX {
        // Auto-scroll to bottom: skip lines that don't fit in the visible area
        total_lines.saturating_sub(visible_height)
    } else {
        state.conversation_scroll
    };
    let text = Text::from(lines);
    let paragraph = Paragraph::new(text);
    frame.render_widget(paragraph.scroll((scroll, 0)), inner);

    // Render approval banner overlay (drawn on top of conversation content)
    super::render_approval_banner(frame, inner, state);
}

fn build_conversation_lines<'a>(state: &'a AppState) -> Vec<Line<'a>> {
    let mut lines = Vec::new();

    for entry in &state.conversation {
        match entry {
            ConversationEntry::UserInput { text } => {
                lines.push(Line::from(vec![
                    Span::styled(">>> ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                    Span::styled(text.clone(), Style::default().fg(Color::White)),
                ]));
                lines.push(Line::raw(""));
            }
            ConversationEntry::ThinkingComplete { content } => {
                lines.push(Line::from(vec![
                    Span::styled("Thinking", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                ]));
                // Render accumulated thinking as a single wrapped block
                for line in content.lines() {
                    lines.push(Line::from(vec![
                        Span::styled(format!("  {}", line), Style::default().fg(Color::DarkGray)),
                    ]));
                }
                lines.push(Line::raw(""));
            }
            ConversationEntry::ToolCall { tool_name, arg_preview } => {
                lines.push(Line::from(vec![
                    Span::styled(format!("[{}]", tool_name), Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
                ]));
                if !arg_preview.is_empty() {
                    lines.push(Line::from(vec![
                        Span::styled(format!("  {}", arg_preview), Style::default().fg(Color::DarkGray)),
                    ]));
                }
            }
            ConversationEntry::ToolResult { tool_name, preview, success } => {
                let status = if *success { "OK" } else { "ERR" };
                let color = if *success { Color::Green } else { Color::Red };
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("  {} {} ", status, tool_name),
                        Style::default().fg(color),
                    ),
                ]));
                for line in preview.lines().take(6) {
                    lines.push(Line::from(vec![
                        Span::styled(format!("    {}", line), Style::default().fg(Color::DarkGray)),
                    ]));
                }
                lines.push(Line::raw(""));
            }
            ConversationEntry::AgentAnswer { text } => {
                lines.push(Line::raw(""));
                for line in text.lines() {
                    lines.push(Line::from(vec![
                        Span::styled(line, Style::default().fg(Color::White)),
                    ]));
                }
                lines.push(Line::raw(""));
            }
            ConversationEntry::RunSummary { iterations, tool_calls, elapsed_ms } => {
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
                    Span::styled(
                        format!("Error: {}", message),
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    ),
                ]));
            }
        }
    }

    lines
}
