//! Event buffer that converts AgentStreamEvent into AppState mutations.
//!
//! Instead of printing directly, this maintains state that the ratatui
//! render loop reads from AppState.

use crate::app::{AppState, ConversationEntry, ToolCallEntry, ToolCallStatus};
use vol_llm_core::AgentStreamEvent;

/// Stateful event buffer that tracks rendering state for deduplication.
pub struct EventBuffer {
    thinking_active: bool,
    thinking_buffer: String,
    content_buffer: String,
}

impl EventBuffer {
    pub fn new() -> Self {
        Self {
            thinking_active: false,
            thinking_buffer: String::new(),
            content_buffer: String::new(),
        }
    }

    /// Process an event and mutate AppState accordingly.
    pub fn apply(&mut self, event: &AgentStreamEvent, state: &mut AppState) {
        match event {
            AgentStreamEvent::AgentStart { input, .. } => {
                state.reset_for_run();
                state.conversation.push(ConversationEntry::UserInput {
                    text: input.clone(),
                });
            }

            AgentStreamEvent::AgentComplete { response: _, .. } => {
                // Flush any pending thinking/content
                self.flush_thinking(state);
                self.flush_content();

                let elapsed = state.run_start
                    .map(|s| s.elapsed())
                    .unwrap_or_default();
                state.run_elapsed = elapsed;
                state.conversation.push(ConversationEntry::RunSummary {
                    iterations: state.iteration,
                    tool_calls: state.tool_call_count,
                    elapsed_ms: elapsed.as_millis(),
                });
                state.is_running = false;
            }

            AgentStreamEvent::AgentAborted { reason, .. } => {
                self.flush_thinking(state);
                self.flush_content();
                let elapsed = state.run_start
                    .map(|s| s.elapsed())
                    .unwrap_or_default();
                state.run_elapsed = elapsed;
                state.conversation.push(ConversationEntry::Error {
                    message: reason.clone(),
                });
                state.is_running = false;
            }

            AgentStreamEvent::MaxIterationsReached { current_iteration, max_iterations, .. } => {
                self.flush_thinking(state);
                self.flush_content();
                state.conversation.push(ConversationEntry::Error {
                    message: format!(
                        "Max iterations reached ({}/{}) — waiting for user decision...",
                        current_iteration, max_iterations,
                    ),
                });
            }

            AgentStreamEvent::IterationContinued { from_iteration, .. } => {
                state.conversation.push(ConversationEntry::AgentAnswer {
                    text: format!(
                        "Continuing from iteration {} (counter reset to 0)",
                        from_iteration,
                    ),
                });
            }

            // LLM Call — meta events, not displayed
            AgentStreamEvent::LLMCallStart { .. }
            | AgentStreamEvent::LLMCallComplete { .. }
            | AgentStreamEvent::LLMCallError { .. } => {}

            // Thinking — push empty entry on start, mutate last entry on delta
            AgentStreamEvent::ThinkingStart { .. } => {
                self.thinking_active = true;
                self.thinking_buffer.clear();
                state.conversation.push(ConversationEntry::Thinking {
                    content: String::new(),
                });
            }

            AgentStreamEvent::ThinkingDelta { delta, .. } => {
                // Append to last Thinking entry in-place
                if let Some(ConversationEntry::Thinking { content }) = state.conversation.last_mut() {
                    content.push_str(delta);
                }
            }

            AgentStreamEvent::ThinkingComplete { .. } => {
                self.thinking_active = false;
                // Content already streamed, no-op
            }

            // Content — push empty streaming entry on start, mutate on delta
            AgentStreamEvent::ContentStart { .. } => {
                self.content_buffer.clear();
                state.conversation.push(ConversationEntry::ContentStreaming {
                    content: String::new(),
                });
            }

            AgentStreamEvent::ContentDelta { delta, .. } => {
                // Append to last ContentStreaming entry in-place
                if let Some(ConversationEntry::ContentStreaming { content }) = state.conversation.last_mut() {
                    content.push_str(delta);
                }
            }

            AgentStreamEvent::ContentComplete { content, .. } => {
                // Mutate last ContentStreaming to AgentAnswer (single source)
                if let Some(ConversationEntry::ContentStreaming { .. }) = state.conversation.last() {
                    let entry = state.conversation.last_mut().unwrap();
                    *entry = ConversationEntry::AgentAnswer {
                        text: content.clone(),
                    };
                } else if !content.is_empty() {
                    // Fallback: no streaming entry was pushed
                    state.conversation.push(ConversationEntry::AgentAnswer {
                        text: content.clone(),
                    });
                }
            }

            // Tools
            AgentStreamEvent::ToolCallBegin { tool_name, arguments, .. } => {
                let seq = state.tool_call_count + 1;
                state.tool_call_count = seq;

                let arg_preview = extract_arg_preview(arguments);
                state.tool_calls.push(ToolCallEntry {
                    sequence: seq,
                    tool_name: tool_name.clone(),
                    arg_preview: arg_preview.clone(),
                    status: ToolCallStatus::Running,
                    duration_ms: None,
                });
                state.conversation.push(ConversationEntry::ToolCall {
                    tool_name: tool_name.clone(),
                    arg_preview,
                });
            }

            AgentStreamEvent::ToolCallComplete { tool_name, result, duration_ms, .. } => {
                self.update_tool_call_status(state, tool_name, ToolCallStatus::Success, *duration_ms);
                let preview = truncate_preview(result, 200);
                state.conversation.push(ConversationEntry::ToolResult {
                    tool_name: tool_name.clone(),
                    preview,
                    success: true,
                });

                // Track modified files
                if tool_name.contains("Write") || tool_name.contains("Edit") {
                    if let Some(path) = self.extract_file_path_from_result(result) {
                        state.modified_files.insert(path);
                    }
                }
            }

            AgentStreamEvent::ToolCallError { tool_name, error, duration_ms, .. } => {
                self.update_tool_call_status(state, tool_name, ToolCallStatus::Error, *duration_ms);
                state.conversation.push(ConversationEntry::ToolResult {
                    tool_name: tool_name.clone(),
                    preview: error.clone(),
                    success: false,
                });
            }

            AgentStreamEvent::ToolCallSkipped { tool_name, reason, duration_ms, .. } => {
                self.update_tool_call_status(state, tool_name, ToolCallStatus::Skipped, *duration_ms);
                state.conversation.push(ConversationEntry::ToolResult {
                    tool_name: tool_name.clone(),
                    preview: reason.clone(),
                    success: false,
                });
            }

            // Tool argument streaming delta — invisible in TUI (content only)
            AgentStreamEvent::ToolCallArgumentDelta { .. } => {}

            // Iteration
            AgentStreamEvent::IterationComplete { final_answer: Some(answer), iteration, .. } => {
                state.iteration = *iteration;
                state.conversation.push(ConversationEntry::AgentAnswer {
                    text: answer.clone(),
                });
                // Flush content when iteration completes
                self.flush_content();
            }

            AgentStreamEvent::IterationComplete { iteration, .. } => {
                state.iteration = *iteration;
                // Flush content when iteration completes
                self.flush_content();
            }

            // Plugin events — invisible
            AgentStreamEvent::PluginEvent { .. } => {}
        }

        // Auto-scroll conversation to bottom on new content.
        if state.conversation_auto_scroll {
            // Scroll position will be computed by render layer based on auto_scroll flag
            state.conversation_scroll = 0;
        }
        // Auto-scroll tools panel to bottom
        state.tools_scroll = state.tool_calls.len() as u16;
    }

