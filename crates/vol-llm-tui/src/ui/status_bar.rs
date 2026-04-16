//! Status bar widget — fixed 3 rows at top.

use crate::app::AppState;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Paragraph};

/// Render the status bar.
pub fn render_status_bar(frame: &mut Frame, area: Rect, state: &AppState) {
    let elapsed = state.run_start
        .map(|s| s.elapsed())
        .unwrap_or_default();
    let elapsed_secs = elapsed.as_secs();
    let time_str = format!("{:02}:{:02}", elapsed_secs / 60, elapsed_secs % 60);

    let status = if state.is_running { "Running" } else { "Idle" };

    let text = format!(
        " Session: {} │ Run: {} │ Iter: {} │ Tools: {} │ Time: {} │ {}",
        state.session_id,
        state.run_count,
        state.iteration,
        state.tool_call_count,
        time_str,
        status,
    );

    let paragraph = Paragraph::new(text)
        .style(Style::default().fg(Color::White).bg(Color::DarkGray))
        .block(Block::default()
            .borders(Borders::ALL)
            .title(" vol-llm-tui ")
            .style(Style::default().fg(Color::Cyan)));

    frame.render_widget(paragraph, area);
}
