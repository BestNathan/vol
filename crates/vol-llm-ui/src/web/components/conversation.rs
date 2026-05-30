//! Conversation view showing all message types.

use dioxus::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::HtmlElement;

use crate::state::{
    ConversationEntry, ConversationState, GlobalState, UiEvent,
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
            conv.entries.push(ConversationEntry::UserInput { text: input.clone() });
        }
        UiEvent::AgentComplete { .. } => {
            flush_pending_content(&mut conv.entries);
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
        UiEvent::IterationComplete { final_answer: _, .. } => {
            // Content already rendered via ContentComplete stream;
            // do not push duplicate AgentAnswer.
        }
        _ => {}
    }
}

#[component]
pub fn ConversationView() -> Element {
    let signal: Signal<ConversationState> = use_context();
    let global: Signal<GlobalState> = use_context();

    let guard = signal.read();
    let count = guard.active_entries().len();
    if count == 0 {
        return rsx! {
            div { class: "flex-1 overflow-y-auto p-1.5 sm:p-2.5 min-h-0",
                div { class: "flex items-center justify-center h-full text-[#666]", "No messages yet. Type a query and press Send." }
            }
        };
    }

    let entries = guard.active_entries().to_vec();
    let is_running = global.read().is_running;
    let messages: Vec<Element> = (0..count).map(|index| {
        let entry = entries[index].clone();
        let is_last = index == count - 1;
        rsx! { TimelineEntry { entry, is_last, is_running } }
    }).collect();

    // Auto-scroll: scroll to bottom after render if auto_scroll is on
    let signal_clone = signal.clone();
    use_effect(move || {
        let auto_scroll = signal_clone.with(|s| {
            s.active_agent.as_ref()
                .and_then(|id| s.agents.get(id))
                .map(|a| a.auto_scroll)
                .unwrap_or(true)
        });
        if auto_scroll {
            let _ = dioxus::document::eval(
                "setTimeout(()=>{const e=document.querySelector('[data-scroll]');if(e)e.scrollTop=e.scrollHeight;},30)"
            );
        }
    });

    // Track scroll position: detect when user scrolls away from bottom
    let signal_scroll = signal.clone();
    let on_scroll = move |_evt: Event<dioxus::prelude::ScrollData>| {
        if let Some(window) = web_sys::window() {
            if let Some(doc) = window.document() {
                if let Some(el) = doc.query_selector("[data-scroll]").ok().flatten() {
                    if let Ok(el) = el.dyn_into::<HtmlElement>() {
                        let at_bottom = el.scroll_top() + el.client_height() >= el.scroll_height() - 50;
                        let mut s = signal_scroll.write_unchecked();
                        let agent_id = s.active_agent.clone().unwrap_or_default();
                        let ac = s.get_or_create(&agent_id);
                        ac.auto_scroll = at_bottom;
                    }
                }
            }
        }
    };

    rsx! {
        div {
            class: "flex-1 overflow-y-auto p-1.5 sm:p-2.5 min-h-0",
            "data-scroll": "1",
            onscroll: on_scroll,
            {messages.into_iter()}
        }
    }
}

#[component]
fn TimelineEntry(entry: ConversationEntry, is_last: bool, is_running: bool) -> Element {
    let is_user = matches!(entry, ConversationEntry::UserInput { .. });

    let dot_class = if is_last && is_running {
        "w-2 h-2 rounded-full bg-white animate-pulse shrink-0"
    } else {
        "w-2 h-2 rounded-full bg-white shrink-0"
    };

    let content = match entry {
        ConversationEntry::UserInput { text } => {
            rsx! { div { class: "text-white", {text} } }
        }
        ConversationEntry::Thinking { content } => {
            rsx! { div { class: "text-[#888] italic text-sm", {content} } }
        }
        ConversationEntry::ContentStreaming { content } => {
            if content.is_empty() {
                rsx! { div { class: "text-[#888]", "Generating..." } }
            } else {
                rsx! { div { class: "text-[#e0e0e0]", {content} } }
            }
        }
        ConversationEntry::ToolCall { tool_name, arg_preview } => {
            rsx! {
                div {
                    span { class: "font-bold", "[{tool_name}]" }
                    if !arg_preview.is_empty() {
                        div { class: "text-[#888] text-xs mt-0.5", "{arg_preview}" }
                    }
                }
            }
        }
        ConversationEntry::ToolResult { tool_name, preview, success } => {
            let status = if success { "OK" } else { "ERR" };
            let color = if success { "#40c040" } else { "#c04040" };
            let display = truncate_lines(&preview, 6, 90);
            rsx! {
                div { class: "ml-4",
                    div { class: "text-xs",
                        span { class: "font-bold", style: "color: {color};", "[{status}] " }
                        span { style: "color: {color};", "{tool_name}" }
                    }
                    div { class: "text-[#888] text-xs mt-0.5 max-h-[120px] overflow-y-auto font-mono", {display} }
                }
            }
        }
        ConversationEntry::AgentAnswer { text } => {
            rsx! { div { class: "text-[#e0e0e0] whitespace-pre-wrap leading-[1.5]", {text} } }
        }
        ConversationEntry::RunSummary { iterations, tool_calls, elapsed_ms } => {
            let iw = if iterations == 1 { "iteration" } else { "iterations" };
            let tw = if tool_calls == 1 { "tool call" } else { "tool calls" };
            rsx! { div { class: "text-[#80c080] font-bold text-center text-sm", "Done | {iterations} {iw} | {tool_calls} {tw} | {elapsed_ms}ms" } }
        }
        ConversationEntry::EntryCheckpoint { reason, note, created_at } => {
            let note_text = note.as_deref().map(|n| format!(" ({n})")).unwrap_or_default();
            rsx! { div { class: "text-[#888] text-xs italic", "[Checkpoint {created_at}] {reason}{note_text}" } }
        }
        ConversationEntry::Error { ref message } => {
            rsx! { div { class: "text-[#ff6060] font-bold", "Error: {message}" } }
        }
    };

    rsx! {
        div { class: "flex gap-3",
            // Dot + line column
            div { class: "flex flex-col items-center w-3 shrink-0 pt-1.5",
                if is_user {
                    div { class: "text-white text-sm leading-none font-bold", "❯" }
                } else {
                    div { class: dot_class }
                }
                if !is_last {
                    div { class: "w-px flex-1 bg-[#333] min-h-[16px]" }
                }
            }
            // Content column
            div { class: "flex-1 pb-3 min-w-0 break-words",
                {content}
            }
        }
    }
}
