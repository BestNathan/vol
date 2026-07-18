//! Session list dialog overlay rendering.

use crate::app::AppState;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

#[allow(clippy::cast_possible_truncation)]
pub fn render_session_dialog(frame: &mut Frame, area: Rect, state: &AppState) {
    if !state.session_dialog.open {
        return;
    }

    let width = 60.min(area.width);
    let height = (state.session_dialog.sessions.len() as u16 + 6).min(area.height - 2);
    let x = (area.width - width) / 2;
    let y = (area.height - height) / 2;

    let rect = Rect::new(x, y, width, height);

    frame.render_widget(ratatui::widgets::Clear, rect);

    let lines = build_dialog_lines(state);
    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .title(" Sessions (Ctrl+S to dismiss) ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow)),
    );

    frame.render_widget(paragraph, rect);
}

fn build_dialog_lines(state: &AppState) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    if state.session_dialog.sessions.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No saved sessions found.",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for (i, entry) in state.session_dialog.sessions.iter().enumerate() {
            let is_selected = i == state.session_dialog.selected;
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

    lines
}
