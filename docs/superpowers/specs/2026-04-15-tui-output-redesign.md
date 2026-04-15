# Coding Agent TUI Output Redesign

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the current ad-hoc TUI output with a structured, column-aligned, deduplicated streaming display.

**Architecture:** Introduce an `EventBuffer` struct that tracks rendering state, deduplicates redundant events in real-time, and renders each event with consistent column alignment. Remove duplicate rendering in main.rs.

**Tech Stack:** crossterm (terminal IO), vol_llm_agent::AgentStreamEvent

---

## Context

**Problem:** The TUI has three sources of duplicate output:
1. `main.rs:137` manually renders `AgentStart`, then the agent also emits `AgentStart` — **double render**
2. `IterationComplete(final_answer)` prints the answer, then `AgentComplete` prints "Done.", then `main.rs:160-162` prints `response.content` — **triple render of final answer**
3. `ThinkingComplete` prints `[thinking complete]` after the thinking text was already streamed — **redundant**

Additionally, formatting is inconsistent: tool call lines mix different argument display styles, no column alignment, and spacing varies unpredictably.

**Solution:** Centralize all rendering through a stateful `EventBuffer` that:
- Suppresses `ThinkingComplete` (already showed the text via deltas)
- Suppresses `AgentComplete`'s "Done." — replaces with a one-line summary
- Removes manual rendering from main.rs entirely
- Aligns tool call output in columns

---

### Task 1: Redesign render.rs with EventBuffer

**Files:**
- Modify: `crates/vol-llm-tui/src/render.rs` (full rewrite)

- [ ] **Step 1: Replace render.rs with EventBuffer-based implementation**

Replace the entire contents of `crates/vol-llm-tui/src/render.rs`:

```rust
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
                    "✓ Done · {} iteration{} · {} tool call{} · {:.0}ms\n",
                    self.iteration,
                    if self.iteration == 1 { "" } else { "s" },
                    self.tool_call_count,
                    if self.tool_call_count == 1 { "" } else { "s" },
                    elapsed.as_millis(),
                ));
                // Also print response content if available
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
                print_colored(Color::Red, &format!("✗ Aborted: {}\n", reason));
            }

            // LLM Call — meta events, not displayed
            AgentStreamEvent::LLMCallStart { .. }
            | AgentStreamEvent::LLMCallComplete { .. }
            | AgentStreamEvent::LLMCallError { .. } => {}

            // Thinking — stream inline, suppress ThinkingComplete
            AgentStreamEvent::ThinkingStart { .. } => {
                self.thinking_active = true;
                println!();
                print_colored(Color::Yellow, "⏳ Thinking...\n");
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
                // Just ensure we have a newline if no content was streamed
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
                let dur = duration_ms.map(|ms| format!("{:>6}", format!("{}ms", ms)))
                    .unwrap_or_default();
                print_colored(Color::Green, &format!(
                    "  {:<14} {}\n",
                    format!("✓ {}", tool_name),
                    dur,
                ));
                // Show truncated result preview
                let chars: Vec<char> = result.chars().take(200).collect();
                if !chars.is_empty() {
                    let truncated: String = chars.into_iter().collect();
                    let preview = if chars.len() < result.chars().count() {
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
                    "  {:<14} ✗ {}\n",
                    format!("[{}]", tool_name),
                    error,
                ));
            }

            AgentStreamEvent::ToolCallSkipped { tool_name, reason, .. } => {
                println!();
                print_colored(Color::DarkGrey, &format!(
                    "  {:<14} ⊘ {}\n",
                    format!("[{}]", tool_name),
                    reason,
                ));
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
            // Truncate long commands
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
        // Fallback: show first 80 chars of JSON
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
```

- [ ] **Step 2: Verify compilation**

