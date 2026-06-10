//! Context panel -- contributor list with metadata, click to open snapshot dialog.

use dioxus::prelude::*;

use crate::state::{ContextMessageEntry, ContextState};

/// Anchor zone color tag.
fn anchor_badge(zone: &str) -> &'static str {
    match zone {
        "head" => "#4080ff",
        "middle" => "#c0a040",
        "tail" => "#40c040",
        _ => "#888",
    }
}

/// Role color tag.
fn role_color(role: &str) -> &'static str {
    match role {
        "system" => "#c080ff",
        "user" => "#4080ff",
        "assistant" => "#40c040",
        "tool" => "#c0a040",
        _ => "#888",
    }
}

/// Modal dialog showing contributor message snapshot.
#[component]
fn ContextDialog(
    contributor_name: String,
    messages: Vec<ContextMessageEntry>,
    loading: bool,
    on_close: EventHandler<()>,
) -> Element {
    rsx! {
        div {
            class: "fixed inset-0 bg-black/50 flex items-center justify-center z-50",
            onclick: move |_| on_close.call(()),
            div {
                class: "w-[95vw] sm:w-[700px] max-h-[80vh] flex flex-col overflow-hidden bg-[#1a1a2e] border border-[#3a3a55] rounded-lg",
                onclick: move |evt: Event<MouseData>| { evt.stop_propagation(); },
                // Header
                div { class: "flex items-center justify-between flex-shrink-0 px-4 pt-3 pb-2 border-b border-[#3a3a55]",
                    span { class: "text-[15px] font-semibold text-[#e0e0e0] truncate", "{contributor_name}" }
                    button {
                        class: "text-[#888] hover:text-[#e0e0e0] text-[18px] flex-shrink-0 ml-2",
                        onclick: move |_| on_close.call(()),
                        "x"
                    }
                }
                // Content
                div { class: "flex-1 min-h-0 overflow-y-auto px-4 pb-4",
                    if loading {
                        div { class: "text-[#888] text-[13px] py-4 text-center", "Loading..." }
                    } else if messages.is_empty() {
                        div { class: "text-[#666] text-[13px] py-4 text-center", "No messages" }
                    } else {
                        for msg in &messages {
                            div { class: "mb-3",
                                div { class: "flex items-center gap-2 mb-1",
                                    span {
                                        class: "text-[10px] font-bold uppercase px-1.5 py-0.5 rounded",
                                        style: "color: {role_color(&msg.role)}; background: #2a2a44;",
                                        "{msg.role}"
                                    }
                                }
                                div { class: "bg-[#12121e] border border-[#2a2a44] rounded p-2 max-h-[300px] overflow-y-auto",
                                    pre { class: "text-[12px] text-[#ccc] font-mono whitespace-pre-wrap break-words",
                                        "{msg.content}"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Context tab content.
#[component]
pub fn ContextPanel() -> Element {
    let app: crate::web::components::app::AppState = use_context();
    let agents_signal: Signal<crate::state::AgentsState> = use_context();

    let mut ctx_state = use_signal(ContextState::new);

    let rpc_client = app.rpc_client.clone();
    let rpc_client_effect = rpc_client.clone();

    // Load contributors when selected agent changes
    use_effect(move || {
        let selected = agents_signal.read().selected.clone();
        if let Some(ref agent_id) = selected {
            let client = rpc_client_effect.clone();
            let aid = agent_id.clone();
            let mut sig = ctx_state;
            sig.with_mut(|s| {
                s.loading = true;
                s.error = None;
            });
            wasm_bindgen_futures::spawn_local(async move {
                let (tx, rx) = futures_channel::oneshot::channel();
                client.agent_context_config(&aid, move |result| {
                    let _ = tx.send(result);
                });
                match rx.await {
                    Ok(Ok(contributors)) => {
                        sig.with_mut(|s| {
                            s.contributors = contributors;
                            s.loading = false;
                        });
                    }
                    Ok(Err(e)) => {
                        sig.with_mut(|s| {
                            s.error = Some(e);
                            s.loading = false;
                        });
                    }
                    Err(_) => {
                        sig.with_mut(|s| {
                            s.error = Some("request dropped".to_string());
                            s.loading = false;
                        });
                    }
                }
            });
        }
    });

    if agents_signal.read().selected.is_none() {
        return rsx! {
            div { class: "flex-1 flex items-center justify-center text-[#666] text-[14px]",
                "Select an agent to view context"
            }
        };
    }

    let loading = ctx_state.read().loading;
    let error = ctx_state.read().error.clone();
    let contributors = ctx_state.read().contributors.clone();

    let dialog_open = ctx_state.read().dialog_open;
    let dialog_name = ctx_state.read().dialog_contributor_name.clone();
    let dialog_messages = ctx_state.read().dialog_messages.clone();
    let dialog_loading = ctx_state.read().dialog_loading;

    rsx! {
        div { class: "flex-1 min-h-0 flex flex-col overflow-hidden",
            if loading {
                div { class: "flex items-center justify-center h-full text-[#666] text-[14px]",
                    "Loading contributors..."
                }
            } else if let Some(ref err) = error {
                div { class: "flex items-center justify-center h-full text-[#ff6060] text-[14px] text-center px-4",
                    "{err}"
                }
            } else if contributors.is_empty() {
                div { class: "flex items-center justify-center h-full text-[#888] text-[14px]",
                    "No contributors configured"
                }
            } else {
                div { class: "flex-1 overflow-y-auto",
                    for contributor in &contributors {
                        {
                            let name = contributor.name.clone();
                            let agent_id = agents_signal.read().selected.clone().unwrap_or_default();
                            let client = rpc_client.clone();
                            let sig = ctx_state;
                            rsx! {
                                div {
                                    key: "{contributor.name}",
                                    class: "flex items-center gap-3 px-3 py-2 border-b border-[#2a2a44] cursor-pointer hover:bg-[#2a2a44]",
                                    onclick: move |_| {
                                        let name = name.clone();
                                        let aid = agent_id.clone();
                                        let client = client.clone();
                                        let mut sig = sig;
                                        sig.with_mut(|s| {
                                            s.dialog_open = true;
                                            s.dialog_contributor_name = name.clone();
                                            s.dialog_messages = Vec::new();
                                            s.dialog_loading = true;
                                        });
                                        wasm_bindgen_futures::spawn_local(async move {
                                            let (tx, rx) = futures_channel::oneshot::channel();
                                            client.agent_context_snapshot(&aid, &name, move |result| {
                                                let _ = tx.send(result);
                                            });
                                            match rx.await {
                                                Ok(Ok(msgs)) => {
                                                    sig.with_mut(|s| {
                                                        s.dialog_messages = msgs;
                                                        s.dialog_loading = false;
                                                    });
                                                }
                                                Ok(Err(e)) => {
                                                    sig.with_mut(|s| {
                                                        s.dialog_messages = vec![ContextMessageEntry {
                                                            role: "error".to_string(),
                                                            content: format!("Failed to load snapshot: {}", e),
                                                        }];
                                                        s.dialog_loading = false;
                                                    });
                                                }
                                                Err(_) => {
                                                    sig.with_mut(|s| {
                                                        s.dialog_messages = vec![ContextMessageEntry {
                                                            role: "error".to_string(),
                                                            content: "Request dropped".to_string(),
                                                        }];
                                                        s.dialog_loading = false;
                                                    });
                                                }
                                            }
                                        });
                                    },
                                    // Anchor zone badge
                                    span {
                                        class: "text-[9px] font-bold px-1.5 py-0.5 rounded flex-shrink-0",
                                        style: "color: {anchor_badge(&contributor.anchor_zone)}; background: #2a2a44;",
                                        "{contributor.anchor_zone}"
                                    }
                                    // Name
                                    span { class: "font-semibold text-[13px] text-[#e0e0e0] flex-1 min-w-0 truncate",
                                        "{contributor.name}"
                                    }
                                    // Tokens
                                    span { class: "text-[11px] text-[#888] flex-shrink-0",
                                        "{contributor.estimated_tokens} tokens"
                                    }
                                    // Message count
                                    span { class: "text-[11px] text-[#666] flex-shrink-0",
                                        "{contributor.message_count} msg"
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Dialog
            if dialog_open {
                ContextDialog {
                    contributor_name: dialog_name,
                    messages: dialog_messages,
                    loading: dialog_loading,
                    on_close: move |_| {
                        ctx_state.with_mut(|s| {
                            s.dialog_open = false;
                            s.dialog_contributor_name = String::new();
                            s.dialog_messages = Vec::new();
                        });
                    },
                }
            }
        }
    }
}
