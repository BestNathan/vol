//! vol-llm-tui: Interactive ratatui TUI for the coding agent.
//!
//! Provides a full terminal UI with status bar, tool call panel,
//! tabbed conversation/workspace views, multi-line input, and persistent layout.

mod agent_cache;
mod app;
mod approval;
mod render;
mod ui;

use std::io::{self, stdout};
use std::sync::Arc;
use std::time::Duration;

use vol_session::Session;

use app::AppState;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    event::{Event, EventStream, KeyCode, KeyEvent, KeyModifiers},
};
use futures::StreamExt;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use ratatui_textarea::TextArea;
use render::EventBuffer;
use vol_llm_agents::coding::{EventObserver, ObserverError};
use vol_llm_core::AgentStreamEvent;
use vol_session::FileSessionEntryStore;
use vol_session::SessionEntryStore;

/// Observer that forwards events to EventBuffer for AppState mutation.
struct RatatuiObserver {
    buffer: tokio::sync::Mutex<EventBuffer>,
    state: Arc<tokio::sync::Mutex<AppState>>,
}

#[async_trait::async_trait]
impl EventObserver for RatatuiObserver {
    async fn on_event(&self, event: &AgentStreamEvent) -> Result<(), ObserverError> {
        let mut buf = self.buffer.lock().await;
        let mut state = self.state.lock().await;
        buf.apply(event, &mut state);
        Ok(())
    }

    async fn on_complete(&self) -> Result<(), ObserverError> {
        Ok(())
    }
}

impl RatatuiObserver {
    fn new(state: Arc<tokio::sync::Mutex<AppState>>) -> Self {
        Self {
            buffer: tokio::sync::Mutex::new(EventBuffer::new()),
            state,
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Verify API key
    let _api_key = std::env::var("ANTHROPIC_AUTH_TOKEN")
        .expect("ANTHROPIC_AUTH_TOKEN must be set");

    // Create persistent session
    let initial_session = create_session()?;
    let session: Arc<tokio::sync::Mutex<Arc<Session>>> =
        Arc::new(tokio::sync::Mutex::new(initial_session));
    let session_id = session.lock().await.id.clone();

    // Setup terminal with panic recovery
    setup_terminal()?;
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic| {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen);
        original_hook(panic);
    }));

    // Create shared state
    let working_dir = std::env::current_dir().unwrap_or_default();
    let state = Arc::new(tokio::sync::Mutex::new(
        AppState::new(session_id, working_dir.to_string_lossy().as_ref()),
    ));

    // Build pre-built agent configuration cache
    let (store_dir, _) = derive_store_paths();
    let cache = Arc::new(agent_cache::AgentCache::new(working_dir, store_dir));

    // Create ratatui terminal
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    // Main event loop
    let result = run_event_loop(&mut terminal, state, session, cache).await;

    // Cleanup
    cleanup_terminal()?;

    result
}

/// Derive store paths from the current working directory.
/// Returns `(store_dir, sessions_dir)`.
fn derive_store_paths() -> (std::path::PathBuf, std::path::PathBuf) {
    let working_dir = std::env::current_dir().unwrap_or_default();
    let project_name = working_dir
        .file_name()
        .unwrap_or(std::ffi::OsStr::new("default"))
        .to_string_lossy();
    let home = std::env::var("HOME").unwrap_or_default();
    let base = std::path::PathBuf::from(home)
        .join(".vol-coding")
        .join(project_name.as_ref());
    let sessions = base.join("sessions");
    (base, sessions)
}

fn create_session() -> Result<Arc<Session>, Box<dyn std::error::Error>> {
    let (_base_dir, session_dir) = derive_store_paths();

    if let Err(e) = std::fs::create_dir_all(&session_dir) {
        eprintln!("Warning: cannot create session dir: {}", e);
        eprintln!("Using in-memory session (no history persistence)");
        let entry_store = Arc::new(vol_session::InMemoryEntryStore::new());
        return Ok(Arc::new(Session::new(entry_store)));
    }

    let entry_store = Arc::new(FileSessionEntryStore::new(&session_dir));
    Ok(Arc::new(Session::new(entry_store)))
}

fn setup_terminal() -> io::Result<()> {
    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;
    Ok(())
}

