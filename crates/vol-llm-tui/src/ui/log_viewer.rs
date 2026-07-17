//! Log viewer tab rendering.

use crate::app::{AppState, LogLine};
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

pub fn render_log_viewer(frame: &mut Frame, area: Rect, state: &AppState) {
    if state.log_viewer.selected_run.is_some() {
        render_log_entries(frame, area, state);
    } else {
        render_run_list(frame, area, state);
    }
}

fn render_run_list(frame: &mut Frame, area: Rect, state: &AppState) {
    let mut lines = Vec::new();

    if state.log_viewer.run_logs.is_empty() {
        lines.push(Line::from(Span::styled(
            " No log files found.",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for run in &state.log_viewer.run_logs {
            let is_selected = state.log_viewer.selected_run.as_deref() == Some(&run.run_id);
            let style = if is_selected {
                Style::default().fg(Color::White).bg(Color::DarkGray)
            } else {
                Style::default().fg(Color::Gray)
            };
            let short_id = if run.run_id.len() > 12 {
                &run.run_id[..12]
            } else {
                &run.run_id
            };
            lines.push(Line::from(vec![
                Span::styled(format!(" {short_id:<14}"), style),
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

fn render_log_entries(frame: &mut Frame, area: Rect, state: &AppState) {
    let lines = build_log_lines(&state.log_viewer.entries);
    let total_lines = lines.len();
    let scroll = compute_scroll(
        state.log_viewer.scroll,
        state.log_viewer.auto_scroll,
        total_lines,
        area.height,
    );

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .title(format!(
                    " Log: {} ",
                    state.log_viewer.selected_run.as_deref().unwrap_or("")
                ))
                .borders(Borders::ALL),
        )
        .scroll((scroll, 0));

    frame.render_widget(paragraph, area);
}

fn build_log_lines(entries: &[LogLine]) -> Vec<Line<'static>> {
    entries
        .iter()
        .map(|entry| {
            let color = event_color(&entry.event_type);
            Line::from(vec![
                Span::styled(
                    format!("[{}] ", entry.timestamp),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(entry.event_type.clone(), Style::default().fg(color)),
                Span::styled(format!(" — {}", entry.summary), Style::default().fg(color)),
            ])
        })
        .collect()
}

fn event_color(event_type: &str) -> Color {
    match event_type {
        "AgentStart" | "AgentComplete" => Color::Green,
        "ToolCallBegin" | "ToolCallComplete" => Color::Yellow,
        "ToolCallError" | "AgentAborted" => Color::Red,
        _ => Color::White,
    }
}

#[allow(clippy::cast_possible_truncation)]
fn compute_scroll(scroll: u16, auto_scroll: bool, total_lines: usize, view_height: u16) -> u16 {
    if auto_scroll && total_lines > view_height as usize {
        (total_lines - view_height as usize) as u16
    } else {
        scroll.min(total_lines.saturating_sub(view_height as usize) as u16)
    }
}
