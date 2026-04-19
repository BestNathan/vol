# Conversation Rendering Optimization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix four UI rendering issues in the conversation panel: content overflow, delayed thinking/content display, duplicate agent responses, and scroll control.

**Architecture:** Replace batch-on-complete event handling with in-place entry mutation for real-time streaming. Add Paragraph wrapping for text. Switch scroll from PageUp/PageDown-only to Up/Down arrows.

**Tech Stack:** ratatui 0.30, crossterm 0.29, vol-llm-core AgentStreamEvent

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `crates/vol-llm-tui/src/app.rs` | Modify | Add `Thinking`, `ContentStreaming` variants, remove `ThinkingComplete` |
| `crates/vol-llm-tui/src/render.rs` | Modify | Real-time delta handling, remove duplicate content in AgentComplete |
| `crates/vol-llm-tui/src/ui/conversation.rs` | Modify | Render with `Paragraph::wrap()`, handle new variants |
| `crates/vol-llm-tui/src/main.rs` | Modify | Add Up/Down arrow scroll handling |

---

### Task 1: Add streaming entry types and real-time event handling

**Files:**
- Modify: `crates/vol-llm-tui/src/app.rs:25-35` (ConversationEntry enum)
- Modify: `crates/vol-llm-tui/src/render.rs:1-295` (EventBuffer apply logic)

- [ ] **Step 1: Update ConversationEntry enum in app.rs**

Replace the `ThinkingComplete` variant with `Thinking`, and add `ContentStreaming`:

```rust
/// A single rendered entry in the conversation view.
#[derive(Debug, Clone)]
pub enum ConversationEntry {
    UserInput { text: String },
    Thinking { content: String },
    ContentStreaming { content: String },
    ToolCall { tool_name: String, arg_preview: String },
    ToolResult { tool_name: String, preview: String, success: bool },
    AgentAnswer { text: String },
    RunSummary { iterations: u32, tool_calls: u32, elapsed_ms: u128 },
    Error { message: String },
}
```

- [ ] **Step 2: Update render.rs — Thinking real-time streaming**

Replace the Thinking handling in `EventBuffer::apply()` (lines 96-113):

```rust
// Thinking — push empty entry on start, mutate last entry on delta
AgentStreamEvent::ThinkingStart { .. } => {
    self.thinking_active = true;
    self.thinking_buffer.clear();
    state.conversation.push(ConversationEntry::Thinking {
        content: String::new(),
    });
}

AgentStreamEvent::ThinkingDelta { delta, .. } => {
    // Append to last Thinking entry in-place
    if let Some(ConversationEntry::Thinking { content }) = state.conversation.last_mut() {
        content.push_str(delta);
    }
}

AgentStreamEvent::ThinkingComplete { .. } => {
    self.thinking_active = false;
    // Content already streamed, no-op
}
```

- [ ] **Step 3: Update render.rs — Content real-time streaming**

Replace the Content handling (lines 115-130):

```rust
// Content — push empty streaming entry on start, mutate on delta
AgentStreamEvent::ContentStart { .. } => {
    self.content_buffer.clear();
    state.conversation.push(ConversationEntry::ContentStreaming {
        content: String::new(),
    });
}

AgentStreamEvent::ContentDelta { delta, .. } => {
    // Append to last ContentStreaming entry in-place
    if let Some(ConversationEntry::ContentStreaming { content }) = state.conversation.last_mut() {
        content.push_str(delta);
    }
}

AgentStreamEvent::ContentComplete { content, .. } => {
    // Mutate last ContentStreaming to AgentAnswer (single source)
    if let Some(ConversationEntry::ContentStreaming { .. }) = state.conversation.last() {
        let entry = state.conversation.last_mut().unwrap();
        *entry = ConversationEntry::AgentAnswer {
            text: content.clone(),
        };
    } else if !content.is_empty() {
        // Fallback: no streaming entry was pushed
        state.conversation.push(ConversationEntry::AgentAnswer {
            text: content.clone(),
        });
    }
}
```

- [ ] **Step 4: Remove duplicate content from AgentComplete**

In `AgentStreamEvent::AgentComplete` (lines 35-59), remove the content extraction block (lines 50-58). The method should only flush thinking, push RunSummary, and set `is_running = false`:

```rust
AgentStreamEvent::AgentComplete { response: _, .. } => {
    // Flush any pending thinking/content
    self.flush_thinking(state);
    self.flush_content();

    let elapsed = state.run_start
        .map(|s| s.elapsed())
        .unwrap_or_default();
    state.conversation.push(ConversationEntry::RunSummary {
        iterations: state.iteration,
        tool_calls: state.tool_call_count,
        elapsed_ms: elapsed.as_millis(),
    });
    state.is_running = false;
}
```

