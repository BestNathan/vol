//! Tools panel widget — left side, 30% width.

use crate::app::{AppState, ToolCallStatus};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem};
use ratatui::text::{Line, Span};

/// Render the tools panel.
pub fn render_tools_panel(frame: &mut Frame, area: Rect, state: &AppState) {
    let title = format!(" Tools Called ({}) ", state.tool_calls.len());
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .style(Style::default().fg(Color::Blue));

    let items: Vec<ListItem> = state.tool_calls
        .iter()
        .map(|entry| {
            let status_str = match entry.status {
                ToolCallStatus::Running => "…",
                ToolCallStatus::Success => "✓",
                ToolCallStatus::Error => "✗",
                ToolCallStatus::Skipped => "⊘",
            };

            let status_color = match entry.status {
                ToolCallStatus::Running => Color::Yellow,
                ToolCallStatus::Success => Color::Green,
                ToolCallStatus::Error => Color::Red,
                ToolCallStatus::Skipped => Color::DarkGray,
            };

            let lines = vec![
                Line::from(vec![
                    Span::styled(
                        format!("{}. [{}]  {}", entry.sequence, entry.tool_name, status_str),
                        Style::default()
                            .fg(status_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(vec![
                    Span::styled(
                        &entry.arg_preview,
                        Style::default().fg(Color::DarkGray),
                    ),
                ]),
            ];

            ListItem::new(lines)
        })
        .collect();

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}