    fn flush_thinking(&mut self, state: &mut AppState) {
        if self.thinking_active && !self.thinking_buffer.is_empty() {
            state.conversation.push(ConversationEntry::Thinking {
                content: std::mem::take(&mut self.thinking_buffer),
            });
            self.thinking_active = false;
        }
    }

    fn flush_content(&mut self) {
        if !self.content_buffer.is_empty() {
            // Content is handled via ContentComplete, buffer is just a fallback
            self.content_buffer.clear();
        }
    }

    fn update_tool_call_status(
        &mut self,
        state: &mut AppState,
        tool_name: &str,
        status: ToolCallStatus,
        duration_ms: Option<u64>,
    ) {
        for entry in state.tool_calls.iter_mut().rev() {
            if entry.tool_name == tool_name && matches!(entry.status, ToolCallStatus::Running) {
                entry.status = status;
                entry.duration_ms = duration_ms;
                break;
            }
        }
    }

    fn extract_file_path_from_result(&self, result: &str) -> Option<String> {
        // Try to extract file_path from JSON result
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(result) {
            if let Some(path) = parsed.get("file_path").and_then(|v| v.as_str()) {
                return Some(path.to_string());
            }
        }
        None
    }
}

/// Extract a short preview of tool arguments for display.
fn extract_arg_preview(arguments: &str) -> String {
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(arguments) {
        if let Some(cmd) = parsed.get("command").and_then(|v| v.as_str()) {
            if cmd.chars().count() > 80 {
                let truncated: String = cmd.chars().take(77).collect();
                return format!("Command: {}...", truncated);
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
        if arguments.chars().count() > 80 {
            let truncated: String = arguments.chars().take(77).collect();
            return format!("Args: {}...", truncated);
        }
        return format!("Args: {}", arguments);
    }
    String::new()
}

fn truncate_preview(s: &str, max_chars: usize) -> String {
    let total_chars = s.chars().count();
    if total_chars <= max_chars {
        return s.to_string();
    }
    let truncated: String = s.chars().take(max_chars).collect();
    format!("{}...", truncated)
}
