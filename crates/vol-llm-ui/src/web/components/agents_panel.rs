//! Agents panel — card grid, sub-tabs, embedded conversation/sessions.

use dioxus::prelude::*;

use crate::state::{AgentsState, AgentSubTab, ConversationState, UiEventKind};

use super::conversation::ConversationView;
use super::input_area::InputArea;
use super::sessions_panel::SessionsPanel;

/// Agent card — responsive, works on mobile.
#[component]
fn AgentCard(
    agent: crate::web::client::AgentListEntry,
    is_selected: bool,
    on_click: EventHandler<()>,
) -> Element {
    let scope_color = match agent.scope.as_str() {
        "repo" => "#4080ff",
        "user" => "#40c040",
        _ => "#888",
    };

    let card_class = if is_selected {
        "flex items-center gap-2 px-2.5 py-2 rounded-md cursor-pointer border border-[#80a0ff] bg-[#1a2a44] w-full sm:w-auto"
    } else {
        "flex items-center gap-2 px-2.5 py-2 rounded-md cursor-pointer border border-[#2a2a44] bg-[#1e1e36] hover:bg-[#222240] w-full sm:w-auto"
    };

    rsx! {
        div { class: "{card_class}", onclick: move |_| on_click.call(()),
            div { class: "w-2 h-2 rounded-full bg-[#40c040] flex-shrink-0" }
            div { class: "flex flex-col min-w-0 flex-1",
                div { class: "flex items-center gap-1.5",
                    span { class: "font-semibold text-[13px] text-[#e0e0e0] truncate", "{agent.name}" }
                    span {
                        class: "text-[9px] px-1 py-0.5 rounded-[2px] font-bold whitespace-nowrap flex-shrink-0",
                        style: "background: {scope_color}; color: #1a1a2e;",
                        "{agent.scope}"
                    }
                }
                span { class: "text-[11px] text-[#666] truncate", "{agent.description}" }
            }
        }
    }
}

#[component]
fn SubTabButton(label: String, active: bool, onclick: EventHandler<()>) -> Element {
    let cls = if active {
        "px-3 py-1.5 text-[12px] font-semibold cursor-pointer bg-[#1a1a2e] text-[#e0e0e0] border-b-2 border-[#80a0ff]"
    } else {
        "px-3 py-1.5 text-[12px] cursor-pointer text-[#888] hover:text-[#ccc] hover:bg-[#2a2a44] border-b-2 border-transparent"
    };
    rsx! { button { class: "{cls}", onclick: move |_| onclick.call(()), "{label}" } }
}

