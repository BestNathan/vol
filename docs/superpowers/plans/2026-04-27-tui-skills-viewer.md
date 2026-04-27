# TUI Skills Viewer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a dedicated skills viewer tab and status bar count to the TUI so users can see which skills are loaded at startup.

**Architecture:** Create a `SkillLoader` in `main.rs` at TUI startup, discover skills, store metadata in `AppState`. Add a 4th `Skills` tab and render a scrollable table with name, version, scope, and description.

**Tech Stack:** Rust, ratatui TUI, vol-llm-skill crate

---

### Task 1: Add vol-llm-skill dependency and AppState fields

**Files:**
- Modify: `crates/vol-llm-tui/Cargo.toml`
- Modify: `crates/vol-llm-tui/src/app.rs`

- [ ] **Step 1: Add vol-llm-skill to Cargo.toml**

Add the dependency to `crates/vol-llm-tui/Cargo.toml` under `[dependencies]`:

```toml
vol-llm-skill = { path = "../vol-llm-skill" }
```

- [ ] **Step 2: Add SkillDisplayEntry struct to app.rs**

Add this struct below the `LogLine` struct (around line 115) in `crates/vol-llm-tui/src/app.rs`:

```rust
/// Display-friendly skill entry for the Skills tab.
pub struct SkillDisplayEntry {
    pub name: String,
    pub version: String,
    pub scope: String,
    pub description: String,
}
```

- [ ] **Step 3: Add ActiveTab::Skills variant**

In `crates/vol-llm-tui/src/app.rs`, find the `ActiveTab` enum and add `Skills`:

```rust
pub enum ActiveTab {
    Conversation,
    Workspace,
    Skills,
    Logs,
}
```

Update the `toggle` method to cycle through the new tab:

```rust
impl ActiveTab {
    pub fn toggle(self) -> Self {
        match self {
            ActiveTab::Conversation => ActiveTab::Workspace,
            ActiveTab::Workspace => ActiveTab::Skills,
            ActiveTab::Skills => ActiveTab::Logs,
            ActiveTab::Logs => ActiveTab::Conversation,
        }
    }
}
```

- [ ] **Step 4: Add skills field to AppState**

Add `pub skills: Vec<SkillDisplayEntry>` to the `AppState` struct (around line 263, after `log_viewer`):

```rust
pub struct AppState {
    // ... existing fields ...
    pub log_viewer: LogViewer,
    pub skills: Vec<SkillDisplayEntry>,
}
```

Update `AppState::new()` to initialize it:

```rust
impl AppState {
    pub fn new(session_id: String, working_dir: &str) -> Self {
        let workspace = scan_workspace(working_dir);
        Self {
            // ... existing fields ...
            log_viewer: LogViewer::new(),
            skills: Vec::new(),
        }
    }
}
```

- [ ] **Step 5: Compile check**

```bash
cargo check -p vol-llm-tui
```

Expected: All clean, no errors.

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-tui/Cargo.toml crates/vol-llm-tui/src/app.rs
git commit -m "feat(tui): add Skills tab enum variant and AppState skills field"
```

---

### Task 2: Discover skills at TUI startup

**Files:**
- Modify: `crates/vol-llm-tui/src/main.rs`

- [ ] **Step 1: Add imports**

At the top of `crates/vol-llm-tui/src/main.rs`, add:

```rust
use vol_llm_skill::SkillLoader;
```

- [ ] **Step 2: Add helper function to discover skills**

Add this function after the `derive_store_paths()` function (around line 122):

```rust
/// Discover skills from the working directory and return display entries.
async fn discover_skills(working_dir: &std::path::Path) -> Vec<app::SkillDisplayEntry> {
    let loader = SkillLoader::new(Some(working_dir.to_path_buf()));
    if let Err(e) = loader.discover_all().await {
        tracing::warn!(error = %e, "Failed to discover skills");
        return Vec::new();
    }
    loader.list_metadata().await
        .into_iter()
        .map(|m| app::SkillDisplayEntry {
            name: m.name,
            version: m.version,
            scope: match m.scope {
                vol_llm_skill::SkillScope::User => "User".to_string(),
                vol_llm_skill::SkillScope::Repo => "Repo".to_string(),
                vol_llm_skill::SkillScope::Custom(p) => format!("Custom:{}", p.display()),
            },
            description: m.description,
        })
        .collect()
}
```

- [ ] **Step 3: Call discover_skills in main() and populate AppState**

In `main()`, after the `working_dir` is determined (around line 85), discover skills and pass them to `AppState::new()`. Modify the `AppState` creation:

```rust
// Build pre-built agent configuration cache
let (store_dir, _) = derive_store_paths();
let cache = Arc::new(agent_cache::AgentCache::new(working_dir, store_dir));

