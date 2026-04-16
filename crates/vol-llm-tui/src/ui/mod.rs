//! UI layout orchestration.

mod status_bar;
mod tools_panel;

pub use status_bar::render_status_bar;
pub use tools_panel::render_tools_panel;

use crate::app::{ActiveTab, AppState};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};

/// Render the full UI to the frame.
pub fn render_ui(frame: &mut Frame, state: &AppState) {
    let area = frame.area();

    // Status bar: 3 rows at top
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // status bar
            Constraint::Min(1),     // remaining area
        ])
        .split(area);

    // Render status bar
    render_status_bar(frame, chunks[0], state);

    // Split remaining area: tools panel (30%) | content panel (70%)
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(70),
        ])
        .split(chunks[1]);

    // Render tools panel
    render_tools_panel(frame, main_chunks[0], state);

    // Render content panel (tab-driven)
    match state.active_tab {
        ActiveTab::Conversation => {
            // Placeholder — Task 4 will implement
            frame.render_widget(
                ratatui::widgets::Paragraph::new("Conversation tab (coming soon)"),
                main_chunks[1],
            );
        }
        ActiveTab::Workspace => {
            // Placeholder — Task 4 will implement
            frame.render_widget(
                ratatui::widgets::Paragraph::new("Workspace tab (coming soon)"),
                main_chunks[1],
            );
        }
    }
}
