//! UI layout orchestration.

mod conversation;
mod input_area;
mod status_bar;
mod tools_panel;
mod workspace_panel;

pub use conversation::render_conversation;
pub use input_area::render_input_area;
pub use status_bar::render_status_bar;
pub use tools_panel::render_tools_panel;
pub use workspace_panel::render_workspace;

use crate::app::{ActiveTab, AppState};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

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

    // Render content panel with tab bar
    render_right_panel(frame, main_chunks[1], state);
}

fn render_right_panel(frame: &mut Frame, area: Rect, state: &AppState) {
    // Tab bar: 1 row at top
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),  // tab bar
            Constraint::Min(1),     // tab content
        ])
        .split(area);

    // Render tab bar
    render_tab_bar(frame, chunks[0], state);

    // Render active tab content
    match state.active_tab {
        ActiveTab::Conversation => {
            render_conversation(frame, chunks[1], state);
        }
        ActiveTab::Workspace => {
            render_workspace(frame, chunks[1], state);
        }
    }
}

fn render_tab_bar(frame: &mut Frame, area: Rect, state: &AppState) {
    let active = &state.active_tab;

    let conv_style = if matches!(active, ActiveTab::Conversation) {
        Style::default().fg(Color::Black).bg(Color::White).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let ws_style = if matches!(active, ActiveTab::Workspace) {
        Style::default().fg(Color::Black).bg(Color::White).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let tabs = Line::from(vec![
        Span::raw(" "),
        Span::styled(" Conversation ", conv_style),
        Span::raw(" "),
        Span::styled(" Workspace ", ws_style),
        Span::raw(" "),
    ]);

    let paragraph = Paragraph::new(tabs)
        .block(Block::default().borders(Borders::BOTTOM));

    frame.render_widget(paragraph, area);
}