// Discover skills at startup
let skills = discover_skills(&working_dir).await;

// Create shared state
let state = Arc::new(tokio::sync::Mutex::new(
    AppState::new(session_id, working_dir.to_string_lossy().as_ref(), skills),
));
```

- [ ] **Step 4: Update AppState::new to accept skills**

In `crates/vol-llm-tui/src/app.rs`, change the signature:

```rust
impl AppState {
    pub fn new(session_id: String, working_dir: &str, skills: Vec<SkillDisplayEntry>) -> Self {
        let workspace = scan_workspace(working_dir);
        Self {
            // ... existing fields ...
            log_viewer: LogViewer::new(),
            skills,
        }
    }
}
```

- [ ] **Step 5: Compile check**

```bash
cargo check -p vol-llm-tui
```

Expected: All clean.

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-tui/src/main.rs crates/vol-llm-tui/src/app.rs
git commit -m "feat(tui): discover skills at TUI startup"
```

---

### Task 3: Add skills count to status bar

**Files:**
- Modify: `crates/vol-llm-tui/src/ui/status_bar.rs`

- [ ] **Step 1: Add skills count segment**

In `crates/vol-llm-tui/src/ui/status_bar.rs`, modify the `text` format string. Find this line:

```rust
let text = format!(
    " {}Session: {}{} │ Run: {} │ Iter: {} │ Tools: {} │ Time: {} │ {}",
```

Change it to conditionally include the skills count:

```rust
let skills_part = if state.skills.is_empty() {
    String::new()
} else {
    format!(" │ Skills: {}", state.skills.len())
};

let text = format!(
    " {}Session: {}{} │ Run: {} │ Iter: {} │ Tools: {}{} │ Time: {} │ {}",
    unsafe_prefix,
    prefix,
    state.session_id,
    state.run_count,
    state.iteration,
    state.tool_call_count,
    skills_part,
    time_str,
    status,
);
```

- [ ] **Step 2: Compile check**

```bash
cargo check -p vol-llm-tui
```

Expected: All clean.

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-tui/src/ui/status_bar.rs
git commit -m "feat(tui): show skills count in status bar"
```

---

### Task 4: Create skills panel widget

**Files:**
- Create: `crates/vol-llm-tui/src/ui/skills_panel.rs`
- Modify: `crates/vol-llm-tui/src/ui/mod.rs`

- [ ] **Step 1: Write the skills panel module**

Create `crates/vol-llm-tui/src/ui/skills_panel.rs`:

```rust
//! Skills panel widget — scrollable table of loaded skills.

use crate::app::{AppState, SkillDisplayEntry};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::text::{Line, Span, Text};

/// Render the skills panel.
pub fn render_skills(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Skills ({}) ", state.skills.len()));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.skills.is_empty() {
        let empty = Paragraph::new("No skills discovered")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(empty, inner);
        return;
    }

    // Compute column widths
    let max_width = inner.width as usize;
    let name_width = 22.min(max_width.saturating_sub(20));
    let version_width = 8.min(max_width.saturating(name_width + 12));
    let scope_width = 10.min(max_width.saturating(name_width + version_width + 4));
    let desc_width = max_width.saturating_sub(name_width + version_width + scope_width + 4);

    let lines: Vec<Line> = state.skills
        .iter()
        .map(|entry| render_skill_row(entry, name_width, version_width, scope_width, desc_width))
        .collect();

    let text = Text::from(lines);
    let paragraph = Paragraph::new(text);
    frame.render_widget(paragraph, inner);
}

