//! Conversation view showing all message types.

use dioxus::prelude::*;

use crate::state::{
    ConversationEntry, ConversationState, UiEvent,
};

fn truncate_lines(s: &str, max_lines: usize, max_chars: usize) -> String {
    let lines: Vec<&str> = s.lines().take(max_lines).collect();
    let result = lines.join("\n");
    if result.chars().count() > max_chars {
        format!("{}...", result.chars().take(max_chars.saturating_sub(3)).collect::<String>())
    } else { result }
}

fn flush_pending_content(entries: &mut Vec<ConversationEntry>) {
    if let Some(ConversationEntry::ContentStreaming { content }) = entries.last() {
        let text = content.clone();
        if !text.is_empty() {
            *entries.last_mut().unwrap() = ConversationEntry::AgentAnswer { text };
        }
    }
}

pub fn reduce_conversation(s: &mut ConversationState, event: &UiEvent) {
    match event {
        UiEvent::AgentStart { input } => {
            s.entries.push(ConversationEntry::UserInput { text: input.clone() });
            if s.auto_scroll { s.conversation_scroll = 0; }
        }
        UiEvent::AgentComplete { response } => {
            flush_pending_content(&mut s.entries);
            let tc = s.entries.iter().filter(|e| matches!(e, ConversationEntry::ToolCall { .. })).count() as u32;
            s.entries.push(ConversationEntry::RunSummary { iterations: 0, tool_calls: tc, elapsed_ms: 0 });
            if !response.is_empty() {
                s.entries.push(ConversationEntry::AgentAnswer { text: response.clone() });
            }
            if s.auto_scroll { s.conversation_scroll = 0; }
        }
        UiEvent::AgentAborted { reason } | UiEvent::AgentError { message: reason } => {
            flush_pending_content(&mut s.entries);
            s.entries.push(ConversationEntry::Error { message: reason.clone() });
        }
        UiEvent::ThinkingStart => {
            s.entries.push(ConversationEntry::Thinking { content: String::new() });
        }
        UiEvent::ThinkingDelta { delta } => {
            if let Some(ConversationEntry::Thinking { content }) = s.entries.last_mut() {
                content.push_str(delta);
            }
        }
        UiEvent::ContentStart => {
            s.entries.push(ConversationEntry::ContentStreaming { content: String::new() });
        }
        UiEvent::ContentDelta { delta } => {
            if let Some(ConversationEntry::ContentStreaming { content }) = s.entries.last_mut() {
                content.push_str(delta);
            }
        }
        UiEvent::ContentComplete { content } => {
            if let Some(ConversationEntry::ContentStreaming { .. }) = s.entries.last() {
                *s.entries.last_mut().unwrap() = ConversationEntry::AgentAnswer { text: content.clone() };
            } else if !content.is_empty() {
                s.entries.push(ConversationEntry::AgentAnswer { text: content.clone() });
            }
        }
        UiEvent::MaxIterationsReached { current, max } => {
            s.entries.push(ConversationEntry::Error {
                message: format!("Max iterations reached ({}/{}) — waiting for user decision...", current, max),
            });
        }
        UiEvent::IterationContinued { from_iteration } => {
            s.entries.push(ConversationEntry::AgentAnswer {
                text: format!("Continuing from iteration {from_iteration} (counter reset to 0)"),
            });
        }
        UiEvent::IterationComplete { final_answer, .. } => {
            if let Some(answer) = final_answer {
                s.entries.push(ConversationEntry::AgentAnswer { text: answer.clone() });
            }
        }
        _ => {}
    }
}

#[component]
pub fn ConversationView() -> Element {
    let signal: Signal<ConversationState> = use_context();

    let count = signal.read().entries.len();
    if count == 0 {
        return rsx! {
            div { class: "conversation",
                div { class: "conversation-empty", "No messages yet. Type a query and press Send." }
            }
        };
    }

    let entries = signal.read().entries.clone();
    let messages: Vec<Element> = (0..count).map(|index| {
        let entry = entries[index].clone();
        rsx! { MessageEntry { entry } }
    }).collect();
    rsx! {
        div { class: "conversation", {messages.into_iter()} }
    }
}

#[component]
fn MessageEntry(entry: ConversationEntry) -> Element {
    match entry {
        ConversationEntry::UserInput { text } => {
            rsx! { div { class: "msg msg-user", div { class: "msg-user-prefix", ">>> " } {text} } }
        }
        ConversationEntry::Thinking { content } => {
            rsx! { div { class: "msg msg-thinking", div { class: "msg-thinking-prefix", "Thinking" } div { class: "msg-thinking-content", {content} } } }
        }
        ConversationEntry::ContentStreaming { content } => {
            if content.is_empty() { rsx! { div { class: "msg msg-streaming", "Generating..." } } }
            else { rsx! { div { class: "msg msg-streaming", {content} } } }
        }
        ConversationEntry::ToolCall { tool_name, arg_preview } => {
            rsx! { div { class: "msg msg-tool", div { class: "msg-tool-name", "[{tool_name}]" } if !arg_preview.is_empty() { div { class: "msg-tool-arg", "{arg_preview}" } } } }
        }
        ConversationEntry::ToolResult { tool_name, preview, success } => {
            let cls = if success { "msg-tool-result" } else { "msg-tool-result-error" };
            let status = if success { "OK" } else { "ERR" };
            let color = if success { "#40c040" } else { "#c04040" };
            let display = truncate_lines(&preview, 6, 90);
            rsx! { div { class: "msg {cls}", div { span { class: "msg-tool-result-prefix", style: "color: {color};", "[{status}] " } span { style: "color: {color}; font-weight: bold;", "{tool_name}" } } div { class: "msg-tool-result-content", {display} } } }
        }
        ConversationEntry::AgentAnswer { text } => { rsx! { div { class: "msg msg-answer", {text} } } }
        ConversationEntry::RunSummary { iterations, tool_calls, elapsed_ms } => {
            let iw = if iterations == 1 { "iteration" } else { "iterations" };
            let tw = if tool_calls == 1 { "tool call" } else { "tool calls" };
            rsx! { div { class: "msg msg-summary", "Done | {iterations} {iw} | {tool_calls} {tw} | {elapsed_ms}ms" } }
        }
        ConversationEntry::Error { message } => { rsx! { div { class: "msg msg-error", "Error: {message}" } } }
    }
}
