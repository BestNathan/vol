//! Stream event renderer — converts AgentStreamEvent to aligned terminal output.
//!
//! Uses EventBuffer to track state and deduplicate redundant events:
//! - ThinkingComplete is suppressed (thinking text already streamed)
//! - AgentComplete renders a summary line, not just "Done."
//! - Tool calls use column-aligned formatting

use crossterm::{
    style::{Color, Print, ResetColor, SetForegroundColor},
    execute,
};
use std::io::{stdout, Write};
use std::time::Duration;
use vol_llm_agent::AgentStreamEvent;

/// Stateful event buffer that tracks rendering state for deduplication.
pub struct EventBuffer {
    iteration: u32,
    tool_call_count: u32,
    run_start: Option<std::time::Instant>,
    thinking_active: bool,
}

impl EventBuffer {
    pub fn new() -> Self {
        Self {
            iteration: 0,
            tool_call_count: 0,
            run_start: None,
            thinking_active: false,
        }
    }

    /// Start tracking a new agent run
    pub fn start_run(&mut self) {
        self.iteration = 0;
        self.tool_call_count = 0;
        self.run_start = Some(std::time::Instant::now());
        self.thinking_active = false;
    }

    /// Get total elapsed time for the run
    pub fn elapsed(&self) -> Duration {
        self.run_start.map(|s| s.elapsed()).unwrap_or_default()
    }

