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
    let conv = s.active_mut();
    match event {
        UiEvent::AgentStart { input } => {
            conv.entries.clear();
            conv.entries.push(ConversationEntry::UserInput { text: input.clone() });
        }
        UiEvent::AgentComplete { response: _ } => {
            flush_pending_content(&mut conv.entries);
            let tc = conv.entries.iter().filter(|e| matches!(e, ConversationEntry::ToolCall { .. })).count() as u32;
            conv.entries.push(ConversationEntry::RunSummary { iterations: 0, tool_calls: tc, elapsed_ms: 0 });
        }
        UiEvent::AgentAborted { reason } | UiEvent::AgentError { message: reason } => {
            flush_pending_content(&mut conv.entries);
            conv.entries.push(ConversationEntry::Error { message: reason.clone() });
        }
        UiEvent::ThinkingStart => {
            conv.entries.push(ConversationEntry::Thinking { content: String::new() });
        }
        UiEvent::ThinkingDelta { delta } => {
            if let Some(ConversationEntry::Thinking { content }) = conv.entries.last_mut() {
                content.push_str(delta);
            }
        }
        UiEvent::ThinkingComplete => {
            // No-op — thinking content already streamed via deltas
        }
        UiEvent::LlmCallStart { iteration } => {
            conv.entries.push(ConversationEntry::LlmCall { iteration: *iteration, model: String::new() });
        }
        UiEvent::LlmCallComplete { model } => {
            if let Some(ConversationEntry::LlmCall { model: m, .. }) = conv.entries.last_mut() {
                *m = model.clone();
            }
        }
        UiEvent::LlmCallError { error } => {
            conv.entries.push(ConversationEntry::Error { message: format!("LLM error: {error}") });
        }
        UiEvent::ContentStart => {
            conv.entries.push(ConversationEntry::ContentStreaming { content: String::new() });
        }
        UiEvent::ContentDelta { delta } => {
            if let Some(ConversationEntry::ContentStreaming { content }) = conv.entries.last_mut() {
                content.push_str(delta);
            }
        }
        UiEvent::ContentComplete { content } => {
            if let Some(ConversationEntry::ContentStreaming { .. }) = conv.entries.last() {
                *conv.entries.last_mut().unwrap() = ConversationEntry::AgentAnswer { text: content.clone() };
            } else if !content.is_empty() {
                conv.entries.push(ConversationEntry::AgentAnswer { text: content.clone() });
            }
        }
        UiEvent::ToolCallBegin { tool_name, arguments } => {
            // Extract a brief preview for display
            let preview = if arguments.is_empty() {
                String::new()
            } else if let Ok(v) = serde_json::from_str::<serde_json::Value>(arguments) {
                v.get("command").and_then(|v| v.as_str())
                    .map(|c| if c.len() > 80 { format!("Command: {}...", &c[..77]) } else { format!("Command: {}", c) })
                    .or_else(|| v.get("path").and_then(|v| v.as_str()).map(|p| format!("Path: {}", p)))
                    .or_else(|| v.get("file_path").and_then(|v| v.as_str()).map(|f| format!("File: {}", f)))
                    .unwrap_or_else(|| format!("Args: {}", if arguments.len() > 80 { format!("{}...", &arguments[..77]) } else { arguments.clone() }))
            } else {
                format!("Args: {}", if arguments.len() > 80 { format!("{}...", &arguments[..77]) } else { arguments.clone() })
            };
            conv.entries.push(ConversationEntry::ToolCall {
                tool_name: tool_name.clone(),
                arg_preview: preview,
            });
        }
        UiEvent::ToolCallComplete { tool_name, result, duration_ms: _ } => {
            let preview = if result.len() > 200 {
                format!("{}...", &result[..197])
            } else {
                result.clone()
            };
            conv.entries.push(ConversationEntry::ToolResult {
                tool_name: tool_name.clone(),
                preview,
                success: true,
            });
        }
        UiEvent::ToolCallError { tool_name, error, duration_ms: _ } => {
            conv.entries.push(ConversationEntry::ToolResult {
                tool_name: tool_name.clone(),
                preview: error.clone(),
                success: false,
            });
        }
        UiEvent::ToolCallSkipped { tool_name, reason, duration_ms: _ } => {
            conv.entries.push(ConversationEntry::ToolResult {
                tool_name: tool_name.clone(),
                preview: reason.clone(),
                success: false,
            });
        }
        UiEvent::MaxIterationsReached { current, max } => {
            conv.entries.push(ConversationEntry::Error {
                message: format!("Max iterations reached ({}/{}) — waiting for user decision...", current, max),
            });
        }
        UiEvent::IterationContinued { from_iteration } => {
            conv.entries.push(ConversationEntry::AgentAnswer {
                text: format!("Continuing from iteration {from_iteration} (counter reset to 0)"),
            });
        }
        UiEvent::IterationComplete { final_answer, .. } => {
            if let Some(answer) = final_answer {
                conv.entries.push(ConversationEntry::AgentAnswer { text: answer.clone() });
            }
        }
        _ => {}
    }
}

