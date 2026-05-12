//! Sessions panel listing all persisted sessions with load-on-click into Conversation view.

use dioxus::prelude::*;

use crate::state::{ActiveTab, ConversationEntry, ConversationState, SessionsState};
use crate::web::client::SessionEntry;

/// Convert raw session entries to ConversationEntry for display.
fn session_entries_to_conversation(entries: Vec<SessionEntry>) -> Vec<ConversationEntry> {
    entries.into_iter().filter_map(|e| {
        match e.entry_type.as_str() {
            "message" => {
                let data = &e.data;
                if let Some(msg) = data.get("message") {
                    if let Some(role) = msg.get("role").and_then(|v| v.as_str()) {
                        let text = msg.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
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
                            _ => None,
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            "checkpoint" => {
                let reason = e.data.get("reason")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Checkpoint")
                    .to_string();
                let note = e.data.get("note").and_then(|v| v.as_str()).map(|s| s.to_string());
                Some(ConversationEntry::EntryCheckpoint { reason, note, created_at: e.created_at })
            }
            "summary" => {
                Some(ConversationEntry::RunSummary {
                    iterations: 0,
                    tool_calls: 0,
                    elapsed_ms: 0,
                })
            }
            _ => None,
        }
    }).collect()
}

/// Format a Unix timestamp as a human-readable age label.
fn format_age(ts: i64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
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

/// Session item component with view-on-click and resume button.
#[component]
fn SessionItem(session_id: String, entry_count: usize, created_at: i64) -> Element {
    let app: super::app::AppState = use_context();
    let conversation_signal: Signal<ConversationState> = use_context();
    let active_tab = app.active_tab;
    let mut is_resuming = use_signal(|| false);

    rsx! {
        div {
            class: "session-item",
            onclick: {
                let sid = session_id.clone();
                let rpc = app.rpc_client.clone();
                let tab = active_tab;
                let conv = conversation_signal;
                move |_: Event<MouseData>| {
                    let sid = sid.clone();
                    let rpc = rpc.clone();
                    let mut tab = tab.clone();
                    let mut conv = conv.clone();

                    rpc.session_entries(&sid, move |result| {
                        match result {
                            Ok(entries) => {
                                let conv_entries = session_entries_to_conversation(entries);
                                conv.with_mut(|s| { s.entries = conv_entries; });
                                tab.set(ActiveTab::Conversation);
                            }
                            Err(e) => log::error!("Failed to load session: {}", e),
                        }
                    });
                }
            },
            span { class: "session-item-id", "{truncate_id(&session_id)}" }
            span { class: "session-item-count", "{entry_count} entries" }
            span { class: "session-item-age", "{format_age(created_at)}" }
            button {
                class: "session-resume-btn",
                onclick: {
                    let resuming = is_resuming;
                    let sid = session_id.clone();
                    let rpc = app.rpc_client.clone();
                    let tab = active_tab;
                    let conv = conversation_signal;
                    move |evt: Event<MouseData>| {
                        evt.stop_propagation();
                        let mut resuming = resuming;
                        let sid = sid.clone();
                        resuming.set(true);
                        let rpc = rpc.clone();
                        let mut tab = tab.clone();
                        let mut conv = conv.clone();

                        rpc.session_resume(&sid, move |result| {
                            match result {
                                Ok(resp) => {
                                    let conv_entries = session_entries_to_conversation(resp.entries);
                                    conv.with_mut(|s| { s.entries = conv_entries; });
                                    tab.set(ActiveTab::Conversation);
                                }
                                Err(e) => log::error!("Failed to resume session: {}", e),
                            }
                            resuming.set(false);
                        });
                    }
                },
                if *is_resuming.read() { "Resuming..." } else { "Resume" }
            }
        }
    }
}

/// Sessions panel component.
#[component]
pub fn SessionsPanel() -> Element {
    let app: super::app::AppState = use_context();
    let sessions_signal: Signal<SessionsState> = use_context();
    let _conversation_signal: Signal<ConversationState> = use_context();

    // Load sessions on mount
    use_hook(move || {
        let rpc = app.rpc_client.clone();
        let mut sig = sessions_signal;

        sig.with_mut(|s| {
            s.loading = true;
            s.error = None;
        });

        rpc.session_list(move |result| {
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
            div { class: "sessions-panel",
                div { class: "sessions-panel-loading", "Loading sessions..." }
            }
        };
    }

    if let Some(ref e) = error {
        return rsx! {
            div { class: "sessions-panel",
                div { class: "sessions-panel-error", "Error: {e}" }
            }
        };
    }

    if sessions.is_empty() {
        return rsx! {
            div { class: "sessions-panel",
                div { class: "sessions-panel-empty", "No sessions found" }
            }
        };
    }

    let items: Vec<Element> = sessions.iter().map(|session| {
        rsx! {
            SessionItem {
                session_id: session.id.clone(),
                entry_count: session.entry_count,
                created_at: session.created_at,
            }
        }
    }).collect();

    rsx! {
        div { class: "sessions-panel",
            div { class: "sessions-panel-header", "Sessions" }
            {items.into_iter()}
        }
    }
}