    /// Render a single event with deduplication and alignment.
    pub fn render(&mut self, event: &AgentStreamEvent) {
        match event {
            AgentStreamEvent::AgentStart { input, .. } => {
                self.start_run();
                println!();
                print_colored(Color::Cyan, &format!(">>> {}\n", input));
            }

            AgentStreamEvent::AgentComplete { response, .. } => {
                let elapsed = self.elapsed();
                println!();
                print_colored(Color::Green, &format!(
                    "Done · {} iteration{} · {} tool call{} · {:.0}ms\n",
                    self.iteration,
                    if self.iteration == 1 { "" } else { "s" },
                    self.tool_call_count,
                    if self.tool_call_count == 1 { "" } else { "s" },
                    elapsed.as_millis(),
                ));
                // Print response content from the event payload
                if let Some(resp) = response {
                    if let Some(content) = resp.get("content").and_then(|v| v.as_str()) {
                        if !content.is_empty() {
                            println!();
                            print_colored(Color::White, content);
                            println!();
                        }
                    }
                }
            }

            AgentStreamEvent::AgentAborted { reason, .. } => {
                println!();
                print_colored(Color::Red, &format!("Aborted: {}\n", reason));
            }

            AgentStreamEvent::MaxIterationsReached { current_iteration, max_iterations, .. } => {
                println!();
                print_colored(Color::Yellow, &format!(
                    "\u{26a0} Max iterations reached ({}/{}) — waiting for user decision...\n",
                    current_iteration, max_iterations,
                ));
            }

            AgentStreamEvent::IterationContinued { from_iteration, .. } => {
                println!();
                print_colored(Color::Green, &format!(
                    ">>> Continuing from iteration {} (counter reset to 0)\n",
                    from_iteration,
                ));
            }

            // LLM Call — meta events, not displayed
            AgentStreamEvent::LLMCallStart { .. }
            | AgentStreamEvent::LLMCallComplete { .. }
            | AgentStreamEvent::LLMCallError { .. } => {}

            // Thinking — stream inline, suppress ThinkingComplete
            AgentStreamEvent::ThinkingStart { .. } => {
                self.thinking_active = true;
                println!();
                print_colored(Color::Yellow, "Thinking...\n");
            }

            AgentStreamEvent::ThinkingDelta { delta, .. } => {
                print_colored(Color::DarkGrey, delta);
            }

            AgentStreamEvent::ThinkingComplete { .. } => {
                // Suppress — the delta text already showed the thinking
                self.thinking_active = false;
            }

            // Content — stream inline
            AgentStreamEvent::ContentStart { .. } => {
                println!();
            }

            AgentStreamEvent::ContentDelta { delta, .. } => {
                print_colored(Color::White, delta);
            }

            AgentStreamEvent::ContentComplete { .. } => {
                // No-op — content already streamed via deltas
            }

            // Tools — column-aligned format
            AgentStreamEvent::ToolCallBegin { tool_name, arguments, .. } => {
                self.tool_call_count += 1;
                let arg_preview = extract_arg_preview(arguments);
                println!();
                print_colored(Color::Blue, &format!(
                    "{:<16} {}\n",
                    format!("[{}]", tool_name),
                    arg_preview,
                ));
            }

            AgentStreamEvent::ToolCallComplete { tool_name, result, duration_ms, .. } => {
                let dur = duration_ms.map(|ms| format!("{}ms", ms))
                    .unwrap_or_default();
                print_colored(Color::Green, &format!(
                    "  {:<14} {}\n",
                    format!("OK {}", tool_name),
                    dur,
                ));
                // Show truncated result preview
                let total_chars = result.chars().count();
                let chars: Vec<char> = result.chars().take(200).collect();
                if !chars.is_empty() {
                    let truncated: String = chars.into_iter().collect();
                    let preview = if truncated.chars().count() < total_chars {
                        format!("{}...", truncated)
                    } else {
                        truncated
                    };
                    for line in preview.lines().take(6) {
                        print_colored(Color::DarkGrey, &format!("    {}\n", line));
                    }
                }
            }

            AgentStreamEvent::ToolCallError { tool_name, error, .. } => {
                println!();
                print_colored(Color::Red, &format!(
                    "  {:<14} {}\n",
                    format!("[{}]", tool_name),
                    error,
                ));
            }

            AgentStreamEvent::ToolCallSkipped { tool_name, reason, .. } => {
                println!();
                print_colored(Color::DarkGrey, &format!(
                    "  {:<14} {}\n",
                    format!("[{}]", tool_name),
                    reason,
                ));
            }

            AgentStreamEvent::ToolCallArgumentDelta { tool_name, delta, .. } => {
                print_colored(Color::DarkGrey, &format!("\r  {:<14} {} bytes\r", format!("[{}] args...", tool_name), delta.len()));
            }

            // Iteration — show final answer only, skip bare iteration complete
            AgentStreamEvent::IterationComplete { final_answer: Some(answer), iteration, .. } => {
                self.iteration = *iteration;
                println!();
                print_colored(Color::Green, &format!(">>> {}\n", answer));
            }

            AgentStreamEvent::IterationComplete { iteration, .. } => {
                self.iteration = *iteration;
                // Skip bare iteration complete — tool output already shows progress
            }

            // Plugin events — invisible
            AgentStreamEvent::PluginEvent { .. } => {}
        }
        let _ = stdout().flush();
    }
}

/// Extract a short preview of tool arguments for display.
fn extract_arg_preview(arguments: &str) -> String {
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(arguments) {
        if let Some(cmd) = parsed.get("command").and_then(|v| v.as_str()) {
            if cmd.len() > 80 {
                return format!("Command: {}...", &cmd[..77]);
            }
            return format!("Command: {}", cmd);
        }
        if let Some(path) = parsed.get("path").and_then(|v| v.as_str()) {
            return format!("Path: {}", path);
        }
        if let Some(file_path) = parsed.get("file_path").and_then(|v| v.as_str()) {
            return format!("File: {}", file_path);
        }
        if let Some(url) = parsed.get("url").and_then(|v| v.as_str()) {
            return format!("URL: {}", url);
        }
        if arguments.len() > 80 {
            return format!("Args: {}...", &arguments[..77]);
        }
        return format!("Args: {}", arguments);
    }
    String::new()
}

fn print_colored(color: Color, text: &str) {
    let _ = execute!(stdout(), SetForegroundColor(color), Print(text), ResetColor);
}