#[component]
pub fn AgentsPanel() -> Element {
    let app: crate::web::components::app::AppState = use_context();
    let agents_signal: Signal<AgentsState> = use_context();
    let conv_signal: Signal<ConversationState> = use_context();

    // Load agents helper
    let rpc_load = app.rpc_client.clone();
    let sig_load = agents_signal;

    // Initial load on mount
    use_hook(move || {
        let mut sig = sig_load;
        sig.with_mut(|s| { s.loading = true; s.error = None; });
        let sig2 = sig_load;
        rpc_load.agent_list(move |result| {
            let mut sig2 = sig2;
            sig2.with_mut(|s| {
                s.loading = false;
                match result {
                    Ok(agents) => { s.agents = agents; s.error = None; }
                    Err(e) => { s.error = Some(e); }
                }
            });
        });
    });

    // Retry on WS reconnect
    let rpc_retry = app.rpc_client.clone();
    let sig_retry = agents_signal;
    use_hook(move || {
        let _sub = app.event_bus.subscribe(UiEventKind::WsConnected, move |_| {
            let mut sig = sig_retry;
            sig.with_mut(|s| { s.loading = true; s.error = None; });
            let sig2 = sig_retry;
            rpc_retry.agent_list(move |result| {
                let mut sig2 = sig2;
                sig2.with_mut(|s| {
                    s.loading = false;
                    match result {
                        Ok(agents) => { s.agents = agents; s.error = None; }
                        Err(e) => { s.error = Some(e); }
                    }
                });
            });
        });
    });

    let agents = agents_signal.read().agents.clone();
    let loading = agents_signal.read().loading;
    let error = agents_signal.read().error.clone();
    let selected = agents_signal.read().selected.clone();
    let sub_tab = agents_signal.read().sub_tab;

    // Empty + error = backend not available
    if agents.is_empty() && error.is_some() {
        let err = error.as_deref().unwrap_or("unknown");
        let rpc_btn = app.rpc_client.clone();
        let sig_btn = agents_signal;
        return rsx! { div { class: "flex-1 overflow-y-auto p-3",
            div { class: "flex flex-col items-center justify-center h-full gap-3 text-center",
                div { class: "text-[#ff6060] text-[14px]", "Failed to load agents" }
                div { class: "text-[#888] text-[12px] max-w-[300px] break-words", "{err}" }
                button {
                    class: "px-4 py-1.5 bg-[#3a3a55] text-[#ccc] rounded text-[13px] hover:bg-[#4a4a65]",
                    onclick: move |_| {
                        let mut sig = sig_btn;
                        sig.with_mut(|s| { s.loading = true; s.error = None; });
                        let sig2 = sig_btn;
                        rpc_btn.agent_list(move |result| {
                            let mut sig2 = sig2;
                            sig2.with_mut(|s| {
                                s.loading = false;
                                match result {
                                    Ok(agents) => { s.agents = agents; s.error = None; }
                                    Err(e) => { s.error = Some(e); }
                                }
                            });
                        });
                    },
                    "Retry"
                }
            }
        }};
    }

    // Loading with no cached data
    if loading && agents.is_empty() {
        return rsx! { div { class: "flex-1 overflow-y-auto p-2",
            div { class: "flex items-center justify-center h-full text-[#666] text-[14px]", "Loading agents..." }
        }};
    }

    // Empty (loaded successfully but no agents registered)
    if agents.is_empty() {
        let rpc_btn = app.rpc_client.clone();
        let sig_btn = agents_signal;
        return rsx! { div { class: "flex-1 overflow-y-auto p-3",
            div { class: "flex flex-col items-center justify-center h-full gap-3 text-center",
                div { class: "text-[#888] text-[14px]", "No agents discovered" }
                div { class: "text-[#666] text-[12px] max-w-[300px]",
                    "Place agent .md files in .agents/agents/ and restart the backend."
                }
                button {
                    class: "px-4 py-1.5 bg-[#3a3a55] text-[#ccc] rounded text-[13px] hover:bg-[#4a4a65]",
                    onclick: move |_| {
                        let mut sig = sig_btn;
                        sig.with_mut(|s| { s.loading = true; s.error = None; });
                        let sig2 = sig_btn;
                        rpc_btn.agent_list(move |result| {
                            let mut sig2 = sig2;
                            sig2.with_mut(|s| {
                                s.loading = false;
                                match result {
                                    Ok(agents) => { s.agents = agents; s.error = None; }
                                    Err(e) => { s.error = Some(e); }
                                }
                            });
                        });
                    },
                    "Refresh"
                }
            }
        }};
    }

    let selected_agent = agents.iter().find(|a| selected.as_ref() == Some(&a.id));

    rsx! {
        div { class: "flex flex-col flex-1 min-h-0 overflow-hidden",
            // Card grid — responsive: stack on mobile, wrap on desktop
            div { class: "flex flex-col sm:flex-row sm:flex-wrap gap-2 p-2 border-b border-[#333355] overflow-y-auto max-h-[200px] min-h-[60px] flex-shrink-0",
                for agent in &agents {
                    AgentCard {
                        key: "{agent.id}",
                        agent: agent.clone(),
                        is_selected: selected.as_ref() == Some(&agent.id),
                        on_click: {
                            let mut sig = agents_signal;
                            let mut conv_sig = conv_signal;
                            let agent_id = agent.id.clone();
                            let is_selected = selected.as_ref() == Some(&agent.id);
                            move |_: ()| {
                                sig.with_mut(|s| {
                                    if is_selected { s.selected = None; }
                                    else {
                                        s.selected = Some(agent_id.clone());
                                        s.sub_tab = AgentSubTab::Conversation;
                                    }
                                });
                                conv_sig.with_mut(|cs| {
                                    cs.set_active(if is_selected { None } else { Some(agent_id.clone()) });
                                });
                            }
                        },
                    }
                }
            }

            // Info bar
            if let Some(agent) = selected_agent {
                div { class: "flex items-center gap-2 px-3 py-1.5 bg-[#1a2a44] border-b border-[#333355]",
                    span { class: "font-bold text-[13px] text-[#e0e0e0] truncate", "{agent.name}" }
                    span { class: "text-[12px] text-[#888] truncate hidden sm:inline", "{agent.description}" }
                }
            }

            // Sub-tabs + content
            if selected.is_some() {
                div { class: "flex border-b border-[#333355] bg-[#252540] flex-shrink-0",
                    SubTabButton {
                        label: "Conversation".to_string(),
                        active: sub_tab == AgentSubTab::Conversation,
                        onclick: {
                            let mut sig = agents_signal;
                            move |_: ()| { sig.with_mut(|s| s.sub_tab = AgentSubTab::Conversation); }
                        },
                    }
                    SubTabButton {
                        label: "Sessions".to_string(),
                        active: sub_tab == AgentSubTab::Sessions,
                        onclick: {
                            let mut sig = agents_signal;
                            move |_: ()| { sig.with_mut(|s| s.sub_tab = AgentSubTab::Sessions); }
                        },
                    }
                }
                div { class: "flex-1 min-h-0 flex flex-col overflow-hidden",
                    match sub_tab {
                        AgentSubTab::Conversation => rsx! {
                            ConversationView {}
                            InputArea {}
                        },
                        AgentSubTab::Sessions => rsx! {
                            SessionsPanel {}
                        },
                    }
                }
            } else {
                div { class: "flex-1 flex items-center justify-center text-[#666] text-[14px]",
                    "Select an agent to start"
                }
            }
        }
    }
}
