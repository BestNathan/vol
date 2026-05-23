//! Agents panel v2 — card grid, sub-tabs, embedded conversation/sessions.

use dioxus::prelude::*;

use crate::state::{AgentsState, AgentSubTab};
use crate::web::client::AgentListEntry;

use super::conversation::ConversationView;
use super::sessions_panel::SessionsPanel;

/// Agent card component.
#[component]
fn AgentCard(
    agent: AgentListEntry,
    is_selected: bool,
    on_select: EventHandler<String>,
    on_deselect: EventHandler<()>,
) -> Element {
    let agent_id = agent.id.clone();
    let agent_id2 = agent.id.clone();
    let scope_color = match agent.scope.as_str() {
        "repo" => "#4080ff",
        "user" => "#40c040",
        _ => "#888",
    };

    let card_class = if is_selected {
        "flex items-center gap-2 px-3 py-2 rounded-md cursor-pointer border border-[#80a0ff] bg-[#1a2a44] min-w-[180px] max-w-[260px]"
    } else {
        "flex items-center gap-2 px-3 py-2 rounded-md cursor-pointer border border-[#2a2a44] bg-[#1e1e36] hover:bg-[#222240] min-w-[180px] max-w-[260px]"
    };

    rsx! {
        div {
            class: "{card_class}",
            onclick: move |_| {
                if is_selected {
                    on_deselect.call(());
                } else {
                    on_select.call(agent_id2.clone());
                }
            },
            div { class: "w-2 h-2 rounded-full bg-[#40c040] flex-shrink-0" }
            div { class: "flex flex-col min-w-0",
                div { class: "flex items-center gap-1.5",
                    span { class: "font-semibold text-[13px] text-[#e0e0e0] truncate", "{agent.name}" }
                    span {
                        class: "text-[9px] px-1 py-0.5 rounded-[2px] font-bold whitespace-nowrap",
                        style: "background: {scope_color}; color: #1a1a2e;",
                        "{agent.scope}"
                    }
                }
                span { class: "text-[11px] text-[#666] truncate mt-0.5", "{agent.description}" }
            }
        }
    }
}

/// Sub-tab button for Conversation/Sessions.
#[component]
fn SubTabButton(label: String, active: bool, onclick: EventHandler<()>) -> Element {
    let cls = if active {
        "px-3 py-1.5 text-[12px] font-semibold cursor-pointer bg-[#1a1a2e] text-[#e0e0e0] border-b-2 border-[#80a0ff]"
    } else {
        "px-3 py-1.5 text-[12px] cursor-pointer text-[#888] hover:text-[#ccc] hover:bg-[#2a2a44] border-b-2 border-transparent"
    };
    rsx! {
        button { class: "{cls}", onclick: move |_| onclick.call(()), "{label}" }
    }
}

/// Agents panel component — card grid + sub-tabs.
#[component]
pub fn AgentsPanel() -> Element {
    let app: crate::web::components::app::AppState = use_context();
    let agents_signal: Signal<AgentsState> = use_context();

    // Load agents on mount
    let rpc = app.rpc_client.clone();
    use_hook(move || {
        let mut sig = agents_signal;
        sig.with_mut(|s| { s.loading = true; s.error = None; });
        rpc.agent_list(move |result| {
            let mut sig2 = agents_signal;
            sig2.with_mut(|s| {
                s.loading = false;
                match result {
                    Ok(agents) => { s.agents = agents; }
                    Err(e) => { s.error = Some(e); }
                }
            });
        });
    });

    // Read state
    let agents = agents_signal.read().agents.clone();
    let loading = agents_signal.read().loading;
    let error = agents_signal.read().error.clone();
    let selected = agents_signal.read().selected.clone();
    let sub_tab = agents_signal.read().sub_tab;

    // Loading
    if loading && agents.is_empty() {
        return rsx! { div { class: "flex-1 overflow-y-auto p-2",
            div { class: "flex items-center justify-center h-full text-[#666]", "Loading agents..." }
        }};
    }

    // Error
    if let Some(ref e) = error {
        return rsx! { div { class: "flex-1 overflow-y-auto p-2",
            div { class: "flex items-center justify-center h-full text-[#ff6060] p-5 text-center", "Error: {e}" }
        }};
    }

    // Empty
    if agents.is_empty() {
        return rsx! { div { class: "flex-1 overflow-y-auto p-2",
            div { class: "flex items-center justify-center h-full text-[#666]", "No agents discovered" }
        }};
    }

    let selected_agent = agents.iter().find(|a| selected.as_ref() == Some(&a.id));

    let on_select = {
        let mut sig = agents_signal;
        move |id: String| { sig.with_mut(|s| s.selected = Some(id)); }
    };
    let on_deselect = {
        let mut sig = agents_signal;
        move |_: ()| { sig.with_mut(|s| s.selected = None); }
    };

    rsx! {
        div { class: "flex flex-col h-full",
            // Agent card grid
            div { class: "flex flex-wrap gap-2 p-2 border-b border-[#333355] overflow-y-auto max-h-[180px] min-h-[60px]",
                for agent in &agents {
                    AgentCard {
                        key: "{agent.id}",
                        agent: agent.clone(),
                        is_selected: selected.as_ref() == Some(&agent.id),
                        on_select: on_select.clone(),
                        on_deselect: on_deselect.clone(),
                    }
                }
            }

            // Info bar
            if let Some(agent) = selected_agent {
                div { class: "flex items-center gap-2 px-3 py-1.5 bg-[#1a2a44] border-b border-[#333355]",
                    span { class: "font-bold text-[13px] text-[#e0e0e0]", "{agent.name}" }
                    span { class: "text-[12px] text-[#888] flex-1 truncate", "{agent.description}" }
                }
            }

            // Sub-tabs + content
            if selected.is_some() {
                div { class: "flex border-b border-[#333355] bg-[#252540]",
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
                div { class: "flex-1 overflow-hidden",
                    match sub_tab {
                        AgentSubTab::Conversation => rsx! { ConversationView {} },
                        AgentSubTab::Sessions => rsx! { SessionsPanel {} },
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