fn cleanup_terminal() -> io::Result<()> {
    execute!(stdout(), LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}

async fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: Arc<tokio::sync::Mutex<AppState>>,
    session: Arc<tokio::sync::Mutex<Arc<Session>>>,
    cache: Arc<agent_cache::AgentCache>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut render_interval = tokio::time::interval(Duration::from_millis(33)); // ~30fps
    let mut events = EventStream::new();

    loop {
        tokio::select! {
            // Input checked first — handles key presses immediately
            biased;

            maybe_event = events.next() => {
                match maybe_event {
                    Some(Ok(Event::Key(key))) => {
                        let key_action = {
                            let mut state = state.lock().await;
                            handle_key(key, &mut state)
                        };
                        match key_action {
                            KeyAction::Exit => {
                                let is_exiting = {
                                    let state_guard = state.lock().await;
                                    state_guard.exiting
                                };
                                if is_exiting {
                                    tokio::time::sleep(Duration::from_millis(100)).await;
                                }
                                return Ok(());
                            }
                            KeyAction::Send(input) => {
                                spawn_agent(input, state.clone(), session.clone(), cache.clone());
                            }
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
                                            state.last_error = None;
                                            state.is_running = false;
                                            state.session_dialog.open = false;
                                            state.conversation.clear();
                                            if let Ok(msgs) = resumed.get_messages().await {
                                                for msg in msgs {
                                                    match msg.message.role {
                                                        vol_llm_core::MessageRole::User => {
                                                            if let Some(content) = &msg.message.content {
                                                                state.conversation.push(app::ConversationEntry::UserInput { text: content.as_str().to_string() });
                                                            }
                                                        }
                                                        vol_llm_core::MessageRole::Assistant => {
                                                            if let Some(content) = &msg.message.content {
                                                                state.conversation.push(app::ConversationEntry::AgentAnswer { text: content.as_str().to_string() });
                                                            }
                                                        }
                                                        vol_llm_core::MessageRole::System => {}
                                                        vol_llm_core::MessageRole::Tool => {}
                                                    }
                                                }
                                            }
                                            let mut sess = session.lock().await;
                                            *sess = Arc::new(resumed);
                                        }
                                        Err(e) => {
                                            let mut state = state.lock().await;
                                            state.session_dialog.open = false;
                                            state.last_error = Some(format!("Failed to resume session: {}", e));
                                        }
                                    }
                                });
                            }
                            KeyAction::None => {}
                        }
                    }
                    Some(Ok(Event::Resize(_, _))) => {
                        // Terminal resized — next render will adjust
                    }
                    _ => {}
                }
            }

            // Render on idle — only when no input is pending
            _ = render_interval.tick() => {
                let mut state = state.lock().await;
                if matches!(state.active_tab, app::ActiveTab::Logs) && !state.log_viewer.loaded {
                    state.log_viewer.scan_logs();
                    state.log_viewer.loaded = true;
                }
                terminal.draw(|f| ui::render_ui(f, &state))?;
            }
        }
    }
}

enum KeyAction {
    Exit,
    Send(String),
    ResumeSession(String),
    None,
}

fn send_input(state: &mut AppState) -> KeyAction {
    let input = state.input.lines().join("\n").trim().to_string();
    if input.is_empty() {
        return KeyAction::None;
    }
    state.input = TextArea::default();
    KeyAction::Send(input)
}

fn respond_approval(approval_state: &crate::approval::ApprovalState, approved: bool, reason: Option<String>) {
    tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            *approval_state.response.lock().await = Some((approved, reason));
            approval_state.notify.notify_one();
            // Clear approval state so UI restores the input box
            approval_state.clear().await;
        });
    });
}