fn render_skill_row(
    entry: &SkillDisplayEntry,
    name_width: usize,
    version_width: usize,
    scope_width: usize,
    desc_width: usize,
) -> Line<'static> {
    let name = pad_or_truncate(&entry.name, name_width);
    let version = pad_or_truncate(&entry.version, version_width);
    let scope = pad_or_truncate(&entry.scope, scope_width);
    let desc = pad_or_truncate(&entry.description, desc_width);

    let scope_color = match entry.scope.as_str() {
        "User" => Color::Green,
        "Repo" => Color::Blue,
        _ => Color::Yellow,
    };

    Line::from(vec![
        Span::styled(name, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::raw(" │ "),
        Span::styled(version, Style::default().fg(Color::DarkGray)),
        Span::raw(" │ "),
        Span::styled(scope, Style::default().fg(scope_color)),
        Span::raw(" │ "),
        Span::styled(desc, Style::default().fg(Color::DarkGray)),
    ])
}

fn pad_or_truncate(s: &str, width: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= width {
        let padding = " ".repeat(width.saturating_sub(char_count));
        format!("{}{}", s, padding)
    } else {
        let truncated: String = s.chars().take(width.saturating_sub(1)).collect();
        format!("{}…", truncated)
    }
}
```

- [ ] **Step 2: Export the render function**

In `crates/vol-llm-tui/src/ui/mod.rs`, add:

```rust
mod skills_panel;
// ...
pub use skills_panel::render_skills;
```

- [ ] **Step 3: Add Skills tab to tab bar**

In `crates/vol-llm-tui/src/ui/mod.rs`, find the `render_tab_bar` function and add the Skills tab:

```rust
let skills_style = if matches!(active, ActiveTab::Skills) {
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
    Span::styled(" Skills ", skills_style),
    Span::raw(" "),
    Span::styled(" Logs ", logs_style),
    Span::raw(" "),
]);
```

- [ ] **Step 4: Add Skills tab render branch**

In `crates/vol-llm-tui/src/ui/mod.rs`, find the `render_right_panel` match block and add:

```rust
match state.active_tab {
    ActiveTab::Conversation => {
        render_conversation(frame, chunks[1], state);
    }
    ActiveTab::Workspace => {
        render_workspace(frame, chunks[1], state);
    }
    ActiveTab::Skills => {
        render_skills(frame, chunks[1], state);
    }
    ActiveTab::Logs => {
        render_log_viewer(frame, chunks[1], state);
    }
}
```

- [ ] **Step 5: Compile check**

```bash
cargo check -p vol-llm-tui
```

Expected: All clean.

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-tui/src/ui/skills_panel.rs crates/vol-llm-tui/src/ui/mod.rs
git commit -m "feat(tui): add Skills tab and scrollable skills panel"
```

---

### Task 5: Add keyboard shortcut for direct Skills tab access

**Files:**
- Modify: `crates/vol-llm-tui/src/main.rs`

- [ ] **Step 1: Add Ctrl+4 shortcut**

In `crates/vol-llm-tui/src/main.rs`, find the `handle_key` function and add after the Ctrl+3 handler:

```rust
// Ctrl+4: direct tab switch to Skills
(KeyModifiers::CONTROL, KeyCode::Char('4')) => {
    state.active_tab = app::ActiveTab::Skills;
    KeyAction::None
}
```

- [ ] **Step 2: Compile check**

```bash
cargo check -p vol-llm-tui
```

Expected: All clean.

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-tui/src/main.rs
git commit -m "feat(tui): add Ctrl+4 shortcut for Skills tab"
```

---

### Task 6: Final integration test

- [ ] **Step 1: Full workspace build**

```bash
cargo build --release -p vol-llm-tui
```

Expected: Clean build, no warnings.

- [ ] **Step 2: Run all tests**

```bash
cargo test --workspace
```

Expected: All tests pass.

- [ ] **Step 3: Final commit if needed**

```bash
git status
```

If any files are modified, commit them.
