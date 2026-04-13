//! vol-llm-tui: Interactive CLI for the coding agent.
//!
//! Provides a REPL loop for interacting with the ReAct agent,
//! with color-coded event rendering and HITL approval for dangerous tools.

#[allow(dead_code)]
mod render;

use crossterm::{
    style::{Color, Print, ResetColor, SetForegroundColor},
    execute,
};
use std::io::{self, BufRead, Write};
use std::sync::Arc;
use vol_llm_agent::react::AgentBuilder;
use vol_llm_agent::session::{InMemoryMessageStore, InMemorySessionStore, Session};
use vol_llm_provider::{AnthropicProvider, LLMConfig};
use vol_llm_tool::ToolRegistry;

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for diagnostics
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("vol_llm_tui=info".parse()?)
                .add_directive("vol_llm_agent=info".parse()?)
                .add_directive("vol_llm_provider=info".parse()?),
        )
        .with_target(false)
        .init();

    // Load API key
    let api_key = std::env::var("ANTHROPIC_AUTH_TOKEN")
        .expect("ANTHROPIC_AUTH_TOKEN must be set");

    // Create LLM provider
    let llm_config = LLMConfig::with_literal_key(
        vol_llm_core::LLMProvider::Anthropic,
        "qwen3.5-plus",
        api_key,
        "https://coding.dashscope.aliyuncs.com/apps/anthropic",
    );
    let provider = AnthropicProvider::new(&llm_config)?;
    let llm: Arc<dyn vol_llm_core::LLMClient> = Arc::new(provider);

    // Create tools
    let mut tools = ToolRegistry::new();
    vol_llm_tools_builtin::register_all(&mut tools);

    // Register web tools if configured
    let mut tool_config = vol_llm_tool::ToolConfig::new();
    if let Ok(api_key) = std::env::var("TAVILY_API_KEY") {
        let search_cfg = vol_llm_tools_builtin::WebSearchConfig {
            provider: "tavily".to_string(),
            api_key,
            proxy: vol_llm_tool::ProxyConfig::default(),
        };
        tool_config.set("web_search", search_cfg);
    }
    if let Ok(max_len) = std::env::var("WEB_FETCH_MAX_LENGTH") {
        let fetch_cfg = vol_llm_tools_builtin::WebFetchConfig {
            max_content_length: max_len.parse().ok(),
            proxy: vol_llm_tool::ProxyConfig::default(),
        };
        tool_config.set("web_fetch", fetch_cfg);
    }
    vol_llm_tools_builtin::register_web_all(&mut tools, &tool_config);
    let web_tools = tool_config.get::<vol_llm_tools_builtin::WebSearchConfig>("web_search")
        .map(|_| 1).unwrap_or(0)
        + tool_config.get::<vol_llm_tools_builtin::WebFetchConfig>("web_fetch")
        .map(|_| 1).unwrap_or(0);

    // Print startup banner
    println!();
    print_colored(Color::Cyan, "=== Coding Agent TUI ===\n");
    println!();
    print_colored(Color::White, &format!("Core tools: {}\n", tools.definitions().len() - web_tools));
    if web_tools > 0 {
        print_colored(Color::Green, &format!("Web tools: {}\n", web_tools));
    } else {
        print_colored(Color::Yellow, "Web tools: not configured (set TAVILY_API_KEY to enable)\n");
    }
    println!();
    print_help();

    // Main REPL loop
    let stdin = io::stdin();
    loop {
        println!();
        print_colored(Color::Cyan, "> ");
        let _ = io::stdout().flush();

        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) => break, // EOF
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
                // Display user input using render module
                render::render_event(&vol_llm_agent::AgentStreamEvent::AgentStart {
                    input: input.to_string(),
                });

                // Create a new session for each run
                let session = Arc::new(Session::new(
                    format!("tui_{}", uuid::Uuid::new_v4().simple()),
                    Arc::new(InMemorySessionStore::new()),
                    Arc::new(InMemoryMessageStore::new()),
                ));

                // Build agent
                let agent = AgentBuilder::new()
                    .with_llm(llm.clone())
                    .with_max_iterations(10)
                    .with_verbose(false)
                    .with_max_history_messages(20)
                    .with_observability_plugin()
                    .with_session(session)
                    .build()?;

                // Run agent — events are rendered internally via the observer system
                // and HITL approval prompts appear inline for dangerous tools
                match agent.run(input).await {
                    Ok(response) => {
                        if !response.content.is_empty() {
                            println!();
                            print_colored(Color::Green, &format!("{}\n", response.content));
                        }
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
