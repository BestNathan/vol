//! vol-llm-tui: Interactive CLI for the coding agent.
//!
//! Provides a REPL loop with structured, deduplicated event rendering
//! via EventBuffer.

mod app;
mod render;

use crate::app::AppState;
use crossterm::{
    style::{Color, Print, ResetColor, SetForegroundColor},
    execute,
};
use std::io::{self, BufRead, Write};
use std::sync::Mutex;
use std::sync::Arc;
use vol_llm_agents::coding::{CodingAgent, CodingAgentConfig, EventObserver, ObserverError};
use vol_llm_core::AgentStreamEvent;
use vol_llm_tool::{ToolConfig, ProxyConfig};
use vol_session::FileMessageStore;

fn print_colored(color: Color, text: &str) {
    let _ = execute!(io::stdout(), SetForegroundColor(color), Print(text), ResetColor);
}

fn print_help() {
    println!();
    println!("Commands:");
    println!("  /quit, /exit  - Exit the TUI");
    println!("  /help         - Show this help message");
    println!("  /clear        - Clear screen");
    println!();
    println!("Type any message to send to the agent.");
}

/// Observer that forwards events to EventBuffer which mutates AppState.
struct TuiRenderer {
    buffer: Mutex<render::EventBuffer>,
    state: Arc<Mutex<AppState>>,
}

#[async_trait::async_trait]
impl EventObserver for TuiRenderer {
    async fn on_event(&self, event: &AgentStreamEvent) -> Result<(), ObserverError> {
        let mut buf = self.buffer.lock().unwrap();
        let mut state = self.state.lock().unwrap();
        buf.apply(event, &mut state);
        Ok(())
    }

    async fn on_complete(&self) -> Result<(), ObserverError> {
        Ok(())
    }
}

impl TuiRenderer {
    fn new(state: Arc<Mutex<AppState>>) -> Self {
        Self {
            buffer: Mutex::new(render::EventBuffer::new()),
            state,
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Verify API key is set (CodingAgent::new() reads from env internally)
    let _api_key = std::env::var("ANTHROPIC_AUTH_TOKEN")
        .expect("ANTHROPIC_AUTH_TOKEN must be set");

    // Print startup banner
    println!();
    print_colored(Color::Cyan, "=== Coding Agent TUI ===\n");
    println!();
    print_colored(Color::White, "Type /help for commands.\n");
    println!();
    print_help();

    // Create persistent session for this TUI run
    let session: Arc<vol_llm_agents::coding::Session> = {
        let session_dir = std::env::current_dir()
            .unwrap_or_default()
            .join(".vol-sessions");
        if let Err(e) = std::fs::create_dir_all(&session_dir) {
            print_colored(Color::Yellow, &format!("Warning: cannot create session dir: {}\n", e));
            print_colored(Color::Yellow, "Using in-memory session (no history persistence)\n");
            use vol_session::{InMemorySessionStore, InMemoryMessageStore};
            Arc::new(vol_llm_agents::coding::Session::new(
                "tui_memory".to_string(),
                Arc::new(InMemorySessionStore::new()),
                Arc::new(InMemoryMessageStore::new()),
            ))
        } else {
            let session_id = format!("tui_{}", chrono::Utc::now().format("%Y%m%d_%H%M%S%.3f"));
            let message_store = Arc::new(FileMessageStore::new(&session_dir, &session_id));
            let session_store = Arc::new(vol_session::InMemorySessionStore::new());
            let session = Arc::new(vol_llm_agents::coding::Session::new(
                session_id.clone(),
                session_store,
                message_store,
            ));
            print_colored(Color::Green, &format!("Session: {}\n", session_id));
            session
        }
    };

    // Main REPL loop
    let stdin = io::stdin();
    loop {
        println!();
        print_colored(Color::Cyan, "> ");
        let _ = io::stdout().flush();

        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {}
            Err(e) => {
                eprintln!("Error reading input: {}", e);
                continue;
            }
        }

        let input = line.trim();

        match input {
            "" => continue,
            "/quit" | "/exit" => {
                print_colored(Color::Yellow, "Goodbye!\n");
                break;
            }
            "/help" => {
                print_help();
                continue;
            }
            "/clear" => {
                print!("\x1B[2J\x1B[1;1H");
                let _ = io::stdout().flush();
                continue;
            }
            _ => {
                // Configure tool_config
                let mut tool_config = ToolConfig::new();

                // Register web tools if configured
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

                let config = CodingAgentConfig {
                    max_iterations: 10,
                    working_dir: std::env::current_dir()?,
                    hitl_enabled: true,
                    verbose: false,
                    html_report_path: None,
                    session: Some(session.clone()),
                    tool_config,
                    ..Default::default()
                };

                let agent = match CodingAgent::new(config).await {
                    Ok(a) => a,
                    Err(e) => {
                        print_colored(Color::Red, &format!("Error creating agent: {}\n", e));
                        continue;
                    }
                };

                // Create AppState for this run
                let working_dir = std::env::current_dir()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let session_id = session.id.clone();
                let state = Arc::new(Mutex::new(AppState::new(session_id, &working_dir)));

                // Attach TUI renderer as observer
                let renderer = Arc::new(TuiRenderer::new(state.clone()));
                let agent = agent.with_observer(renderer.clone());

                // Run agent — all events render via TuiRenderer -> EventBuffer
                match agent.run(input).await {
                    Ok(_response) => {
                        // Final answer already rendered via IterationComplete
                        // AgentComplete summary line also rendered by EventBuffer
                    }
                    Err(e) => {
                        println!();
                        print_colored(Color::Red, &format!("Error: {}\n", e));
                    }
                }
            }
        }
    }

    Ok(())
}
