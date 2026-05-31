//! Conversation view showing all message types.
//! Tool calls/results show a summary line; clicking opens a detail modal.

use dioxus::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::HtmlElement;

use crate::state::{
    ConversationEntry, ConversationState, GlobalState, UiEvent,
};

/// Data for the tool call detail modal.
#[derive(Debug, Clone, PartialEq)]
struct ToolDetail {
    tool_name: String,
    arguments: String,
    result: Option<String>,
    success: Option<bool>,
}

fn find_tool_detail(entries: &[ConversationEntry], index: usize) -> Option<ToolDetail> {
    match &entries[index] {
        ConversationEntry::ToolCall { tool_name, full_arguments, .. } => {
            let result = entries[index + 1..].iter().find_map(|e| {
                if let ConversationEntry::ToolResult { tool_name: tn, full_result, success, .. } = e {
                    if tn == tool_name { Some((full_result.clone(), *success)) } else { None }
                } else { None }
            });
            Some(ToolDetail {
                tool_name: tool_name.clone(),
                arguments: full_arguments.clone(),
                result: result.as_ref().map(|(r, _)| r.clone()),
                success: result.map(|(_, s)| s),
            })
        }
        ConversationEntry::ToolResult { tool_name, full_result, success, .. } => {
            let arguments = entries[..index].iter().rev().find_map(|e| {
                if let ConversationEntry::ToolCall { tool_name: tn, full_arguments, .. } = e {
                    if tn == tool_name { Some(full_arguments.clone()) } else { None }
                } else { None }
            }).unwrap_or_default();
            Some(ToolDetail {
                tool_name: tool_name.clone(),
                arguments,
                result: Some(full_result.clone()),
                success: Some(*success),
            })
        }
        _ => None,
    }
}

fn flush_pending_content(entries: &mut Vec<ConversationEntry>) {
    if let Some(ConversationEntry::ContentStreaming { content }) = entries.last() {
        let text = content.clone();
        if !text.is_empty() {
            *entries.last_mut().unwrap() = ConversationEntry::AgentAnswer { text };
        }
    }
}

fn clear_running_banner(entries: &mut Vec<ConversationEntry>) {
    entries.retain(|e| !matches!(e, ConversationEntry::RunningBanner { .. }));
}

pub fn reduce_conversation(s: &mut ConversationState, event: &UiEvent) {
    let conv = s.active_mut();
    match event {
        UiEvent::AgentStart { input } => {
            conv.entries.push(ConversationEntry::UserInput { text: input.clone() });
        }
        UiEvent::AgentComplete { .. } => {
            flush_pending_content(&mut conv.entries);
            clear_running_banner(&mut conv.entries);
        }
        UiEvent::AgentAborted { reason } | UiEvent::AgentError { message: reason } => {
            flush_pending_content(&mut conv.entries);
            clear_running_banner(&mut conv.entries);
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
        UiEvent::ThinkingComplete => {}
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
            let preview = crate::state::format_tool_args(arguments);
            conv.entries.push(ConversationEntry::ToolCall {
                tool_name: tool_name.clone(),
                arg_preview: preview,
                full_arguments: arguments.clone(),
            });
        }
        UiEvent::ToolCallArgumentDelta { .. } => {}
        UiEvent::ToolCallComplete { tool_name, result, .. } => {
            let preview = crate::state::truncate_preview(result, 200);
            conv.entries.push(ConversationEntry::ToolResult {
                tool_name: tool_name.clone(),
                preview,
                full_result: result.clone(),
                success: true,
            });
        }
        UiEvent::ToolCallError { tool_name, error, .. } => {
            conv.entries.push(ConversationEntry::ToolResult {
                tool_name: tool_name.clone(),
                preview: error.clone(),
                full_result: error.clone(),
                success: false,
            });
        }
        UiEvent::ToolCallSkipped { tool_name, reason, .. } => {
            conv.entries.push(ConversationEntry::ToolResult {
                tool_name: tool_name.clone(),
                preview: reason.clone(),
                full_result: reason.clone(),
                success: false,
            });
        }
        UiEvent::ApprovalRequest { .. } => {}
        UiEvent::ApprovalResolved { .. } => {}
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
        UiEvent::IterationComplete { .. } => {
            clear_running_banner(&mut conv.entries);
        }
        UiEvent::WsConnected | UiEvent::WsConnecting | UiEvent::WsDisconnected { .. }
        | UiEvent::WsReconnecting { .. } | UiEvent::WsReconnectFailed | UiEvent::WsReconnected => {}
    }
}

#[component]
pub fn ConversationView() -> Element {
    let signal: Signal<ConversationState> = use_context();
    let global: Signal<GlobalState> = use_context();
    let detail = use_signal(|| None::<ToolDetail>);

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
        rsx! { TimelineEntry { entry, is_last, is_running, index, entries: entries.clone(), detail } }
    }).collect();

    // Auto-scroll
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
        if let Some(ref d) = *detail.read() {
            ToolDetailModal { detail: d.clone(), detail_signal: detail }
        }
    }
}

