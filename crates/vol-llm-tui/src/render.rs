//! Stream event renderer — converts AgentStreamEvent to colored terminal output.

use crossterm::{
    style::{Color, Print, ResetColor, SetForegroundColor},
    execute,
};
use std::io::{stdout, Write};
use vol_llm_agent::AgentStreamEvent;

fn print_colored(color: Color, text: &str) {
    let _ = execute!(stdout(), SetForegroundColor(color), Print(text), ResetColor);
}

pub fn render_event(event: &AgentStreamEvent) {
    match event {
        AgentStreamEvent::AgentStart { input } => {
            println!();
            print_colored(Color::Cyan, &format!(">>> {}\n", input));
        }

        AgentStreamEvent::ThinkingComplete { thinking } => {
            if !thinking.is_empty() {
                println!();
                print_colored(Color::Yellow, "Thinking...\n");
                print_colored(Color::DarkGrey, &format!("  {}\n", thinking));
            }
        }

        AgentStreamEvent::ToolCallBegin { tool_name, arguments, .. } => {
            println!();
            print_colored(Color::Blue, &format!("[{}] ", tool_name));

            // Try to extract command for display
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(arguments) {
                if let Some(cmd) = parsed.get("command").and_then(|v| v.as_str()) {
                    print_colored(Color::DarkGrey, &format!("Command: {}\n", cmd));
                } else if let Some(path) = parsed.get("path").and_then(|v| v.as_str()) {
                    print_colored(Color::DarkGrey, &format!("Path: {}\n", path));
                } else {
                    print_colored(Color::DarkGrey, &format!("Args: {}\n", arguments));
                }
            } else {
                print_colored(Color::DarkGrey, &format!("Args: {}\n", arguments));
            }
        }

        AgentStreamEvent::ToolCallComplete { tool_name, result, .. } => {
            print_colored(Color::Green, &format!("  ✓ {} completed\n", tool_name));
            // Show truncated result
            let preview = if result.len() > 300 {
                format!("{}...", &result[..300])
            } else {
                result.clone()
            };
            for line in preview.lines().take(10) {
                println!("    {}", line);
            }
        }

        AgentStreamEvent::IterationComplete { final_answer: Some(answer), .. } => {
            println!();
            print_colored(Color::Green, &format!("✓ {}\n", answer));
        }

        AgentStreamEvent::IterationComplete { iteration, .. } => {
            print_colored(Color::White, &format!("\n[Iteration {} complete]\n", iteration));
        }

        AgentStreamEvent::AgentComplete => {
            println!();
            print_colored(Color::Green, "Done.\n");
        }

        AgentStreamEvent::AgentAborted { reason } => {
            println!();
            print_colored(Color::Red, &format!("Aborted: {}\n", reason));
        }

        // Plugin events are internal — skip rendering
        AgentStreamEvent::PluginEvent { .. } => {}
    }
    let _ = stdout().flush();
}
