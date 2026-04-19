# Approval Panel Replace Input Box Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the input box area with an approval panel when HITL is triggered, and remove the floating approval banner.

**Architecture:** `render_input_area()` in `input_area.rs` checks `has_pending_approval()` and renders either the approval panel or the textarea. Delete `approval_banner.rs` and all references to it.

**Tech Stack:** ratatui 0.30, crossterm 0.29

---

### Task 1: Replace input_area.rs with approval panel support

**Files:**
- Modify: `crates/vol-llm-tui/src/ui/input_area.rs`

- [ ] **Step 1: Rewrite input_area.rs to support approval panel**

Replace the entire file content with:

```rust
//! Input area widget — bottom of right panel, multi-line text input.
//! When an approval is pending, the entire area is replaced with an approval panel.

use crate::app::AppState;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

/// Render the input area at the bottom of the right panel.
/// If approval is pending, renders an approval panel instead of the textarea.
pub fn render_input_area(frame: &mut Frame, area: Rect, state: &AppState) {
    if area.height < 3 {
        return;
    }

    if state.approval_state.has_pending_approval() {
        render_approval_panel(frame, area, state);
    } else {
        render_textarea(frame, area, state);
    }
}

fn render_textarea(frame: &mut Frame, area: Rect, state: &AppState) {
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
    let hint = if state.is_running {
        Line::from(vec![
            Span::styled(
                " Running... (input disabled) ",
                Style::default().fg(Color::Yellow),
            ),
        ])
    } else if state.unsafe_mode {
        Line::from(vec![
            Span::styled(" Enter ", Style::default().fg(Color::Blue)),
            Span::styled("Send  ", Style::default().fg(Color::DarkGray)),
            Span::styled(" Ctrl+U ", Style::default().fg(Color::Yellow)),
            Span::styled("Unsafe  ", Style::default().fg(Color::DarkGray)),
            Span::styled(" Ctrl+Q ", Style::default().fg(Color::Red)),
            Span::styled("Quit", Style::default().fg(Color::DarkGray)),
        ])
    } else {
        Line::from(vec![
            Span::styled(" Enter ", Style::default().fg(Color::Blue)),
            Span::styled("Send  ", Style::default().fg(Color::DarkGray)),
            Span::styled(" Esc ", Style::default().fg(Color::Blue)),
            Span::styled("Clear  ", Style::default().fg(Color::DarkGray)),
            Span::styled(" Ctrl+U ", Style::default().fg(Color::Yellow)),
            Span::styled("Unsafe  ", Style::default().fg(Color::DarkGray)),
            Span::styled(" Ctrl+Q ", Style::default().fg(Color::Red)),
            Span::styled("Quit", Style::default().fg(Color::DarkGray)),
        ])
    };

    let hint_paragraph = Paragraph::new(hint).wrap(Wrap { trim: false });
    frame.render_widget(hint_paragraph, hint_area);
}

fn render_approval_panel(frame: &mut Frame, area: Rect, state: &AppState) {
    // Read approval fields synchronously
    let tool_name = state.approval_state.tool_name
        .try_lock()
        .ok()
        .and_then(|g| g.clone())
        .unwrap_or_else(|| "unknown".to_string());

    let arguments_preview = state.approval_state.arguments
        .try_lock()
        .ok()
        .and_then(|g| g.as_ref().map(|s| extract_command_preview(s)))
        .unwrap_or_default();

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Input ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let tool_line = Line::from(vec![
        Span::styled(
            format!(" \u{26A0} {}", tool_name),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ),
    ]);

    let cmd_line = if arguments_preview.is_empty() {
        Line::from(Span::styled(" ", Style::default().fg(Color::DarkGray)))
    } else {
        Line::from(vec![
            Span::styled(format!("  {}", arguments_preview), Style::default().fg(Color::DarkGray)),
        ])
    };

    let actions = Line::from(vec![
        Span::styled(" [A] ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::styled("Approve  ", Style::default().fg(Color::DarkGray)),
        Span::styled(" [R] ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        Span::styled("Reject  ", Style::default().fg(Color::DarkGray)),
        Span::styled(" [S] ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        Span::styled("Stop", Style::default().fg(Color::DarkGray)),
    ]);

    let text = ratatui::text::Text::from(vec![
        tool_line,
        cmd_line,
        Line::raw(""),
        actions,
    ]);

    let paragraph = Paragraph::new(text).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

fn extract_command_preview(arguments: &str) -> String {
    // Try to parse JSON and extract command field
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(arguments) {
        if let Some(cmd) = parsed.get("command").and_then(|v| v.as_str()) {
            return truncate(cmd, 100);
        }
        if let Some(path) = parsed.get("path").and_then(|v| v.as_str()) {
            return format!("Path: {}", path);
        }
        if let Some(file_path) = parsed.get("file_path").and_then(|v| v.as_str()) {
            return format!("File: {}", file_path);
        }
        // Fall back to pretty-printed JSON snippet
        return truncate(&serde_json::to_string_pretty(&parsed).unwrap_or_default(), 100);
    }
    // Raw string fallback
    truncate(arguments, 60)
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len - 3])
    } else {
        s.to_string()
    }
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p vol-llm-tui 2>&1 | head -20`
Expected: Compiles successfully (approval_banner still referenced elsewhere, see next task)

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-tui/src/ui/input_area.rs
git commit -m "feat: input_area supports approval panel rendering"
```

---

### Task 2: Remove approval_banner.rs and all references

**Files:**
- Delete: `crates/vol-llm-tui/src/ui/approval_banner.rs`
- Modify: `crates/vol-llm-tui/src/ui/mod.rs`
- Modify: `crates/vol-llm-tui/src/ui/conversation.rs`

- [ ] **Step 1: Delete approval_banner.rs**

```bash
rm crates/vol-llm-tui/src/ui/approval_banner.rs
```

- [ ] **Step 2: Remove approval_banner from mod.rs**

In `crates/vol-llm-tui/src/ui/mod.rs`, remove these lines:

```rust
mod approval_banner;
```

```rust
pub use approval_banner::render_approval_banner;
```

- [ ] **Step 3: Remove render_approval_banner call from conversation.rs**

In `crates/vol-llm-tui/src/ui/conversation.rs`, find the `render_conversation` function and remove the approval banner render call at the end:

Remove these lines:
```rust
    // Render approval banner overlay (drawn on top of conversation content)
    super::render_approval_banner(frame, inner, state);
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p vol-llm-tui 2>&1 | head -20`
Expected: Compiles successfully with no errors

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-tui/src/ui/approval_banner.rs crates/vol-llm-tui/src/ui/mod.rs crates/vol-llm-tui/src/ui/conversation.rs
git commit -m "feat: remove approval_banner, approval panel lives in input_area"
```

---

### Task 3: Full workspace verification

- [ ] **Step 1: Full workspace check**

Run: `cargo check --workspace 2>&1 | tail -20`
Expected: Compiles successfully with no new errors

- [ ] **Step 2: Build**

Run: `cargo build -p vol-llm-tui 2>&1 | tail -10`
Expected: Builds successfully

- [ ] **Step 3: Commit (if no prior commit)**

```bash
git add .
git commit -m "chore: full workspace verification"
```
