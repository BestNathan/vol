//! Workspace panel widget — file tree with modification markers.

use crate::app::AppState;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::text::{Line, Span, Text};

/// Render the workspace panel.
pub fn render_workspace(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Workspace ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.workspace.entries.is_empty() {
        let empty = Paragraph::new("Workspace directory empty or unavailable")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(empty, inner);
        return;
    }

    let lines: Vec<Line> = state.workspace.entries
        .iter()
        .map(|entry| {
            let indent = "  ".repeat(entry.indent);
            let prefix = if entry.is_dir {
                format!("{}{}{}", indent, dir_icon(), entry.path.split('/').last().unwrap_or(&entry.path))
            } else {
                let modified = if entry.modified {
                    " ✎ M"
                } else {
                    ""
                };
                format!("{}{}{}{}", indent, file_icon(), entry.path.split('/').last().unwrap_or(&entry.path), modified)
            };

            let style = if entry.is_dir {
                Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)
            } else if entry.modified {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::White)
            };

            Line::from(vec![Span::styled(prefix, style)])
        })
        .collect();

    let text = Text::from(lines);
    let paragraph = Paragraph::new(text);
    frame.render_widget(paragraph.scroll((state.workspace_scroll, 0)), inner);
}

fn dir_icon() -> &'static str {
    "📁 "
}

fn file_icon() -> &'static str {
    "📄 "
}
