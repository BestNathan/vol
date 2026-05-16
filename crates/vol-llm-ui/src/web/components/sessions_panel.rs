//! Sessions panel listing all persisted sessions. View opens an overlay,
//! resume swaps the agent session — only resume modifies the conversation.

use dioxus::prelude::*;

use crate::state::{ActiveTab, ConversationEntry, ConversationState, SessionsState};
use crate::web::client::{JsonRpcClient, SessionEntry};

fn truncate_for_log(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

/// Convert raw session entries to ConversationEntry for display.
fn session_entries_to_conversation(entries: Vec<SessionEntry>) -> Vec<ConversationEntry> {
    entries.into_iter().filter_map(|e| {
        let entry_type = e.entry_type.clone();
        let data_debug = serde_json::to_string(&e.data).unwrap_or_default();
        let result = match e.entry_type.as_str() {
            "message" => {
                let data = &e.data;
                // data.message is SessionMessage wrapper
                if let Some(session_msg) = data.get("message").and_then(|m| m.get("message")) {
                    // session_msg.message is the actual vol_llm_core::Message with role/content
                    if let Some(msg) = session_msg.get("message") {
                        if let Some(role) = msg.get("role").and_then(|v| v.as_str()) {
                            let text = match msg.get("content") {
                                Some(serde_json::Value::String(s)) => s.clone(),
                                Some(serde_json::Value::Array(parts)) => {
                                    parts.iter().filter_map(|p| {
                                        p.get("text").and_then(|v| v.as_str())
                                            .or_else(|| p.get("type").and_then(|v| v.as_str()))
                                    }).collect::<Vec<_>>().join("\n")
                                }
                                other => {
                                    log::warn!("session entry message content unexpected type: {:?}", other);
                                    String::new()
                                }
                            };
                            match role {
                                "user" => Some(ConversationEntry::UserInput { text }),
                                "assistant" => Some(ConversationEntry::AgentAnswer { text }),
                                "tool" => {
                                    let tool_name = msg.get("name").and_then(|v| v.as_str()).unwrap_or("tool").to_string();
                                    Some(ConversationEntry::ToolResult {
                                        tool_name,
                                        preview: text,
                                        success: true,
                                    })
                                }
                                _ => {
                                    log::warn!("session entry unknown role: {role}");
                                    None
                                }
                            }
                        } else {
                            log::warn!("session entry message missing role, data: {}", truncate_for_log(&data_debug, 200));
                            None
                        }
                    } else {
                        log::warn!("session entry data missing inner message, data: {}", truncate_for_log(&data_debug, 200));
                        None
                    }
                } else {
                    log::warn!("session entry data missing message wrapper, data: {}", truncate_for_log(&data_debug, 200));
                    None
                }
            }
            "checkpoint" => {
                let reason = e.data.get("checkpoint")
                    .and_then(|c| c.get("reason"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("Checkpoint")
                    .to_string();
                let note = e.data.get("checkpoint")
                    .and_then(|c| c.get("note"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                Some(ConversationEntry::EntryCheckpoint { reason, note, created_at: e.created_at })
            }
            "summary" => {
                Some(ConversationEntry::RunSummary {
                    iterations: 0,
                    tool_calls: 0,
                    elapsed_ms: 0,
                })
            }
            _ => {
                log::warn!("session entry unknown entry_type: {entry_type}, data: {}", truncate_for_log(&data_debug, 100));
                None
            }
        };
        if result.is_none() {
            log::warn!("session entry dropped: type={entry_type}, data_preview={}", truncate_for_log(&data_debug, 100));
        }
        result
    }).collect()
}

/// Format a Unix timestamp as a human-readable age label.
fn format_age(ts: i64) -> String {
    let now = web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let diff = (now - ts).max(0);
    if diff < 60 {
        format!("{diff}s ago")
    } else if diff < 3600 {
        format!("{}m ago", diff / 60)
    } else if diff < 86400 {
        format!("{}h ago", diff / 3600)
    } else {
        format!("{}d ago", diff / 86400)
    }
}

/// Truncate a session ID for display.
fn truncate_id(id: &str) -> String {
    if id.len() > 12 {
        format!("{}...", &id[..12])
    } else {
        id.to_string()
    }
}

/// Overlay that renders session entries in a modal dialog.
#[component]
fn SessionDetailOverlay(
    session_id: String,
    entries: Signal<Vec<ConversationEntry>>,
    loading: Signal<bool>,
    show: Signal<bool>,
    had_parse_failure: Signal<bool>,
) -> Element {
    if !*show.read() {
        return VNode::empty();
    }

    let is_loading = *loading.read();
    let has_error = *had_parse_failure.read();
    let items: Vec<Element> = entries.read().iter().map(|entry| {
        let e = entry.clone();
        rsx! {
            match e {
                ConversationEntry::UserInput { text } => rsx! {
                    div { class: "mb-2.5 px-2.5 py-2 rounded-md max-w-full break-words whitespace-pre-wrap bg-[#1a2a44] border-l-[3px] border-[#4080ff]",
                        div { class: "text-[#4080ff] font-bold", ">>> " }
                        {text}
                    }
                },
                ConversationEntry::AgentAnswer { text } => rsx! {
                    div { class: "mb-2.5 px-2.5 py-2 rounded-md max-w-full break-words whitespace-pre-wrap text-[#e0e0e0] leading-[1.5]", {text} }
                },
                ConversationEntry::ToolResult { tool_name, preview, success } => {
                    let cls = if success {
                        "mb-2.5 px-2.5 py-2 rounded-md max-w-full break-words whitespace-pre-wrap bg-[#1a2a1a] border-l-[3px] border-[#40c040]"
                    } else {
                        "mb-2.5 px-2.5 py-2 rounded-md max-w-full break-words whitespace-pre-wrap bg-[#2a1a1a] border-l-[3px] border-[#c04040]"
                    };
                    let status = if success { "OK" } else { "ERR" };
                    let color = if success { "#40c040" } else { "#c04040" };
                    rsx! {
                        div { class: "{cls}",
                            div {
                                span { class: "font-bold", style: "color: {color};", "[{status}] " }
                                span { style: "color: {color}; font-weight: bold;", "{tool_name}" }
                            }
                            div { class: "text-[#888] text-[12px] mt-1 pl-1 max-h-[120px] overflow-y-auto font-mono", "{truncate_lines(&preview, 6, 90)}" }
                        }
                    }
                }
                ConversationEntry::EntryCheckpoint { reason, note, .. } => {
                    let note_text = note.as_deref().map(|n| format!(" ({n})")).unwrap_or_default();
                    rsx! { div { class: "mb-2.5 px-2.5 py-2 rounded-md max-w-full break-words whitespace-pre-wrap bg-[#2a2a20] border-l-[3px] border-[#c0a040] text-[#aaa] text-[12px] italic", "[Checkpoint] {reason}{note_text}" } }
                }
                ConversationEntry::RunSummary { iterations, tool_calls, elapsed_ms } => {
                    let iw = if iterations == 1 { "iteration" } else { "iterations" };
                    let tw = if tool_calls == 1 { "tool call" } else { "tool calls" };
                    rsx! { div { class: "mb-2.5 px-2.5 py-2 rounded-md max-w-full break-words whitespace-pre-wrap text-[#80c080] font-bold py-1.5", "Done | {iterations} {iw} | {tool_calls} {tw} | {elapsed_ms}ms" } }
                }
                _ => rsx! { div { class: "mb-2.5 px-2.5 py-2 rounded-md max-w-full break-words whitespace-pre-wrap", "Entry" } },
            }
        }
    }).collect();

    rsx! {
        div {
            class: "fixed inset-0 bg-black/70 z-[200] flex items-center justify-center",
            onclick: move |_: Event<MouseData>| { show.set(false); },
            div {
                class: "bg-[#1a1a2e] border border-[#333355] rounded-lg w-[80vw] max-w-[900px] h-[70vh] flex flex-col overflow-hidden",
                onclick: move |evt: Event<MouseData>| { evt.stop_propagation(); },
                div { class: "flex items-center justify-between px-3 py-2 border-b border-[#2a2a44] font-mono text-[13px] text-[#e0e0e0]",
                    span { "Session: {session_id}" }
                    button {
                        class: "bg-none border-none text-[#888] text-[16px] cursor-pointer px-1.5 py-0.5 rounded-[3px] hover:text-[#ff6060] hover:bg-[#2a1a1a]",
                        onclick: move |_: Event<MouseData>| { show.set(false); },
                        "x"
                    }
                }
                if is_loading {
                    div { class: "flex items-center justify-center flex-1 text-[#666]", "Loading..." }
                } else if has_error && entries.read().is_empty() {
                    div { class: "flex-1 flex items-center justify-center flex-col text-[#ff6060] p-5 text-center",
                        div { class: "text-[14px] font-semibold mb-2", "Failed to parse session entries" }
                        div { class: "text-[12px] text-[#888]", "Check browser console (F12) for details" }
                    }
                } else if entries.read().is_empty() {
                    div { class: "flex items-center justify-center flex-1 text-[#666]", "No entries" }
                } else {
                    div { class: "flex-1 overflow-y-auto p-2",
                        {items.into_iter()}
                    }
                }
            }
        }
    }
}

fn truncate_lines(s: &str, max_lines: usize, max_chars: usize) -> String {
    let lines: Vec<&str> = s.lines().take(max_lines).collect();
    let result = lines.join("\n");
    if result.chars().count() > max_chars {
        format!("{}...", result.chars().take(max_chars.saturating_sub(3)).collect::<String>())
    } else { result }
}

/// Session item component — click to view in overlay, resume to swap agent session.
#[component]
fn SessionItem(
    session_id: String,
    entry_count: usize,
    created_at: i64,
    rpc: JsonRpcClient,
    conversation_signal: Signal<ConversationState>,
    active_tab: Signal<ActiveTab>,
) -> Element {
    let mut show_detail = use_signal(|| false);
    let entries = use_signal(|| Vec::<ConversationEntry>::new());
    let mut loading = use_signal(|| false);
    let is_resuming = use_signal(|| false);
    let had_parse_failure = use_signal(|| false);

    // Pre-clone for the view onclick handler.
    let rpc_view = rpc.clone();
    let sid_view = session_id.clone();

    // Pre-clone for the resume onclick handler.
    let rpc_resume = rpc.clone();
    let sid_resume = session_id.clone();
    let conv_resume = conversation_signal;
    let tab_resume = active_tab;

    rsx! {
        div {
            class: "flex items-center px-2.5 py-2 border-b border-[#2a2a44] cursor-pointer gap-2 hover:bg-[#222240]",
            onclick: move |_: Event<MouseData>| {
                if entries.read().is_empty() && !*loading.read() {
                    loading.set(true);
                    let rpc = rpc_view.clone();
                    let sid = sid_view.clone();
                    let mut ent = entries;
                    let mut ld = loading;
                    let mut parse_fail = had_parse_failure;
                    rpc.session_entries(&sid, move |result| {
                        match result {
                            Ok(e) => {
                                let converted = session_entries_to_conversation(e.clone());
                                if e.len() > 0 && converted.is_empty() {
                                    parse_fail.set(true);
                                }
                                ent.set(converted);
                            }
                            Err(e) => {
                                log::error!("Failed to load session entries: {e}");
                                parse_fail.set(true);
                            }
                        }
                        ld.set(false);
                    });
                }
                show_detail.set(true);
            },
            span { class: "font-mono text-[13px] text-[#e0e0e0] font-semibold min-w-[80px]", "{truncate_id(&session_id)}" }
            span { class: "text-[11px] text-[#888]", "{entry_count} entries" }
            span { class: "text-[11px] text-[#666] ml-auto", "{format_age(created_at)}" }
            button {
                class: "px-2.5 py-0.5 bg-[#408040] text-[#e0e0e0] border-none rounded-[3px] cursor-pointer text-[12px] ml-1 flex-shrink-0 hover:bg-[#50a050] disabled:bg-[#333355] disabled:cursor-not-allowed",
                disabled: *is_resuming.read(),
                onclick: move |evt: Event<MouseData>| {
                    evt.stop_propagation();
                    let mut resuming = is_resuming;
                    resuming.set(true);
                    let rpc = rpc_resume.clone();
                    let sid = sid_resume.clone();
                    let mut conv = conv_resume;
                    let mut tab = tab_resume;
                    rpc.session_resume(&sid, move |result| {
                        match result {
                            Ok(resp) => {
                                let conv_entries = session_entries_to_conversation(resp.entries);
                                conv.with_mut(|s| { s.entries = conv_entries; });
                                tab.set(ActiveTab::Conversation);
                            }
                            Err(e) => log::error!("Failed to resume session: {e}"),
                        }
                        resuming.set(false);
                    });
                },
                if *is_resuming.read() { "Resuming..." } else { "Resume" }
            }
        }

        SessionDetailOverlay {
            session_id,
            entries,
            loading,
            show: show_detail,
            had_parse_failure,
        }
    }
}

/// Sessions panel component.
#[component]
pub fn SessionsPanel() -> Element {
    let app: super::app::AppState = use_context();
    let sessions_signal: Signal<SessionsState> = use_context();
    let conversation_signal: Signal<ConversationState> = use_context();
    let rpc_for_load = app.rpc_client.clone();
    let rpc_for_items = app.rpc_client.clone();

    // Load sessions on mount
    use_hook(move || {
        let mut sig = sessions_signal;

        sig.with_mut(|s| {
            s.loading = true;
            s.error = None;
        });

        rpc_for_load.session_list(move |result| {
            sig.with_mut(|s| {
                s.loading = false;
                match result {
                    Ok(sessions) => {
                        s.sessions = sessions;
                    }
                    Err(e) => {
                        s.error = Some(e);
                    }
                }
            });
        });
    });

    let (sessions, loading, error) = {
        let s = sessions_signal.read();
        (s.sessions.clone(), s.loading, s.error.clone())
    };

    if loading {
        return rsx! {
            div { class: "flex-1 overflow-y-auto p-2",
                div { class: "flex items-center justify-center h-full text-[#666] p-5 text-center", "Loading sessions..." }
            }
        };
    }

    if let Some(ref e) = error {
        return rsx! {
            div { class: "flex-1 overflow-y-auto p-2",
                div { class: "flex items-center justify-center h-full text-[#ff6060] p-5 text-center", "Error: {e}" }
            }
        };
    }

    if sessions.is_empty() {
        return rsx! {
            div { class: "flex-1 overflow-y-auto p-2",
                div { class: "flex items-center justify-center h-full text-[#666] p-5 text-center", "No sessions found" }
            }
        };
    }

    let items: Vec<Element> = sessions.iter().map(|session| {
        rsx! {
            SessionItem {
                session_id: session.id.clone(),
                entry_count: session.entry_count,
                created_at: session.created_at,
                rpc: rpc_for_items.clone(),
                conversation_signal,
                active_tab: app.active_tab,
            }
        }
    }).collect();

    rsx! {
        div { class: "flex-1 overflow-y-auto p-2",
            div { class: "px-2.5 pt-1 pb-2 text-[12px] font-semibold text-[#888] uppercase tracking-[0.5px]", "Sessions" }
            {items.into_iter()}
        }
    }
}
