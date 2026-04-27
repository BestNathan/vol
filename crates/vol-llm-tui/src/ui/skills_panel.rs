//! Skills panel widget — scrollable table of loaded skills.

use crate::app::SkillDisplayEntry;
use crate::app::AppState;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::text::{Line, Span, Text};

/// Render the skills panel.
pub fn render_skills(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Skills ({}) ", state.skills.len()));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.skills.is_empty() {
        let empty = Paragraph::new("No skills discovered")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(empty, inner);
        return;
    }

    // Compute column widths
    let max_width = inner.width as usize;
    let name_width = 22.min(max_width.saturating_sub(20));
    let version_width = 8.min(max_width.saturating_sub(name_width + 12));
    let scope_width = 10.min(max_width.saturating_sub(name_width + version_width + 4));
    let desc_width = max_width.saturating_sub(name_width + version_width + scope_width + 4);

    let lines: Vec<Line> = state.skills
        .iter()
        .map(|entry| render_skill_row(entry, name_width, version_width, scope_width, desc_width))
        .collect();

    let text = Text::from(lines);
    let paragraph = Paragraph::new(text);
    frame.render_widget(paragraph, inner);
}

fn render_skill_row(
    entry: &SkillDisplayEntry,
    name_width: usize,
    version_width: usize,
    scope_width: usize,
    desc_width: usize,
) -> Line<'static> {
    let name = pad_or_truncate(&entry.name, name_width);
    let version = pad_or_truncate(&entry.version, version_width);
    let scope = pad_or_truncate(&entry.scope, scope_width);
    let desc = pad_or_truncate(&entry.description, desc_width);

    let scope_color = match entry.scope.as_str() {
        "User" => Color::Green,
        "Repo" => Color::Blue,
        _ => Color::Yellow,
    };

    Line::from(vec![
        Span::styled(name, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::raw(" | "),
        Span::styled(version, Style::default().fg(Color::DarkGray)),
        Span::raw(" | "),
        Span::styled(scope, Style::default().fg(scope_color)),
        Span::raw(" | "),
        Span::styled(desc, Style::default().fg(Color::DarkGray)),
    ])
}

fn pad_or_truncate(s: &str, width: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= width {
        let padding = " ".repeat(width.saturating_sub(char_count));
        format!("{}{}", s, padding)
    } else {
        let truncated: String = s.chars().take(width.saturating_sub(1)).collect();
        format!("{}\u{2026}", truncated)
    }
}
