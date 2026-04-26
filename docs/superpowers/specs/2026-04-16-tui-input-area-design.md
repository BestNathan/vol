# TUI Input Area & Exit Feedback Design

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a visible, interactive input area to the TUI and improve exit/quit feedback so users can type and exit reliably.

**Architecture:** The right panel layout gains a third vertical zone: tab bar → content → input area. The TextArea widget (already in AppState) is rendered via `ratatui-textarea`'s `Widget` impl. Keyboard routing already works; only the rendering is missing.

**Tech Stack:** ratatui 0.30, ratatui-textarea 0.8, existing AppState with `TextArea<'static>` input field

---

## Problem

1. **No input box** — `render_right_panel` only renders tab bar + content. The TextArea widget exists in `AppState.input` but is never drawn.
2. **Unclear exit** — Ctrl+Q exits silently; no visual feedback before terminal restores.

---

## Target Layout

```
┌──────────────┬────────────────────────────────┐
│              │ Conversation | Workspace        │
│ Tools Panel  │ ┌────────────────────────────┐ │
│   (30%)      │ │ [conversation content...]  │ │
│              │ │                            │ │
│              │ └────────────────────────────┘ │
│              │ ┌────────────────────────────┐ │
│              │ │ > Hello world              │ │
│              │ │   (4 rows TextArea)        │ │
│              │ └────────────────────────────┘ │
│              │ [Ctrl+Enter] [Esc] [Ctrl+Q]    │
└──────────────┴────────────────────────────────┘
```

---

## File Changes

| File | Action | Responsibility |
|------|--------|----------------|
| `crates/vol-llm-tui/src/ui/mod.rs` | Modify | `render_right_panel`: split into 3 chunks (tab bar, content, input area) |
| `crates/vol-llm-tui/src/ui/input_area.rs` | Create | Render input area: TextArea widget + shortcut hints |
| `crates/vol-llm-tui/src/main.rs` | Modify | Ctrl+Q: render "Quitting..." before exit |

---

### Task 1: Create input_area.rs widget

**Files:**
- Create: `crates/vol-llm-tui/src/ui/input_area.rs`
- Modify: `crates/vol-llm-tui/src/ui/mod.rs` (export)

- [ ] **Step 1: Create input_area.rs**

```rust
//! Input area widget — bottom of right panel, multi-line text input.

use crate::app::AppState;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::text::{Line, Span};
use ratatui_textarea::TextArea;

/// Render the input area at the bottom of the right panel.
///
/// Layout:
/// - TextArea widget (fills the given area minus 1 row)
/// - Shortcut hint row (bottom row, dark gray)
pub fn render_input_area(frame: &mut Frame, area: Rect, state: &AppState) {
    if area.height < 3 {
        // Too small to render meaningfully
        return;
    }

    // The input area is split into: TextArea (all but last row) + hint (last row)
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

    // Render TextArea widget with border
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Input ");
    let inner = block.inner(text_area);
    frame.render_widget(block, text_area);

    // Render the actual TextArea
    let mut textarea_widget = state.input.clone();
    textarea_widget.set_block(Block::default()); // already have outer block
    frame.render_widget(textarea_widget, inner);

    // Render shortcut hints
    let hint = if state.is_running {
        Line::from(vec![
            Span::styled(
                " Running... (input disabled) ",
                Style::default().fg(Color::Yellow),
            ),
        ])
    } else {
        Line::from(vec![
            Span::styled(" Ctrl+Enter ", Style::default().fg(Color::Blue)),
            Span::styled("Send  ", Style::default().fg(Color::DarkGray)),
            Span::styled(" Esc ", Style::default().fg(Color::Blue)),
            Span::styled("Clear  ", Style::default().fg(Color::DarkGray)),
            Span::styled(" Ctrl+Q ", Style::default().fg(Color::Red)),
            Span::styled("Quit", Style::default().fg(Color::DarkGray)),
        ])
    };

    let hint_paragraph = Paragraph::new(hint);
    frame.render_widget(hint_paragraph, hint_area);
}
```

