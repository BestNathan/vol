// crates/vol-llm-ui/src/tui/bin/tui.rs
//
// vol-llm-tui: Terminal UI for agent interaction using ratatui.

use std::path::PathBuf;
use std::sync::Arc;

use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    event::{Event, EventStream},
};
use futures::StreamExt;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::RwLock;
use vol_llm_agents::coding::CodingAgentConfig;
use vol_llm_ui::AgentConnection;
use vol_llm_ui::LocalConnection;
use vol_llm_ui::state::UiState;
use vol_llm_ui::tui::{handle_key, render_ui, InputAction};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Verify API key
    let _api_key = std::env::var("ANTHROPIC_AUTH_TOKEN")
        .expect("ANTHROPIC_AUTH_TOKEN must be set");

    // Parse args
    let working_dir = std::env::current_dir().unwrap_or_default();
    let project = working_dir
        .file_name()
        .unwrap_or(std::ffi::OsStr::new("default"))
        .to_string_lossy();
    let store_dir =
        PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into()))
            .join(".vol-coding")
            .join(project.as_ref())
            .join("sessions");

    // Create shared state
    let session_id = uuid::Uuid::new_v4().to_string();
    let (render_tx, mut render_rx) = tokio::sync::mpsc::channel(64);
    let ui_state = Arc::new(RwLock::new(UiState::new(
        session_id,
        working_dir.to_string_lossy().as_ref(),
        "local",
    )));

    // Build agent config
    let agent_config = CodingAgentConfig {
        working_dir: working_dir.clone(),
        store_dir: store_dir.clone(),
        ..CodingAgentConfig::default()
    };

    // Create connection (observer updates state directly)
    let connection = LocalConnection::new(agent_config, ui_state.clone(), render_tx.clone());
    let connection = Arc::new(connection);

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic| {
        let _ = disable_raw_mode();
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen);
        original_hook(panic);
    }));

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    // Main loop state
    let mut events = EventStream::new();
    let mut input_buf = String::new();

    loop {
        tokio::select! {
            biased;

            // Render — highest priority
            _ = render_rx.recv() => {
                let state = ui_state.read().await;
                terminal.draw(|f| render_ui(f, &state))?;
            }

            // Input
            maybe_event = events.next() => {
                match maybe_event {
                    Some(Ok(Event::Key(key))) => {
                        let mut state = ui_state.write().await;
                        let action = handle_key(key, &mut state, &input_buf);

                        match action {
                            InputAction::Exit => break,
                            InputAction::Send(text) => {
                                input_buf.clear();
                                state.is_running = true;
                                let conn = connection.clone();
                                let state_clone = ui_state.clone();
                                let render_tx_clone = render_tx.clone();
                                tokio::spawn(async move {
                                    match conn.submit(text).await {
                                        Ok(rx) => {
                                            let mut rx = rx;
                                            while let Some(_event) = rx.recv().await {
                                                // Events applied by observer
                                            }
                                            // Ensure is_running is cleared
                                            let mut s = state_clone.write().await;
                                            s.is_running = false;
                                            drop(s);
                                            let _ = render_tx_clone.try_send(());
                                        }
                                        Err(e) => {
                                            let mut s = state_clone.write().await;
                                            s.is_running = false;
                                            s.last_error = Some(format!("{}", e));
                                            drop(s);
                                            let _ = render_tx_clone.try_send(());
                                        }
                                    }
                                });
                            }
                            InputAction::ResumeSession(_id) => {
                                // TODO: Resume session via connection
                            }
                            InputAction::None => {}
                        }
                        // Trigger render after input handling
                        let _ = render_tx.try_send(());
                    }
                    Some(Ok(Event::Resize(_, _))) => {}
                    _ => {}
                }
            }
        }
    }

    // Cleanup
    execute!(std::io::stdout(), LeaveAlternateScreen)?;
    disable_raw_mode()?;

    Ok(())
}