```bash
cargo check -p vol-llm-tui
```

Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-tui/src/render.rs
git commit -m "refactor: replace render.rs with EventBuffer-based aligned output"
```

---

### Task 2: Rewrite main.rs to use EventBuffer

**Files:**
- Modify: `crates/vol-llm-tui/src/main.rs:1-176`

- [ ] **Step 1: Add EventBuffer import and remove duplicate rendering**

In `main.rs`, replace the entire REPL loop's agent execution section (lines 135-169). The key changes:
1. Remove `render::render_event(&AgentStreamEvent::agent_start(input))` — no manual render
2. Create an `EventBuffer` and hook it up to receive events from the agent
3. Remove `response.content` print — already rendered via IterationComplete

Since the current TUI uses `ReActAgent` directly (not `CodingAgent` with an observer), we need to wire the `EventBuffer` as an observer. The simplest approach: the TUI subscribes to the agent's event broadcast channel and renders events as they arrive.

Actually, looking at the current code more carefully — the TUI creates a `ReActAgent` directly, and the events are broadcast through `run_ctx.event_tx`. But the TUI doesn't currently subscribe to that channel — events are only consumed by the observability plugin and session listener.

The cleanest approach: Create a `TuiObserver` that implements `EventObserver` and renders through `EventBuffer` as events arrive. This requires the CodingAgent integration.

Let me check if CodingAgent is already available in the TUI...

Yes, `vol-llm-agents` is already a dependency in the TUI's Cargo.toml. The right approach is to rewrite main.rs to use `CodingAgent` with a `TuiObserver` that renders via `EventBuffer`.

- [ ] **Step 2: Rewrite main.rs to use CodingAgent with TuiObserver**

Replace the entire `main.rs`:

```rust
//! vol-llm-tui: Interactive CLI for the coding agent.
//!
//! Provides a REPL loop with structured, deduplicated event rendering
//! via EventBuffer.

mod render;

use crossterm::{
    style::{Color, Print, ResetColor, SetForegroundColor},
    execute,
};
use std::io::{self, BufRead, Write};
use std::sync::Arc;
use vol_llm_agents::coding::{CodingAgent, CodingAgentConfig, ChannelledEventObserver};
use vol_llm_agent::AgentStreamEvent;
use vol_llm_tool::{ToolConfig, ProxyConfig};

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

/// Observer that forwards events to EventBuffer for rendering.
struct TuiRenderer {
    buffer: tokio::sync::Mutex<render::EventBuffer>,
}

#[async_trait::async_trait]
impl vol_llm_agents::coding::EventObserver for TuiRenderer {
    async fn on_event(&self, event: &AgentStreamEvent) -> Result<(), vol_llm_agents::coding::ObserverError> {
        let mut buf = self.buffer.lock().await;
        buf.render(event);
        Ok(())
    }

    async fn on_complete(&self) -> Result<(), vol_llm_agents::coding::ObserverError> {
        Ok(())
    }
}

impl TuiRenderer {
    fn new() -> Self {
        Self {
            buffer: tokio::sync::Mutex::new(render::EventBuffer::new()),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load API key
    let api_key = std::env::var("ANTHROPIC_AUTH_TOKEN")
        .expect("ANTHROPIC_AUTH_TOKEN must be set");

    // Print startup banner
    println!();
    print_colored(Color::Cyan, "=== Coding Agent TUI ===\n");
    println!();
    print_colored(Color::White, "Type /help for commands.\n");
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
                // Create CodingAgent with TUI renderer as observer
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

                // Attach TUI renderer as observer
                let renderer = Arc::new(TuiRenderer::new());
                let agent = agent.with_observer(renderer.clone());

                // Run agent — all events render via TuiRenderer → EventBuffer
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
```

- [ ] **Step 3: Verify compilation**

```bash
cargo check -p vol-llm-tui
```

Expected: Compiles successfully

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-tui/src/main.rs
git commit -m "refactor: rewrite TUI to use CodingAgent with EventBuffer renderer"
```

---

### Task 3: Full workspace verification

**Files:** No changes — just verification

- [ ] **Step 1: Full workspace check**

```bash
cargo check --workspace
```

- [ ] **Step 2: Run all tests**

```bash
cargo test --workspace --lib
```

Expected: All existing tests pass

- [ ] **Step 3: Commit**

No changes needed if all passes.

---

## Summary of Changes

| File | Change | Lines |
|------|--------|-------|
| `crates/vol-llm-tui/src/render.rs` | Full rewrite: EventBuffer with aligned columns, dedup | ~180 → ~200 |
| `crates/vol-llm-tui/src/main.rs` | Rewrite: CodingAgent + TuiRenderer observer | ~176 → ~140 |

**Key behavioral changes:**
1. No more duplicate `AgentStart` rendering
2. No more triple-printed final answer
3. `ThinkingComplete` suppressed (text already streamed)
4. `AgentComplete` shows summary: `✓ Done · 3 iterations · 5 tool calls · 1200ms`
5. Tool calls use column-aligned format: `[tool_name]  Command: xxx`
6. Result previews truncated to 200 chars / 6 lines (was 300 chars / 10 lines)
7. `IterationComplete` without final_answer is hidden (tool output shows progress)
