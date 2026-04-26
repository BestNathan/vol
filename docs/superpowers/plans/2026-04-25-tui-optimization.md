# TUI Agent/Session/Logger Optimization Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add session list with resume capability, agent config caching, and a JSONL log viewer tab to vol-llm-tui.

**Architecture:** Three focused additions: (1) `list_sessions()` concrete method on `FileSessionEntryStore` to scan session files, (2) `SessionManager` + `AgentCache` structs in TUI to manage sessions and pre-built tool config, (3) `ActiveTab::Logs` with lazy-loaded `LogViewer` state for browsing JSONL event logs. No breaking changes to existing flows.

**Tech Stack:** Rust, ratatui, tokio, vol-session, vol-llm-observability

---

### Task 1: Add `list_sessions()` to `FileSessionEntryStore`

**Files:**
- Modify: `crates/vol-session/src/file_store.rs`
- Modify: `crates/vol-session/src/lib.rs`
- Test: `crates/vol-session/src/file_store.rs` (inline tests)

- [ ] **Step 1: Add `SessionSummary` struct and `list_sessions()` method**

Add this struct and method to `impl FileSessionEntryStore` in `file_store.rs`, inserting before the `#[async_trait]` impl block (around line 162):

```rust
/// Summary of a session file for listing purposes.
pub struct SessionSummary {
    pub session_id: String,
    pub created_at: i64,
    pub entry_count: usize,
}

impl FileSessionEntryStore {
    /// Scan `{entry_dir}/*.jsonl` and return session summaries.
    /// Parses the first entry from each file to get `created_at`,
    /// and counts total lines for `entry_count`.
    pub fn list_sessions(&self) -> std::io::Result<Vec<SessionSummary>> {
        let mut summaries = Vec::new();
        let dir = match std::fs::read_dir(&self.entry_dir) {
            Ok(d) => d,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(summaries),
            Err(e) => return Err(e),
        };

        for entry in dir {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            if !path.is_file() {
                continue;
            }

            let session_id = match path.file_stem().and_then(|s| s.to_str()) {
                Some(id) => id.to_string(),
                None => continue,
            };

            let file = std::fs::File::open(&path)?;
            let reader = std::io::BufReader::new(file);
            let mut count = 0;
            let mut created_at: Option<i64> = None;

            for line in reader.lines() {
                let line = line?;
                if line.trim().is_empty() {
                    continue;
                }
                if created_at.is_none() {
                    if let Some(parsed) = Self::from_json(&line) {
                        created_at = Some(parsed.created_at);
                    }
                }
                count += 1;
            }

            if let Some(ts) = created_at {
                summaries.push(SessionSummary {
                    session_id,
                    created_at: ts,
                    entry_count: count,
                });
            }
        }

        summaries.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(summaries)
    }
}
```

- [ ] **Step 2: Add tests for `list_sessions()`**

Add to the `entry_tests` module at the bottom of `file_store.rs`:

```rust
#[tokio::test]
async fn test_file_entry_store_list_sessions() {
    let temp_dir = tempdir().unwrap();
    let store = FileSessionEntryStore::new(temp_dir.path());

    let entry_a = SessionEntry::new_message("session-a".to_string(), Message::user("hello"));
    store.save(entry_a).await.unwrap();

    let entry_b = SessionEntry::new_message("session-b".to_string(), Message::user("world"));
    store.save(entry_b).await.unwrap();

    let summaries = store.list_sessions().unwrap();
    assert_eq!(summaries.len(), 2);
    assert_eq!(summaries[0].entry_count, 1);
    assert_eq!(summaries[1].entry_count, 1);
}

#[tokio::test]
async fn test_file_entry_store_list_sessions_empty_dir() {
    let temp_dir = tempdir().unwrap();
    let store = FileSessionEntryStore::new(temp_dir.path());
    let summaries = store.list_sessions().unwrap();
    assert!(summaries.is_empty());
}
```

- [ ] **Step 3: Export `SessionSummary` from `lib.rs`**

In `crates/vol-session/src/lib.rs`, change:
```rust
pub use file_store::FileSessionEntryStore;
```
to:
```rust
pub use file_store::{FileSessionEntryStore, SessionSummary};
```

- [ ] **Step 4: Build and test**

Run: `cargo test -p vol-session -- --test-threads=1`
Expected: All tests pass including the two new ones.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-session/src/file_store.rs crates/vol-session/src/lib.rs
git commit -m "feat(vol-session): add list_sessions() and SessionSummary to FileSessionEntryStore"
```

---

### Task 2: Add session list dialog and session resume to TUI

**Files:**
- Modify: `crates/vol-llm-tui/src/app.rs`
- Modify: `crates/vol-llm-tui/src/main.rs`
- Modify: `crates/vol-llm-tui/src/ui/mod.rs`
- Create: `crates/vol-llm-tui/src/ui/session_dialog.rs`
- Test: `cargo check -p vol-llm-tui`

- [ ] **Step 1: Add session dialog state types to `app.rs`**

Add after the `ActiveTab` impl block:

```rust
/// State for the session list dialog overlay.
pub struct SessionDialog {
    pub open: bool,
    pub sessions: Vec<SessionDialogEntry>,
    pub selected: usize,
}

pub struct SessionDialogEntry {
    pub session_id: String,
    pub entry_count: usize,
    pub age_label: String,
}

impl SessionDialog {
    pub fn new() -> Self {
        Self {
            open: false,
            sessions: Vec::new(),
            selected: 0,
        }
    }
}
```

Add `session_dialog` field to `AppState` struct (after `approval_state`):
```rust
    pub session_dialog: SessionDialog,
```

Add in `AppState::new()` constructor:
```rust
            session_dialog: SessionDialog::new(),
```

- [ ] **Step 2: Wrap session in `Arc<Mutex<>>` in `main.rs`**

In `main()`, change:
```rust
    let session: Arc<Session> = create_session()?;
    let session_id = session.id.clone();
```
to:
```rust
    let initial_session = create_session()?;
    let session: Arc<tokio::sync::Mutex<Arc<Session>>> =
        Arc::new(tokio::sync::Mutex::new(initial_session));
    let session_id = session.lock().unwrap().id.clone();
```

Update `run_event_loop` signature:
```rust
async fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: Arc<tokio::sync::Mutex<AppState>>,
    session: Arc<tokio::sync::Mutex<Arc<Session>>>,
) -> Result<(), Box<dyn std::error::Error>> {
```

- [ ] **Step 3: Update `spawn_agent` to accept mutex-wrapped session**

Change the `spawn_agent` function signature from `session: Arc<Session>` to:
```rust
fn spawn_agent(
    input: String,
    state: Arc<tokio::sync::Mutex<AppState>>,
    session: Arc<tokio::sync::Mutex<Arc<Session>>>,
) {
    tokio::spawn(async move {
        let session = session.lock().await.clone();
```

Add `.lock().await.clone()` at the top of the spawn body (right after `tokio::spawn(async move {`), before the `{ let mut state = state.lock().await; ... }` block. The rest of the function uses the local `session` variable.

Update the spawn call in the event loop from:
```rust
spawn_agent(input, state.clone(), session.clone());
```
to:
```rust
spawn_agent(input, state.clone(), session.clone());
```
(this call stays the same since `session` is already `Arc<Mutex<Arc<Session>>>`)

- [ ] **Step 4: Add `ResumeSession` to `KeyAction`**

Change the `KeyAction` enum:
```rust
enum KeyAction {
    Exit,
    Send(String),
    ResumeSession(String),
    None,
}
```

- [ ] **Step 5: Add `format_age` helper**

Add before the `spawn_agent` function:

```rust
fn format_age(created_at: i64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let diff = now.saturating_sub(created_at);
    if diff < 60 {
        "just now".to_string()
    } else if diff < 3600 {
        format!("{}m ago", diff / 60)
    } else if diff < 86400 {
        format!("{}h ago", diff / 3600)
    } else if diff < 172800 {
        "yesterday".to_string()
    } else {
        format!("{}d ago", diff / 86400)
    }
}
```

- [ ] **Step 6: Add session dialog key handling in `handle_key`**

Add BEFORE the `match (key.modifiers, key.code)` block (after the approval response handling):

```rust
    // Session dialog navigation — highest priority when open
    if state.session_dialog.open {
        match key.code {
            KeyCode::Esc => {
                state.session_dialog.open = false;
                return KeyAction::None;
            }
            KeyCode::Enter => {
                if let Some(entry) = state.session_dialog.sessions.get(state.session_dialog.selected) {
                    return KeyAction::ResumeSession(entry.session_id.clone());
                }
                return KeyAction::None;
            }
            KeyCode::Up => {
                if state.session_dialog.selected > 0 {
                    state.session_dialog.selected -= 1;
                }
                return KeyAction::None;
            }
            KeyCode::Down => {
                if state.session_dialog.selected + 1 < state.session_dialog.sessions.len() {
                    state.session_dialog.selected += 1;
                }
                return KeyAction::None;
            }
            KeyCode::Char('n') => {
                state.session_dialog.open = false;
                state.session_id = uuid::Uuid::new_v4().to_string();
                return KeyAction::None;
            }
            KeyCode::Char('d') => {
                if let Some(entry) = state.session_dialog.sessions.get(state.session_dialog.selected) {
                    if entry.session_id != state.session_id {
                        let id = entry.session_id.clone();
                        state.session_dialog.sessions.remove(state.session_dialog.selected);
                        if state.session_dialog.selected >= state.session_dialog.sessions.len() && state.session_dialog.selected > 0 {
                            state.session_dialog.selected -= 1;
                        }
                        let (_, sessions_dir) = derive_store_paths();
                        let store = vol_session::FileSessionEntryStore::new(&sessions_dir);
                        tokio::spawn(async move {
                            let _ = store.delete_session(&id).await;
                        });
                    }
                }
                return KeyAction::None;
            }
            _ => {}
        }
    }
```

- [ ] **Step 7: Add Ctrl+S binding**

In the main `match (key.modifiers, key.code)` block, add alongside the existing CONTROL bindings (near Ctrl+1/2):

```rust
        // Ctrl+S: toggle session list dialog
        (KeyModifiers::CONTROL, KeyCode::Char('s')) => {
            if !state.is_running {
                state.session_dialog.open = !state.session_dialog.open;
                if state.session_dialog.open {
                    let (_, sessions_dir) = derive_store_paths();
                    let store = vol_session::FileSessionEntryStore::new(&sessions_dir);
                    let summaries = store.list_sessions().unwrap_or_default();
                    state.session_dialog.sessions = summaries
                        .into_iter()
                        .map(|s| SessionDialogEntry {
                            session_id: s.session_id,
                            entry_count: s.entry_count,
                            age_label: format_age(s.created_at),
                        })
                        .collect();
                    state.session_dialog.selected = 0;
                }
            }
            KeyAction::None
        }
```

- [ ] **Step 8: Handle `ResumeSession` in event loop**

In `run_event_loop`, add after the `KeyAction::Send(input)` arm:

```rust
                            KeyAction::ResumeSession(session_id) => {
                                let session = session.clone();
                                let state = state.clone();
                                tokio::spawn(async move {
                                    let (_, sessions_dir) = derive_store_paths();
                                    let store = Arc::new(vol_session::FileSessionEntryStore::new(&sessions_dir));
                                    match vol_session::Session::resume(session_id.clone(), store).await {
                                        Ok(resumed) => {
                                            let mut state = state.lock().await;
                                            state.session_id = resumed.id.clone();
                                            state.session_dialog.open = false;
                                            state.conversation.clear();
                                            if let Ok(msgs) = resumed.get_messages().await {
                                                for msg in msgs {
                                                    match msg.message.role {
                                                        vol_llm_core::MessageRole::User => {
                                                            if let Some(content) = &msg.message.content {
                                                                state.conversation.push(ConversationEntry::UserInput { text: content.clone() });
                                                            }
                                                        }
                                                        vol_llm_core::MessageRole::Assistant => {
                                                            if let Some(content) = &msg.message.content {
                                                                state.conversation.push(ConversationEntry::AgentAnswer { text: content.clone() });
                                                            }
                                                        }
                                                        vol_llm_core::MessageRole::System => {}
                                                    }
                                                }
                                            }
                                            let mut sess = session.lock().await;
                                            *sess = resumed;
                                        }
                                        Err(e) => {
                                            let mut state = state.lock().await;
                                            state.session_dialog.open = false;
                                            state.last_error = Some(format!("Failed to resume session: {}", e));
                                        }
                                    }
                                });
                            }
```

- [ ] **Step 9: Create session dialog render module**

Create `crates/vol-llm-tui/src/ui/session_dialog.rs`:

```rust
//! Session list dialog overlay rendering.

use crate::app::AppState;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

pub fn render_session_dialog(frame: &mut Frame, area: Rect, state: &AppState) {
    if !state.session_dialog.open {
        return;
    }

    let width = 60.min(area.width);
    let height = (state.session_dialog.sessions.len() as u16 + 6).min(area.height - 2);
    let x = (area.width - width) / 2;
    let y = (area.height - height) / 2;

    let rect = Rect::new(x, y, width, height);

    frame.render_widget(ratatui::widgets::Clear, rect);

    let lines = build_dialog_lines(state);
    let paragraph = Paragraph::new(lines)
        .block(Block::default()
            .title(" Sessions (Ctrl+S to dismiss) ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow)));

    frame.render_widget(paragraph, rect);
}

fn build_dialog_lines(state: &AppState) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    if state.session_dialog.sessions.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No saved sessions found.",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for (i, entry) in state.session_dialog.sessions.iter().enumerate() {
            let is_selected = i == state.session_dialog.selected;
            let prefix = if is_selected { "> " } else { "  " };
            let short_id = if entry.session_id.len() > 8 {
                &entry.session_id[..8]
            } else {
                &entry.session_id
            };
            let style = if is_selected {
                Style::default()
                    .fg(Color::White)
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            lines.push(Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(format!("{:<10}", short_id), style),
                Span::styled(
                    format!(" {:>4} entries", entry.entry_count),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("    {}", entry.age_label),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " [n] New  [Enter] Resume  [d] Delete  [Esc] Cancel",
        Style::default().fg(Color::DarkGray),
    )));

    lines
}
```

- [ ] **Step 10: Wire session dialog into UI**

In `ui/mod.rs`, add:
```rust
mod session_dialog;
pub use session_dialog::render_session_dialog;
```

In `render_ui`, add after the existing render calls:
```rust
    // Render session dialog overlay if open
    render_session_dialog(frame, area, state);
```

- [ ] **Step 11: Add `uuid` dependency**

Check `crates/vol-llm-tui/Cargo.toml`. If `uuid` is not present, add:
```toml
uuid = { version = "1", features = ["v4"] }
```

- [ ] **Step 12: Build and verify**

Run: `cargo check -p vol-llm-tui`
Expected: No errors.

- [ ] **Step 13: Commit**

```bash
git add crates/vol-llm-tui/src/app.rs crates/vol-llm-tui/src/main.rs crates/vol-llm-tui/src/ui/mod.rs crates/vol-llm-tui/src/ui/session_dialog.rs crates/vol-llm-tui/Cargo.toml
git commit -m "feat(vol-llm-tui): add session list dialog with resume, delete, and navigation"
```

---

### Task 3: Add `AgentCache` and integrate into spawn_agent

**Files:**
- Create: `crates/vol-llm-tui/src/agent_cache.rs`
- Modify: `crates/vol-llm-tui/src/main.rs`
- Test: `cargo check -p vol-llm-tui`

- [ ] **Step 1: Create `AgentCache` struct**

Create `crates/vol-llm-tui/src/agent_cache.rs`:

```rust
//! Pre-built agent configuration cache to avoid per-run reconstruction.

use std::path::PathBuf;
use vol_llm_tool::ToolConfig;

pub struct AgentCache {
    pub working_dir: PathBuf,
    pub store_dir: PathBuf,
    pub tool_config: ToolConfig,
}

impl AgentCache {
    pub fn new(working_dir: PathBuf, store_dir: PathBuf) -> Self {
        let mut tool_config = ToolConfig::new();

        if let Ok(tavily_key) = std::env::var("TAVILY_API_KEY") {
            tool_config.set("web_search", vol_llm_tools_builtin::WebSearchConfig {
                provider: "tavily".to_string(),
                api_key: tavily_key,
                proxy: vol_llm_tool::ProxyConfig::default(),
            });
        }

        if let Ok(max_len) = std::env::var("WEB_FETCH_MAX_LENGTH") {
            tool_config.set("web_fetch", vol_llm_tools_builtin::WebFetchConfig {
                max_content_length: max_len.parse().ok(),
                proxy: vol_llm_tool::ProxyConfig::default(),
            });
        }

        Self {
            working_dir,
            store_dir,
            tool_config,
        }
    }
}
```

- [ ] **Step 2: Wire `AgentCache` into `main.rs`**

Add module declaration:
```rust
mod agent_cache;
```

In `main()`, after deriving `working_dir` and before `run_event_loop`:
```rust
    let (store_dir, _) = derive_store_paths();
    let cache = Arc::new(agent_cache::AgentCache::new(working_dir.clone(), store_dir));
```

Update `run_event_loop` to accept cache:
```rust
async fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: Arc<tokio::sync::Mutex<AppState>>,
    session: Arc<tokio::sync::Mutex<Arc<Session>>>,
    cache: Arc<agent_cache::AgentCache>,
) -> Result<(), Box<dyn std::error::Error>> {
```

Update call in `main()`:
```rust
    let result = run_event_loop(&mut terminal, state, session, cache).await;
```

- [ ] **Step 3: Rewrite `spawn_agent` to use cache**

Replace the entire `spawn_agent` function body with:

```rust
fn spawn_agent(
    input: String,
    state: Arc<tokio::sync::Mutex<AppState>>,
    session: Arc<tokio::sync::Mutex<Arc<Session>>>,
    cache: Arc<agent_cache::AgentCache>,
) {
    tokio::spawn(async move {
        {
            let mut state = state.lock().await;
            state.is_running = true;
            state.approval_state.clear().await;
        }

        let session = session.lock().await.clone();

        let unsafe_mode = {
            let state_guard = state.lock().await;
            state_guard.unsafe_mode
        };

        let approval_state = {
            let state_guard = state.lock().await;
            state_guard.approval_state.unsafe_mode.store(unsafe_mode, std::sync::atomic::Ordering::Relaxed);
            state_guard.approval_state.clone()
        };

        let agent = match vol_llm_agents::coding::CodingAgentBuilder::new()
            .working_dir(cache.working_dir.clone())
            .store_dir(cache.store_dir.clone())
            .max_iterations(10)
            .session(session)
            .hitl_enabled(true)
            .unsafe_mode(unsafe_mode)
            .approval_handler(approval_state.into_handler())
            .tool_config(cache.tool_config.clone())
            .with_logger()
            .build()
            .await
        {
            Ok(a) => a,
            Err(e) => {
                let mut state = state.lock().await;
                state.conversation.push(app::ConversationEntry::Error {
                    message: format!("Error creating agent: {}", e),
                });
                state.is_running = false;
                return;
            }
        };

        let observer = Arc::new(RatatuiObserver::new(state.clone()));
        let agent = agent.with_observer(observer);

        match agent.run(&input).await {
            Ok(_response) => {}
            Err(e) => {
                let mut state = state.lock().await;
                state.conversation.push(app::ConversationEntry::Error {
                    message: format!("Error: {}", e),
                });
                state.is_running = false;
            }
        }
    });
}
```

Remove the old tool config construction block (`let mut tool_config = ToolConfig::new(); ...`) and the `derive_store_paths()` / `working_dir` / `store_dir` local variable computation from inside `spawn_agent`.

- [ ] **Step 4: Update spawn call in event loop**

Change the spawn call from:
```rust
spawn_agent(input, state.clone(), session.clone());
```
to:
```rust
spawn_agent(input, state.clone(), session.clone(), cache.clone());
```

- [ ] **Step 5: Build and verify**

Run: `cargo check -p vol-llm-tui`
Expected: No errors.

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-tui/src/agent_cache.rs crates/vol-llm-tui/src/main.rs
git commit -m "feat(vol-llm-tui): add AgentCache for pre-built tool config, simplify spawn_agent"
```

---

### Task 4: Add `ActiveTab::Logs` and LogViewer rendering

**Files:**
- Modify: `crates/vol-llm-tui/src/app.rs`
- Modify: `crates/vol-llm-tui/src/main.rs`
- Modify: `crates/vol-llm-tui/src/ui/mod.rs`
- Create: `crates/vol-llm-tui/src/ui/log_viewer.rs`
- Modify: `crates/vol-llm-tui/Cargo.toml`
- Test: `cargo check -p vol-llm-tui`

- [ ] **Step 1: Add Logs tab and log viewer state to `app.rs`**

Update `ActiveTab` enum:
```rust
pub enum ActiveTab {
    Conversation,
    Workspace,
    Logs,
}
```

Update `toggle()`:
```rust
    pub fn toggle(self) -> Self {
        match self {
            ActiveTab::Conversation => ActiveTab::Workspace,
            ActiveTab::Workspace => ActiveTab::Logs,
            ActiveTab::Logs => ActiveTab::Conversation,
        }
    }
```

Add types before the `AppState` struct:

```rust
pub struct LogViewer {
    pub run_logs: Vec<LogRunSummary>,
    pub selected_run: Option<String>,
    pub entries: Vec<LogLine>,
    pub scroll: u16,
    pub auto_scroll: bool,
    pub loaded: bool,
}

pub struct LogRunSummary {
    pub run_id: String,
    pub event_count: usize,
    pub last_event: String,
    pub last_event_time: String,
}

pub struct LogLine {
    pub event_type: String,
    pub summary: String,
    pub timestamp: String,
}

impl LogViewer {
    pub fn new() -> Self {
        Self {
            run_logs: Vec::new(),
            selected_run: None,
            entries: Vec::new(),
            scroll: 0,
            auto_scroll: true,
            loaded: false,
        }
    }

    /// Scan the logs directory and populate run_logs.
    pub fn scan_logs(&mut self) {
        let working_dir = std::env::current_dir().unwrap_or_default();
        let project_name = working_dir
            .file_name()
            .unwrap_or(std::ffi::OsStr::new("default"))
            .to_string_lossy();
        let home = std::env::var("HOME").unwrap_or_default();
        let base = std::path::PathBuf::from(home)
            .join(".vol-coding")
            .join(project_name.as_ref());
        let logs_dir = base.join("logs");

        if !logs_dir.exists() {
            return;
        }

        let Ok(entries) = std::fs::read_dir(&logs_dir) else { return };

        for entry in entries {
            let Ok(entry) = entry else { continue };
            let path = entry.path();
            if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            if path.parent() != Some(&logs_dir) {
                continue;
            }

            let run_id = match path.file_stem().and_then(|s| s.to_str()) {
                Some(id) => id.to_string(),
                None => continue,
            };

            let content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let lines: Vec<&str> = content.lines().collect();
            let event_count = lines.len();

            let (last_event, last_event_time) = lines
                .last()
                .and_then(|line| serde_json::from_str::<vol_llm_observability::LogEntry>(line).ok())
                .map(|e| (e.event.clone(), e.timestamp.format("%H:%M:%S").to_string()))
                .unwrap_or_else(|| ("unknown".to_string(), "—".to_string()));

            self.run_logs.push(LogRunSummary {
                run_id,
                event_count,
                last_event,
                last_event_time,
            });
        }

        self.run_logs.sort_by(|a, b| b.event_count.cmp(&a.event_count));
    }

    /// Load log entries for a specific run.
    pub fn load_run(&mut self, run_id: &str) {
        let working_dir = std::env::current_dir().unwrap_or_default();
        let project_name = working_dir
            .file_name()
            .unwrap_or(std::ffi::OsStr::new("default"))
            .to_string_lossy();
        let home = std::env::var("HOME").unwrap_or_default();
        let base = std::path::PathBuf::from(home)
            .join(".vol-coding")
            .join(project_name.as_ref());
        let log_path = base.join("logs").join(format!("{}.jsonl", run_id));

        let content = match std::fs::read_to_string(&log_path) {
            Ok(c) => c,
            Err(_) => return,
        };

        self.entries = content
            .lines()
            .filter_map(|line| serde_json::from_str::<vol_llm_observability::LogEntry>(line).ok())
            .map(|entry| LogLine {
                event_type: entry.event.clone(),
                summary: entry.format_event_summary(),
                timestamp: entry.timestamp.format("%H:%M:%S").to_string(),
            })
            .collect();
    }
}
```

Add `log_viewer` field to `AppState` (after `session_dialog`):
```rust
    pub log_viewer: LogViewer,
```

Add in `AppState::new()`:
```rust
            log_viewer: LogViewer::new(),
```

- [ ] **Step 2: Add log viewer key handling**

In `handle_key`, add BEFORE the session dialog handling block:

```rust
    // Log viewer navigation (Logs tab active, no dialog, not running)
    if matches!(state.active_tab, app::ActiveTab::Logs) && !state.session_dialog.open && !state.is_running {
        match key.code {
            KeyCode::Enter => {
                if state.log_viewer.selected_run.is_none() && !state.log_viewer.run_logs.is_empty() {
                    let run_id = state.log_viewer.run_logs[0].run_id.clone();
                    state.log_viewer.load_run(&run_id);
                    state.log_viewer.selected_run = Some(run_id);
                    state.log_viewer.scroll = 0;
                    state.log_viewer.auto_scroll = true;
                }
                return KeyAction::None;
            }
            KeyCode::Esc => {
                if state.log_viewer.selected_run.is_some() {
                    state.log_viewer.selected_run = None;
                    state.log_viewer.entries.clear();
                }
                return KeyAction::None;
            }
            KeyCode::Up => {
                if state.log_viewer.selected_run.is_some() {
                    state.log_viewer.scroll = state.log_viewer.scroll.saturating_sub(1);
                    state.log_viewer.auto_scroll = false;
                }
                return KeyAction::None;
            }
            KeyCode::Down => {
                if state.log_viewer.selected_run.is_some() {
                    state.log_viewer.scroll = state.log_viewer.scroll.saturating_add(1);
                    state.log_viewer.auto_scroll = false;
                }
                return KeyAction::None;
            }
            _ => {}
        }
    }
```

- [ ] **Step 3: Add Ctrl+3 binding**

In the main `match (key.modifiers, key.code)` block, add:

```rust
        (KeyModifiers::CONTROL, KeyCode::Char('3')) => {
            state.active_tab = app::ActiveTab::Logs;
            KeyAction::None
        }
```

- [ ] **Step 4: Add lazy log loading in render tick**

In `run_event_loop`, update the render tick:

```rust
            _ = render_interval.tick() => {
                let mut state = state.lock().await;
                if matches!(state.active_tab, ActiveTab::Logs) && !state.log_viewer.loaded {
                    state.log_viewer.scan_logs();
                    state.log_viewer.loaded = true;
                }
                terminal.draw(|f| ui::render_ui(f, &state))?;
            }
```

- [ ] **Step 5: Create log viewer render module**

Create `crates/vol-llm-tui/src/ui/log_viewer.rs`:

```rust
//! Log viewer tab rendering.

use crate::app::{AppState, LogLine};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

pub fn render_log_viewer(frame: &mut Frame, area: Rect, state: &AppState) {
    if state.log_viewer.selected_run.is_some() {
        render_log_entries(frame, area, state);
    } else {
        render_run_list(frame, area, state);
    }
}

fn render_run_list(frame: &mut Frame, area: Rect, state: &AppState) {
    let mut lines = Vec::new();

    if state.log_viewer.run_logs.is_empty() {
        lines.push(Line::from(Span::styled(
            " No log files found.",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for run in &state.log_viewer.run_logs {
            let is_selected = state.log_viewer.selected_run.as_deref() == Some(&run.run_id);
            let style = if is_selected {
                Style::default().fg(Color::White).bg(Color::DarkGray)
            } else {
                Style::default().fg(Color::Gray)
            };
            let short_id = if run.run_id.len() > 12 {
                &run.run_id[..12]
            } else {
                &run.run_id
            };
            lines.push(Line::from(vec![
                Span::styled(format!(" {:<14}", short_id), style),
                Span::styled(
                    format!(" {:>5} events", run.event_count),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("  {}", run.last_event),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " Enter to view  Esc to go back",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines)
        .block(Block::default()
            .title(" Log Runs ")
            .borders(Borders::ALL));

    frame.render_widget(paragraph, area);
}

fn render_log_entries(frame: &mut Frame, area: Rect, state: &AppState) {
    let lines = build_log_lines(&state.log_viewer.entries);
    let total_lines = lines.len();
    let scroll = compute_scroll(
        state.log_viewer.scroll,
        state.log_viewer.auto_scroll,
        total_lines,
        area.height,
    );

    let paragraph = Paragraph::new(lines)
        .block(Block::default()
            .title(format!(" Log: {} ", state.log_viewer.selected_run.as_deref().unwrap_or("")))
            .borders(Borders::ALL))
        .scroll((scroll, 0));

    frame.render_widget(paragraph, area);
}

fn build_log_lines(entries: &[LogLine]) -> Vec<Line<'static>> {
    entries
        .iter()
        .map(|entry| {
            let color = event_color(&entry.event_type);
            Line::from(vec![
                Span::styled(
                    format!("[{}] ", entry.timestamp),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(&entry.event_type, Style::default().fg(color)),
                Span::styled(
                    format!(" — {}", entry.summary),
                    Style::default().fg(color),
                ),
            ])
        })
        .collect()
}

fn event_color(event_type: &str) -> Color {
    match event_type {
        "AgentStart" | "AgentComplete" => Color::Green,
        "ToolCallBegin" | "ToolCallComplete" => Color::Yellow,
        "ToolCallError" | "AgentAborted" => Color::Red,
        _ => Color::White,
    }
}

fn compute_scroll(scroll: u16, auto_scroll: bool, total_lines: usize, view_height: u16) -> u16 {
    if auto_scroll && total_lines > view_height as usize {
        (total_lines - view_height as usize) as u16
    } else {
        scroll.min(total_lines.saturating_sub(view_height as usize) as u16)
    }
}
```

- [ ] **Step 6: Wire log viewer into UI**

In `ui/mod.rs`, add:
```rust
mod log_viewer;
pub use log_viewer::render_log_viewer;
```

Update `render_right_panel` match:
```rust
    match state.active_tab {
        ActiveTab::Conversation => {
            render_conversation(frame, chunks[1], state);
        }
        ActiveTab::Workspace => {
            render_workspace(frame, chunks[1], state);
        }
        ActiveTab::Logs => {
            render_log_viewer(frame, chunks[1], state);
        }
    }
```

Update `render_tab_bar` to include Logs:

```rust
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

    let logs_style = if matches!(active, ActiveTab::Logs) {
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
        Span::styled(" Logs ", logs_style),
        Span::raw(" "),
    ]);

    let paragraph = Paragraph::new(tabs)
        .block(Block::default().borders(Borders::BOTTOM));

    frame.render_widget(paragraph, area);
}
```

- [ ] **Step 7: Add vol-llm-observability dependency**

In `crates/vol-llm-tui/Cargo.toml`, add:
```toml
vol-llm-observability = { path = "../vol-llm-observability" }
```

- [ ] **Step 8: Build and verify**

Run: `cargo check -p vol-llm-tui`
Expected: No errors.

- [ ] **Step 9: Commit**

```bash
git add crates/vol-llm-tui/src/app.rs crates/vol-llm-tui/src/main.rs crates/vol-llm-tui/src/ui/mod.rs crates/vol-llm-tui/src/ui/log_viewer.rs crates/vol-llm-tui/Cargo.toml
git commit -m "feat(vol-llm-tui): add Logs tab with JSONL log viewer and color-coded rendering"
```

---

### Task 5: Final verification

**Files:** None (verification only)

- [ ] **Step 1: Full workspace build**

Run: `cargo check --workspace`
Expected: No errors.

- [ ] **Step 2: Full test suite**

Run: `cargo test -p vol-session -p vol-llm-tui -p vol-llm-agents -p vol-llm-observability -- --test-threads=1`
Expected: All tests pass.

No commit needed — changes were committed in prior tasks.