fn handle_key(key: KeyEvent, state: &mut AppState) -> KeyAction {
    // Handle approval response keys — highest priority
    if state.approval_state.has_pending_approval() {
        match key.code {
            KeyCode::Char('a') | KeyCode::Char('A') => {
                respond_approval(&state.approval_state, true, None);
                return KeyAction::None;
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                respond_approval(&state.approval_state, false, Some("User rejected".to_string()));
                return KeyAction::None;
            }
            KeyCode::Char('s') | KeyCode::Char('S') => {
                respond_approval(&state.approval_state, false, Some("User stopped execution".to_string()));
                return KeyAction::None;
            }
            _ => {}
        }
    }

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

    // Session dialog navigation — takes precedence over log viewer keys
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
                        let (_, sessions_dir) = derive_store_paths();
                        let store = vol_session::FileSessionEntryStore::new(&sessions_dir);
                        // Block on deletion — near-instant for local file
                        if tokio::task::block_in_place(|| {
                            tokio::runtime::Handle::current().block_on(store.delete_session(&id))
                        }).is_ok() {
                            state.session_dialog.sessions.remove(state.session_dialog.selected);
                            state.session_dialog.selected = 0.min(state.session_dialog.sessions.len().saturating_sub(1));
                        } else {
                            state.last_error = Some(format!("Failed to delete session: {}", id));
                        }
                    }
                }
                return KeyAction::None;
            }
            _ => {}
        }
    }

    match (key.modifiers, key.code) {
        // Alt+Enter: insert newline in multi-line input (check BEFORE plain Enter)
        (KeyModifiers::ALT, KeyCode::Enter) => {
            state.input.input(key);
            KeyAction::None
        }

        // Enter: send input (most terminals don't distinguish Ctrl+Enter)
        (_, KeyCode::Enter) => {
            if state.is_running {
                return KeyAction::None;
            }
            send_input(state)
        }

        // Escape: clear input
        (_, KeyCode::Esc) => {
            state.input = TextArea::default();
            KeyAction::None
        }

        // Tab: switch tabs
        (_, KeyCode::Tab) => {
            state.active_tab = state.active_tab.toggle();
            KeyAction::None
        }

        // PageUp/PageDown: scroll conversation (10-line step)
        (_, KeyCode::PageUp) => {
            state.conversation_scroll = state.conversation_scroll.saturating_sub(10);
            state.conversation_auto_scroll = false;
            KeyAction::None
        }
        (_, KeyCode::PageDown) => {
            state.conversation_scroll = state.conversation_scroll.saturating_add(10);
            state.conversation_auto_scroll = false;
            KeyAction::None
        }

        // Up/Down: scroll conversation (1-line step)
        (_, KeyCode::Up) => {
            state.conversation_scroll = state.conversation_scroll.saturating_sub(1);
            state.conversation_auto_scroll = false;
            KeyAction::None
        }
        (_, KeyCode::Down) => {
            state.conversation_scroll = state.conversation_scroll.saturating_add(1);
            state.conversation_auto_scroll = false;
            KeyAction::None
        }

        // Ctrl+1/2: direct tab switch
        (KeyModifiers::CONTROL, KeyCode::Char('1')) => {
            state.active_tab = app::ActiveTab::Conversation;
            KeyAction::None
        }
        (KeyModifiers::CONTROL, KeyCode::Char('2')) => {
            state.active_tab = app::ActiveTab::Workspace;
            KeyAction::None
        }
        (KeyModifiers::CONTROL, KeyCode::Char('3')) => {
            state.active_tab = app::ActiveTab::Logs;
            KeyAction::None
        }

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
                        .map(|s| app::SessionDialogEntry {
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

        // Quit command
        (_, KeyCode::Char('q')) if key.modifiers == KeyModifiers::CONTROL => {
            if !state.is_running {
                state.exiting = true;
            }
            KeyAction::Exit
        }

        // Unsafe mode toggle
        (_, KeyCode::Char('u')) if key.modifiers == KeyModifiers::CONTROL => {
            state.unsafe_mode = !state.unsafe_mode;
            state.approval_state.unsafe_mode.store(state.unsafe_mode, std::sync::atomic::Ordering::Relaxed);
            state.conversation.push(app::ConversationEntry::AgentAnswer {
                text: if state.unsafe_mode {
                    "Unsafe mode enabled — all tool approvals auto-approved".to_string()
                } else {
                    "Unsafe mode disabled — HITL approval required for dangerous tools".to_string()
                },
            });
            KeyAction::None
        }

        // All other keys: pass to textarea
        _ => {
            state.input.input(key);
            KeyAction::None
        }
    }
}

fn format_age(created_at: i64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
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
