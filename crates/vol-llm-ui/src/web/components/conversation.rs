//! Conversation view showing all message types.

use dioxus::prelude::*;

use crate::state::ConversationEntry;
use crate::web::components::app::AppState;

/// Truncate text to at most `max_lines` lines, each at most `max_chars` chars.
fn truncate_lines(s: &str, max_lines: usize, max_chars: usize) -> String {
    let lines: Vec<&str> = s.lines().take(max_lines).collect();
    let result = lines.join("\n");
    if result.chars().count() > max_chars {
        let truncated: String = result.chars().take(max_chars.saturating_sub(3)).collect();
        format!("{}...", truncated)
    } else {
        result
    }
}

/// Conversation panel displaying all messages.
#[component]
pub fn ConversationView() -> Element {
    let state: AppState = use_context();
    let count = state.signal.read().conversation.len();

    if count == 0 {
        return rsx! {
            div { class: "conversation",
                div { class: "conversation-empty", "No messages yet. Type a query and press Send." }
            }
        };
    }

    // Use indexed rendering
    let messages: Vec<Element> = (0..count).map(|index| {
        let s = state.clone();
        rsx! {
            MessageEntry { index, state: s }
        }
    }).collect();
    rsx! {
        div { class: "conversation",
            {messages.into_iter()}
        }
    }
}

#[component]
fn MessageEntry(state: AppState, index: usize) -> Element {
    // Clone the entry at this index out of state
    let entry = state.signal.read().conversation.get(index).cloned();

    let Some(entry) = entry else {
        return rsx! {};
    };

    match entry {
        ConversationEntry::UserInput { text } => {
            rsx! {
                div { class: "msg msg-user",
                    div { class: "msg-user-prefix", ">>> " }
                    {text}
                }
            }
        }

        ConversationEntry::Thinking { content } => {
            rsx! {
                div { class: "msg msg-thinking",
                    div { class: "msg-thinking-prefix", "Thinking" }
                    div { class: "msg-thinking-content",
                        {content}
                    }
                }
            }
        }

        ConversationEntry::ContentStreaming { content } => {
            if content.is_empty() {
                rsx! {
                    div { class: "msg msg-streaming", "Generating..." }
                }
            } else {
                rsx! {
                    div { class: "msg msg-streaming",
                        {content}
                    }
                }
            }
        }

        ConversationEntry::ToolCall { tool_name, arg_preview } => {
            rsx! {
                div { class: "msg msg-tool",
                    div { class: "msg-tool-name", "[{tool_name}]" }
                    if !arg_preview.is_empty() {
                        div { class: "msg-tool-arg", "{arg_preview}" }
                    }
                }
            }
        }

        ConversationEntry::ToolResult { tool_name, preview, success } => {
            let cls = if success { "msg-tool-result" } else { "msg-tool-result-error" };
            let status = if success { "OK" } else { "ERR" };
            let status_color = if success { "#40c040" } else { "#c04040" };

            // Show truncated preview
            let display_preview = truncate_lines(&preview, 6, 90);

            rsx! {
                div { class: "msg {cls}",
                    div {
                        span { class: "msg-tool-result-prefix", style: "color: {status_color};", "[{status}] " }
                        span { style: "color: {status_color}; font-weight: bold;", "{tool_name}" }
                    }
                    div { class: "msg-tool-result-content",
                        {display_preview}
                    }
                }
            }
        }

        ConversationEntry::AgentAnswer { text } => {
            rsx! {
                div { class: "msg msg-answer",
                    {text}
                }
            }
        }

        ConversationEntry::RunSummary { iterations, tool_calls, elapsed_ms } => {
            let iter_word = if iterations == 1 { "iteration" } else { "iterations" };
            let tc_word = if tool_calls == 1 { "tool call" } else { "tool calls" };
            rsx! {
                div { class: "msg msg-summary",
                    "Done | {iterations} {iter_word} | {tool_calls} {tc_word} | {elapsed_ms}ms"
                }
            }
        }

        ConversationEntry::Error { message } => {
            rsx! {
                div { class: "msg msg-error",
                    "Error: {message}"
                }
            }
        }
    }
}
