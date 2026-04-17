//! Approval banner widget — displayed in conversation when a tool requires HITL approval.

use crate::app::AppState;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph};

/// Render the approval banner in the conversation panel.
/// Returns true if a banner was rendered.
pub fn render_approval_banner(frame: &mut Frame, area: Rect, state: &AppState) -> bool {
    if !state.approval_state.has_pending_approval() {
        return false;
    }

    // Get the tool name and reason for display (sync via try_lock)
    let tool_name = state.approval_state.tool_name
        .try_lock()
        .ok()
        .and_then(|g| g.clone())
        .unwrap_or_else(|| "unknown".to_string());

    let reason = state.approval_state.reason
        .try_lock()
        .ok()
        .and_then(|g| g.clone())
        .unwrap_or_default();

    let arguments_preview = state.approval_state.arguments
        .try_lock()
        .ok()
        .and_then(|g| {
            g.as_ref().map(|s| {
                if s.len() > 80 {
                    format!("{}...", &s[..77])
                } else {
                    s.clone()
                }
            })
        })
        .unwrap_or_default();

    let banner_height = 5u16;
    if area.height < banner_height {
        return false;
    }

    // Position banner near the bottom of the visible area
    let banner_area = Rect {
        x: area.x,
        y: area.y + area.height - banner_height,
        width: area.width,
        height: banner_height,
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Approval Required ")
        .style(Style::default().fg(Color::Yellow));

    let inner = block.inner(banner_area);
    frame.render_widget(block, banner_area);

    let text = Text::from(vec![
        Line::from(vec![
            Span::styled("! ", Style::default().fg(Color::Yellow)),
            Span::styled(&tool_name, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled(format!("  {}", reason), Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled("  ", Style::default().fg(Color::DarkGray)),
            Span::styled(&arguments_preview, Style::default().fg(Color::DarkGray)),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled(" [A] ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled("Approve   ", Style::default().fg(Color::DarkGray)),
            Span::styled(" [R] ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::styled("Reject   ", Style::default().fg(Color::DarkGray)),
            Span::styled(" [S] ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::styled("Stop", Style::default().fg(Color::DarkGray)),
        ]),
    ]);

    let paragraph = Paragraph::new(text);
    frame.render_widget(paragraph, inner);

    true
}