- [ ] **Step 5: Update flush_thinking to use Thinking variant**

Update `flush_thinking()` (lines 216-223):

```rust
fn flush_thinking(&mut self, state: &mut AppState) {
    if self.thinking_active && !self.thinking_buffer.is_empty() {
        state.conversation.push(ConversationEntry::Thinking {
            content: std::mem::take(&mut self.thinking_buffer),
        });
        self.thinking_active = false;
    }
}
```

- [ ] **Step 6: Verify compilation**

```bash
cargo check -p vol-llm-tui 2>&1 | head -40
```

Expected: Compiles successfully (may have warnings about unused `response` parameter in AgentComplete)

- [ ] **Step 7: Commit**

```bash
git add crates/vol-llm-tui/src/app.rs crates/vol-llm-tui/src/render.rs
git commit -m "feat: real-time thinking/content streaming with in-place entry updates"
```

---

### Task 2: Text wrapping and new entry rendering in conversation.rs

**Files:**
- Modify: `crates/vol-llm-tui/src/ui/conversation.rs:1-129`

**Context:** The current `build_conversation_lines()` returns `Vec<Line>` which is fed into `Paragraph::new()`. Wrapping happens at the `Paragraph` level — each `Line` that exceeds panel width gets wrapped. The current code splits text into `Line` entries per input line, which is already correct for wrapping. The fix is simply adding `.wrap(Wrap { trim: false })` to the `Paragraph`.

- [ ] **Step 1: Update imports**

Add `Wrap` to the existing imports (line 7):

```rust
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
```

- [ ] **Step 2: Add wrap to Paragraph in render_conversation**

In `render_conversation()` (line 36), change:

```rust
let paragraph = Paragraph::new(text).wrap(Wrap { trim: false });
```

- [ ] **Step 3: Replace ThinkingComplete with Thinking in the match**

Replace the `ThinkingComplete` match arm (lines 55-66) with `Thinking`:

```rust
ConversationEntry::Thinking { content } => {
    lines.push(Line::from(vec![
        Span::styled("Thinking", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
    ]));
    for line in content.lines() {
        lines.push(Line::from(vec![
            Span::styled(format!("  {}", line), Style::default().fg(Color::DarkGray)),
        ]));
    }
    lines.push(Line::raw(""));
}
```

- [ ] **Step 4: Add ContentStreaming handling**

Add a new match arm for `ContentStreaming` (place after `Thinking`):

```rust
ConversationEntry::ContentStreaming { content } => {
    if content.is_empty() {
        // Show a subtle indicator during streaming start
        lines.push(Line::from(vec![
            Span::styled("Generating...", Style::default().fg(Color::DarkGray)),
        ]));
    } else {
        for line in content.lines() {
            lines.push(Line::from(vec![
                Span::styled(line, Style::default().fg(Color::White)),
            ]));
        }
    }
}
```

- [ ] **Step 5: Verify compilation**

```bash
cargo check -p vol-llm-tui 2>&1 | head -40
```

Expected: Compiles successfully

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-tui/src/ui/conversation.rs
git commit -m "feat: add text wrapping and ContentStreaming entry rendering"
```

---

### Task 3: Up/Down arrow scroll handling

**Files:**
- Modify: `crates/vol-llm-tui/src/main.rs` (key handling section)

- [ ] **Step 1: Add Up/Down arrow scroll handling**

Find the key handling section for `PageUp`/`PageDown` and add `Up`/`Down` handling:

```rust
KeyCode::PageUp => {
    state.conversation_scroll = state.conversation_scroll.saturating_sub(10);
    state.conversation_auto_scroll = false;
}
KeyCode::PageDown => {
    state.conversation_scroll += 10;
    state.conversation_auto_scroll = false;
}
KeyCode::Up => {
    state.conversation_scroll = state.conversation_scroll.saturating_sub(1);
    state.conversation_auto_scroll = false;
}
KeyCode::Down => {
    state.conversation_scroll += 1;
    state.conversation_auto_scroll = false;
}
```

- [ ] **Step 2: Verify compilation**

```bash
cargo check -p vol-llm-tui 2>&1 | head -40
```

Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-tui/src/main.rs
git commit -m "feat: add Up/Down arrow scroll for conversation panel"
```

---

### Task 4: Full verification

- [ ] **Step 1: Workspace check**

```bash
cargo check --workspace 2>&1 | tail -20
```

- [ ] **Step 2: Build and smoke test**

```bash
cargo build -p vol-llm-tui 2>&1 | tail -10
```

Expected: Builds successfully

- [ ] **Step 3: Run tests**

```bash
cargo test -p vol-llm-tui 2>&1
```

Expected: No test failures (if tests exist)
