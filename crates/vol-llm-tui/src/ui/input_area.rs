//! Input area widget — bottom of right panel, multi-line text input.

use crate::app::AppState;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::text::{Line, Span};

/// Render the input area at the bottom of the right panel.
///
/// Layout:
/// - TextArea widget (fills the given area minus 1 row)
/// - Shortcut hint row (bottom row, dark gray)
pub fn render_input_area(frame: &mut Frame, area: Rect, state: &AppState) {
    if area.height < 3 {
        return;
    }

    // Split into: TextArea area (all but last row) + hint (last row)
    let hint_area = Rect {
        x: area.x,
        y: area.y + area.height - 1,
        width: area.width,
        height: 1,
    };
    let text_area = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: area.height - 1,
    };

    // Render outer border block
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Input ");
    let inner = block.inner(text_area);
    frame.render_widget(block, text_area);

    // Render the actual TextArea inside the block
    let mut textarea_widget = state.input.clone();
    textarea_widget.set_block(Block::default());
    frame.render_widget(&textarea_widget, inner);

    // Render shortcut hints
    let hint = if state.approval_state.has_pending_approval() {
        Line::from(vec![
            Span::styled(" [A] ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled("Approve  ", Style::default().fg(Color::DarkGray)),
            Span::styled(" [R] ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::styled("Reject  ", Style::default().fg(Color::DarkGray)),
            Span::styled(" [S] ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::styled("Stop", Style::default().fg(Color::DarkGray)),
        ])
    } else if state.is_running {
        Line::from(vec![
            Span::styled(
                " Running... (input disabled) ",
                Style::default().fg(Color::Yellow),
            ),
        ])
    } else if state.unsafe_mode {
        Line::from(vec![
            Span::styled(" Ctrl+Enter ", Style::default().fg(Color::Blue)),
            Span::styled("Send  ", Style::default().fg(Color::DarkGray)),
            Span::styled(" Ctrl+U ", Style::default().fg(Color::Yellow)),
            Span::styled("Unsafe  ", Style::default().fg(Color::DarkGray)),
            Span::styled(" Ctrl+Q ", Style::default().fg(Color::Red)),
            Span::styled("Quit", Style::default().fg(Color::DarkGray)),
        ])
    } else {
        Line::from(vec![
            Span::styled(" Ctrl+Enter ", Style::default().fg(Color::Blue)),
            Span::styled("Send  ", Style::default().fg(Color::DarkGray)),
            Span::styled(" Ctrl+U ", Style::default().fg(Color::Yellow)),
            Span::styled("Unsafe  ", Style::default().fg(Color::DarkGray)),
            Span::styled(" Ctrl+Q ", Style::default().fg(Color::Red)),
            Span::styled("Quit", Style::default().fg(Color::DarkGray)),
        ])
    };

    let hint_paragraph = Paragraph::new(hint);
    frame.render_widget(hint_paragraph, hint_area);
}
