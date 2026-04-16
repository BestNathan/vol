//! Tools panel widget — left side, 30% width.

use crate::app::{AppState, ToolCallStatus};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::text::{Line, Span};

/// Render the tools panel.
pub fn render_tools_panel(frame: &mut Frame, area: Rect, state: &AppState) {
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

    let items: Vec<ListItem> = state.tool_calls
        .iter()
        .map(|entry| {
            let (status_str, status_color) = status_display(entry);

            let duration_str = entry.duration_ms
                .map(|ms| format!("{ms}ms"))
                .unwrap_or_default();

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

fn status_display(entry: &crate::app::ToolCallEntry) -> (&'static str, Color) {
    match entry.status {
        ToolCallStatus::Running => ("…", Color::Yellow),
        ToolCallStatus::Success => ("✓", Color::Green),
        ToolCallStatus::Error => ("ERR", Color::Red),
        ToolCallStatus::Skipped => ("SKIP", Color::DarkGray),
    }
}