- [ ] **Step 2: Export from mod.rs**

Add to `crates/vol-llm-tui/src/ui/mod.rs`:

```rust
mod input_area;
pub use input_area::render_input_area;
```

- [ ] **Step 3: Verify compilation**

```bash
cargo check -p vol-llm-tui
```

Expected: Compiles successfully (may have dead code warnings)

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-tui/src/ui/input_area.rs crates/vol-llm-tui/src/ui/mod.rs
git commit -m "feat: add input area widget with shortcut hints"
```

---

### Task 2: Wire input area into right panel layout

**Files:**
- Modify: `crates/vol-llm-tui/src/ui/mod.rs:52-74`

- [ ] **Step 1: Update render_right_panel**

Replace the existing `render_right_panel` function (lines 52-74) with:

```rust
fn render_right_panel(frame: &mut Frame, area: Rect, state: &AppState) {
    // Split into: tab bar (1) + content (flexible) + input area (5 rows)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),   // tab bar
            Constraint::Min(3),      // tab content (conversation/workspace)
            Constraint::Length(5),   // input area (4 rows textarea + 1 row hints)
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

    // Render input area
    render_input_area(frame, chunks[2], state);
}
```

- [ ] **Step 2: Verify compilation**

```bash
cargo check -p vol-llm-tui
```

Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-tui/src/ui/mod.rs
git commit -m "feat: wire input area into right panel layout"
```

---

### Task 3: Add exit feedback

**Files:**
- Modify: `crates/vol-llm-tui/src/main.rs` (run_event_loop, cleanup)

- [ ] **Step 1: Add exit message to AppState**

Add field to `AppState` in `crates/vol-llm-tui/src/app.rs`:

```rust
pub exiting: bool,
```

Initialize to `false` in `AppState::new()`.

- [ ] **Step 2: Set exiting flag before quit**

Modify the `handle_key` function in `main.rs`, change the Ctrl+Q branch:

```rust
(_, KeyCode::Char('q')) if key.modifiers == KeyModifiers::CONTROL => {
    if !state.is_running {
        state.exiting = true;
    }
    KeyAction::Exit
}
```

- [ ] **Step 3: Render exiting message in status bar**

Modify `crates/vol-llm-tui/src/ui/status_bar.rs`, prepend "QUITTING " when `state.exiting` is true:

```rust
let prefix = if state.exiting { "QUITTING · " } else { "" };
let title = format!(" {}{}", prefix, state.session_id);
```

- [ ] **Step 4: Add brief delay before exit**

Modify `run_event_loop` in `main.rs`, change the `KeyAction::Exit` match:

```rust
KeyAction::Exit => {
    if state.lock().await.exiting {
        // Brief visual feedback before exit
        drop(state);
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    return Ok(());
}
```

- [ ] **Step 5: Verify compilation**

```bash
cargo check -p vol-llm-tui
```

Expected: Compiles successfully

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-tui/src/app.rs crates/vol-llm-tui/src/main.rs crates/vol-llm-tui/src/ui/status_bar.rs
git commit -m "feat: add visual feedback on Ctrl+Q exit"
```

---

### Task 4: Full workspace verification

- [ ] **Step 1: Full workspace check**

```bash
cargo check --workspace
```

Expected: All crates compile

- [ ] **Step 2: Run all tests**

```bash
cargo test --workspace --lib
```

Expected: All tests pass (except pre-existing vol-llm-provider failure)

---

## Summary

| File | Change |
|------|--------|
| `ui/input_area.rs` | **New** — renders TextArea widget + shortcut hints |
| `ui/mod.rs` | Split right panel into tab bar + content + input area |
| `app.rs` | Add `exiting` flag |
| `main.rs` | Set exiting flag on Ctrl+Q, brief delay before exit |
| `ui/status_bar.rs` | Show "QUITTING" prefix when exiting |