#[component]
pub fn ConversationView() -> Element {
    let signal: Signal<ConversationState> = use_context();

    let guard = signal.read();
    let count = guard.active_entries().len();
    let _version = count; // Trigger re-render when count changes

    if count == 0 {
        return rsx! {
            div { class: "flex-1 overflow-y-auto p-1.5 sm:p-2.5 min-h-0",
                div { class: "flex items-center justify-center h-full text-[#666]", "No messages yet. Type a query and press Send." }
            }
        };
    }

    let entries = guard.active_entries().to_vec();
    let messages: Vec<Element> = (0..count).map(|index| {
        let entry = entries[index].clone();
        rsx! { MessageEntry { entry } }
    }).collect();

    // Auto-scroll to bottom when messages change
    use_effect(move || {
        if let Some(window) = web_sys::window() {
            if let Some(document) = window.document() {
                if let Some(el) = document.get_element_by_id("conversation-scroll") {
                    el.set_scroll_top(el.scroll_height());
                }
            }
        }
    });

    rsx! {
        div {
            id: "conversation-scroll",
            class: "flex-1 overflow-y-auto p-1.5 sm:p-2.5 min-h-0",
            {messages.into_iter()}
        }
    }
}

#[component]
pub(crate) fn MessageEntry(entry: ConversationEntry) -> Element {
    match entry {
        ConversationEntry::UserInput { text } => {
            rsx! { div { class: "mb-1.5 sm:mb-2.5 px-1.5 sm:px-2.5 py-1 sm:py-2 rounded-md max-w-full break-words whitespace-pre-wrap bg-[#1a2a44] border-l-[3px] border-[#4080ff]", div { class: "text-[#4080ff] font-bold", ">>> " } {text} } }
        }
        ConversationEntry::Thinking { content } => {
            rsx! { div { class: "mb-1.5 sm:mb-2.5 px-1.5 sm:px-2.5 py-1 sm:py-2 rounded-md max-w-full break-words whitespace-pre-wrap bg-[#2a2a20] border-l-[3px] border-[#c0c040]", div { class: "text-[#c0c040] font-bold", "Thinking" } div { class: "text-[#888] mt-1 pl-1", {content} } } }
        }
        ConversationEntry::LlmCall { iteration, model } => {
            let model_label = if model.is_empty() { format!("Calling LLM (iteration {iteration})...") } else { format!("Calling LLM: {model} (iteration {iteration})") };
            rsx! { div { class: "mb-1.5 sm:mb-2.5 px-1.5 sm:px-2.5 py-1 sm:py-2 rounded-md max-w-full break-words whitespace-pre-wrap bg-[#2a2030] border-l-[3px] border-[#a060c0]", div { class: "text-[#a060c0] font-bold", {model_label} } } }
        }
        ConversationEntry::ContentStreaming { content } => {
            if content.is_empty() { rsx! { div { class: "mb-1.5 sm:mb-2.5 px-1.5 sm:px-2.5 py-1 sm:py-2 rounded-md max-w-full break-words whitespace-pre-wrap text-[#ccc]", "Generating..." } } }
            else { rsx! { div { class: "mb-1.5 sm:mb-2.5 px-1.5 sm:px-2.5 py-1 sm:py-2 rounded-md max-w-full break-words whitespace-pre-wrap text-[#ccc]", {content} } } }
        }
        ConversationEntry::ToolCall { tool_name, arg_preview } => {
            rsx! { div { class: "mb-1.5 sm:mb-2.5 px-1.5 sm:px-2.5 py-1 sm:py-2 rounded-md max-w-full break-words whitespace-pre-wrap bg-[#1a2a3a] border-l-[3px] border-[#4080c0]", div { class: "text-[#4080c0] font-bold", "[{tool_name}]" } if !arg_preview.is_empty() { div { class: "text-[#888] text-[12px] mt-0.5 pl-1", "{arg_preview}" } } } }
        }
        ConversationEntry::ToolResult { tool_name, preview, success } => {
            let cls = if success {
                "mb-1.5 sm:mb-2.5 px-1.5 sm:px-2.5 py-1 sm:py-2 rounded-md max-w-full break-words whitespace-pre-wrap bg-[#1a2a1a] border-l-[3px] border-[#40c040]"
            } else {
                "mb-1.5 sm:mb-2.5 px-1.5 sm:px-2.5 py-1 sm:py-2 rounded-md max-w-full break-words whitespace-pre-wrap bg-[#2a1a1a] border-l-[3px] border-[#c04040]"
            };
            let status = if success { "OK" } else { "ERR" };
            let color = if success { "#40c040" } else { "#c04040" };
            let display = truncate_lines(&preview, 6, 90);
            rsx! { div { class: cls, div { span { class: "font-bold", style: "color: {color};", "[{status}] " } span { style: "color: {color}; font-weight: bold;", "{tool_name}" } } div { class: "text-[#888] text-[12px] mt-1 pl-1 max-h-[120px] overflow-y-auto font-mono", {display} } } }
        }
        ConversationEntry::AgentAnswer { text } => { rsx! { div { class: "mb-1.5 sm:mb-2.5 px-1.5 sm:px-2.5 py-1 sm:py-2 rounded-md max-w-full break-words whitespace-pre-wrap text-[#e0e0e0] leading-[1.5]", {text} } } }
        ConversationEntry::RunSummary { iterations, tool_calls, elapsed_ms } => {
            let iw = if iterations == 1 { "iteration" } else { "iterations" };
            let tw = if tool_calls == 1 { "tool call" } else { "tool calls" };
            rsx! { div { class: "mb-1.5 sm:mb-2.5 px-1.5 sm:px-2.5 py-1 sm:py-2 rounded-md max-w-full break-words whitespace-pre-wrap text-[#80c080] font-bold py-1.5", "Done | {iterations} {iw} | {tool_calls} {tw} | {elapsed_ms}ms" } }
        }
        ConversationEntry::Error { message } => { rsx! { div { class: "mb-1.5 sm:mb-2.5 px-1.5 sm:px-2.5 py-1 sm:py-2 rounded-md max-w-full break-words whitespace-pre-wrap text-[#ff6060] font-bold bg-[#2a1a1a] border-l-[3px] border-[#c04040]", "Error: {message}" } } }
        ConversationEntry::EntryCheckpoint { reason, note, created_at } => {
            let note_text = note.as_deref().map(|n| format!(" ({n})")).unwrap_or_default();
            rsx! { div { class: "mb-1.5 sm:mb-2.5 px-1.5 sm:px-2.5 py-1 sm:py-2 rounded-md max-w-full break-words whitespace-pre-wrap bg-[#2a2a20] border-l-[3px] border-[#c0a040] text-[#aaa] text-[12px] italic", "[Checkpoint {created_at}] {reason}{note_text}" } }
        }
    }
}
