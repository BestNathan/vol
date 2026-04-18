//! Status bar widget — fixed 3 rows at top.

use crate::app::AppState;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::Paragraph;

/// Render the status bar.
pub fn render_status_bar(frame: &mut Frame, area: Rect, state: &AppState) {
    let elapsed = state.run_start
        .map(|s| s.elapsed())
        .unwrap_or_default();
    let elapsed_secs = elapsed.as_secs();
    let time_str = format!("{:02}:{:02}", elapsed_secs / 60, elapsed_secs % 60);

    let mut status_parts = Vec::new();
    if state.is_running {
        status_parts.push("Running");
    } else {
        status_parts.push("Idle");
    }
    if state.unsafe_mode {
        status_parts.push("UNSAFE");
    }
    let status = status_parts.join(" · ");

    let prefix = if state.exiting { "QUITTING · " } else { "" };
    let unsafe_prefix = if state.unsafe_mode { "!! " } else { "" };
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

    let paragraph = Paragraph::new(text)
        .style(Style::default().fg(Color::White).bg(Color::DarkGray));

    frame.render_widget(paragraph, area);
}
