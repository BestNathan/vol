//! vol-llm-tui: Interactive ratatui TUI for the coding agent.
//!
//! Provides a full terminal UI with status bar, tool call panel,
//! tabbed conversation/workspace views, multi-line input, and persistent layout.

mod app;
mod approval;
mod render;
mod ui;

use std::io::{self, stdout};
use std::sync::Arc;
use std::time::Duration;

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
use vol_llm_agents::coding::{CodingAgent, CodingAgentConfig, EventObserver, ObserverError};
use vol_llm_core::AgentStreamEvent;
use vol_llm_tool::{ToolConfig, ProxyConfig};
use vol_session::FileMessageStore;

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
    let session: Arc<vol_llm_agents::coding::Session> = create_session()?;
    let session_id = session.id.clone();

    // Setup terminal with panic recovery
    setup_terminal()?;
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic| {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen);
        original_hook(panic);
    }));

    // Create shared state
    let working_dir = std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let state = Arc::new(tokio::sync::Mutex::new(
        AppState::new(session_id, &working_dir),
    ));

    // Create ratatui terminal
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    // Main event loop
    let result = run_event_loop(&mut terminal, state, session).await;

    // Cleanup
    cleanup_terminal()?;

    result
}

fn create_session() -> Result<Arc<vol_llm_agents::coding::Session>, Box<dyn std::error::Error>> {
    let session_dir = std::env::current_dir()
        .unwrap_or_default()
        .join(".vol-sessions");

    if let Err(e) = std::fs::create_dir_all(&session_dir) {
        eprintln!("Warning: cannot create session dir: {}", e);
        eprintln!("Using in-memory session (no history persistence)");
        use vol_session::InMemoryMessageStore;
        use vol_llm_agent::session::InMemorySessionStore;
        return Ok(Arc::new(vol_llm_agents::coding::Session::new(
            "tui_memory".to_string(),
            Arc::new(InMemorySessionStore::new()),
            Arc::new(InMemoryMessageStore::new()),
        )));
    }

    let session_id = format!("tui_{}", chrono::Utc::now().format("%Y%m%d_%H%M%S%.3f"));
    let message_store = Arc::new(FileMessageStore::new(&session_dir, &session_id));
    let session_store = Arc::new(vol_session::InMemorySessionStore::new());
    Ok(Arc::new(vol_llm_agents::coding::Session::new(
        session_id.clone(),
        session_store,
        message_store,
    )))
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
    session: Arc<vol_llm_agents::coding::Session>,
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
                                spawn_agent(input, state.clone(), session.clone());
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
                let state = state.lock().await;
                terminal.draw(|f| ui::render_ui(f, &state))?;
            }
        }
    }
}

enum KeyAction {
    Exit,
    Send(String),
    None,
}

fn respond_approval(approval_state: &crate::approval::ApprovalState, approved: bool, reason: Option<String>) {
    tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            *approval_state.response.lock().await = Some((approved, reason));
            approval_state.notify.notify_one();
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

    match (key.modifiers, key.code) {
        // Ctrl+Enter: send input
        (KeyModifiers::CONTROL, KeyCode::Enter) => {
            if state.is_running {
                return KeyAction::None;
            }
            let input = state.input.lines().join("\n").trim().to_string();
            if input.is_empty() {
                return KeyAction::None;
            }
            state.input = TextArea::default();
            KeyAction::Send(input)
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

        // PageUp/PageDown: scroll conversation
        (_, KeyCode::PageUp) => {
            if state.conversation_scroll > 0 {
                state.conversation_scroll -= 1;
                state.conversation_auto_scroll = false;
            }
            KeyAction::None
        }
        (_, KeyCode::PageDown) => {
            state.conversation_scroll += 1;
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

        // Quit command
        (_, KeyCode::Char('q')) if key.modifiers == KeyModifiers::CONTROL => {
            if !state.is_running {
                state.exiting = true;
            }
            KeyAction::Exit
        }

        // All other keys: pass to textarea
        _ => {
            state.input.input(key);
            KeyAction::None
        }
    }
}

fn spawn_agent(
    input: String,
    state: Arc<tokio::sync::Mutex<AppState>>,
    session: Arc<vol_llm_agents::coding::Session>,
) {
    tokio::spawn(async move {
        // Set running flag and clear approval state
        {
            let mut state = state.lock().await;
            state.is_running = true;
            state.approval_state.clear().await;
        }

        // Configure tools
        let mut tool_config = ToolConfig::new();
        if let Ok(tavily_key) = std::env::var("TAVILY_API_KEY") {
            tool_config.set("web_search", vol_llm_tools_builtin::WebSearchConfig {
                provider: "tavily".to_string(),
                api_key: tavily_key,
                proxy: ProxyConfig::default(),
            });
        }
        if let Ok(max_len) = std::env::var("WEB_FETCH_MAX_LENGTH") {
            tool_config.set("web_fetch", vol_llm_tools_builtin::WebFetchConfig {
                max_content_length: max_len.parse().ok(),
                proxy: ProxyConfig::default(),
            });
        }

        let working_dir = std::env::current_dir().unwrap_or_default();
        let unsafe_mode = {
            let state_guard = state.lock().await;
            state_guard.unsafe_mode
        };

        // Get approval state for handler
        let approval_state = {
            let state_guard = state.lock().await;
            state_guard.approval_state.clone()
        };

        let config = CodingAgentConfig {
            max_iterations: 10,
            working_dir,
            hitl_enabled: !unsafe_mode,
            unsafe_mode,
            approval_handler: if !unsafe_mode {
                Some(approval_state.into_handler())
            } else {
                None
            },
            verbose: false,
            html_report_path: None,
            session: Some(session.clone()),
            tool_config,
            ..Default::default()
        };

        let agent = match CodingAgent::new(config).await {
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
            Ok(_response) => {
                // All events handled via observer
            }
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