#[component]
fn TimelineEntry(
    entry: ConversationEntry,
    is_last: bool,
    is_running: bool,
    index: usize,
    entries: Vec<ConversationEntry>,
    detail: Signal<Option<ToolDetail>>,
) -> Element {
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
        ConversationEntry::ToolCall { tool_name, ref arg_preview, .. } => {
            let empty = arg_preview.is_empty();
            rsx! {
                div {
                    class: "cursor-pointer hover:bg-[#2a2a44] rounded px-1 -mx-1 py-0.5 select-none group",
                    onclick: {
                        let mut d = detail;
                        let ents = entries.clone();
                        move |_| { d.set(find_tool_detail(&ents, index)); }
                    },
                    div { class: "flex items-center gap-1.5 text-xs font-mono",
                        span { class: "font-bold text-[#a0a040] whitespace-nowrap", "[{tool_name}]" }
                        if !empty {
                            span { class: "text-[#aaa] truncate", "{arg_preview}" }
                        }
                        span { class: "text-[#666] text-[10px] opacity-0 group-hover:opacity-100 whitespace-nowrap ml-auto", "\u{67e5}\u{770b}\u{00bb}" }
                    }
                }
            }
        }
        ConversationEntry::ToolResult { tool_name, ref preview, success, .. } => {
            let status = if success { "OK" } else { "ERR" };
            let color = if success { "#40c040" } else { "#c04040" };
            let bg = if success { "bg-[#1a2a1a]" } else { "bg-[#2a1a1a]" };
            rsx! {
                div {
                    class: "ml-4 cursor-pointer hover:brightness-125 rounded px-1.5 py-1 select-none group {bg}",
                    onclick: {
                        let mut d = detail;
                        let ents = entries.clone();
                        move |_| { d.set(find_tool_detail(&ents, index)); }
                    },
                    div { class: "flex items-center gap-1.5 text-xs",
                        span { class: "font-bold whitespace-nowrap", style: "color: {color};", "[{status}]" }
                        span { class: "text-[#aaa] whitespace-nowrap", "{tool_name}" }
                        span { class: "text-[#666] text-[10px] opacity-0 group-hover:opacity-100 ml-auto whitespace-nowrap", "\u{67e5}\u{770b}\u{00bb}" }
                    }
                    div { class: "text-[#888] text-xs mt-0.5 font-mono line-clamp-2 overflow-hidden", "{preview}" }
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
        ConversationEntry::RunningBanner { ref run_id } => {
            rsx! {
                div { class: "flex items-center gap-3 px-3 py-3 mb-2 bg-[#1a2a44] border border-[#3a5a7a] rounded-md text-sm",
                    div { class: "w-2.5 h-2.5 rounded-full bg-[#40c040] animate-pulse shrink-0" }
                    div { class: "flex flex-col gap-0.5",
                        span { class: "text-[#c0d0e0] font-semibold", "Agent is currently running" }
                        span { class: "text-[#888] text-xs font-mono", "run_id: {run_id}" }
                        span { class: "text-[#666] text-xs", "Below is the live conversation." }
                    }
                }
            }
        }
    };

    rsx! {
        div { class: "flex gap-3",
            div { class: "flex flex-col items-center w-3 shrink-0 pt-1.5",
                if is_user {
                    div { class: "text-white text-sm leading-none font-bold", "\u{2776}" }
                } else {
                    div { class: dot_class }
                }
                if !is_last {
                    div { class: "w-px flex-1 bg-[#333] min-h-[16px]" }
                }
            }
            div { class: "flex-1 pb-3 min-w-0 break-words",
                {content}
            }
        }
    }
}

/// Modal overlay for tool call details.
#[component]
fn ToolDetailModal(detail: ToolDetail, detail_signal: Signal<Option<ToolDetail>>) -> Element {
    let args_display = format_json_pretty(&detail.arguments);
    let result_display = detail.result.as_deref().map(format_json_pretty);
    let status_badge: Option<(String, String)> = detail.success.map(|ok| {
        if ok { ("OK".to_string(), "#40c040".to_string()) } else { ("ERR".to_string(), "#c04040".to_string()) }
    });

    rsx! {
        div {
            class: "fixed inset-0 bg-black/50 z-50 flex items-center justify-center p-4",
            onclick: {
                let mut d = detail_signal;
                move |_| d.set(None)
            },
            div {
                class: "bg-[#1a1a2e] border border-[#444] rounded-lg w-full max-w-[640px] max-h-[80vh] flex flex-col shadow-2xl",
                onclick: move |e: Event<MouseData>| e.stop_propagation(),
                div { class: "flex items-center justify-between px-4 py-3 border-b border-[#333] shrink-0",
                    div { class: "flex items-center gap-2",
                        span { class: "font-bold text-[#c0c040] text-sm font-mono", "[{detail.tool_name}]" }
                        if let Some((ref label, ref color)) = status_badge {
                            span {
                                class: "text-[10px] px-1.5 py-0.5 rounded font-bold",
                                style: "color: {color}; background: {color}22;",
                                "{label}"
                            }
                        }
                    }
                    button {
                        class: "text-[#888] hover:text-white text-lg leading-none px-1",
                        onclick: { let mut d = detail_signal; move |_| d.set(None) },
                        "\u{00d7}"
                    }
                }
                div { class: "flex-1 overflow-y-auto px-4 py-3 space-y-3 text-sm",
                    div {
                        div { class: "text-[#888] text-xs mb-1 font-bold", "Arguments" }
                        div { class: "text-[#ccc] font-mono text-xs bg-[#111128] rounded p-2.5 whitespace-pre-wrap break-all max-h-[200px] overflow-y-auto",
                            "{args_display}"
                        }
                    }
                    if let Some(ref res) = result_display {
                        div {
                            div { class: "text-[#888] text-xs mb-1 font-bold", "Result" }
                            div { class: "text-[#ccc] font-mono text-xs bg-[#111128] rounded p-2.5 whitespace-pre-wrap break-all max-h-[240px] overflow-y-auto",
                                "{res}"
                            }
                        }
                    } else if detail.result.is_none() && detail.success.is_none() {
                        div { class: "text-[#888] text-xs italic", "Waiting for result..." }
                    }
                }
            }
        }
    }
}

fn format_json_pretty(raw: &str) -> String {
    if raw.is_empty() { return String::new(); }
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(raw) {
        serde_json::to_string_pretty(&val).unwrap_or_else(|_| raw.to_string())
    } else {
        raw.to_string()
    }
}
