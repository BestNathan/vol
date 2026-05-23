//! Agents panel with card grid, sub-tabs, and embedded conversation/sessions.

use dioxus::prelude::*;

use crate::state::{AgentsState, AgentSubTab};
use crate::web::client::AgentListEntry;

use super::conversation::ConversationView;
use super::sessions_panel::SessionsPanel;

/// Agents panel component.
#[component]
pub fn AgentsPanel() -> Element {
    let app: crate::web::components::app::AppState = use_context();
    let agents_signal: Signal<AgentsState> = use_context();

    // Load agents on mount
    use_hook(move || {
        let rpc = app.rpc_client.clone();
        let mut sig = agents_signal;

        sig.with_mut(|s| {
            s.loading = true;
            s.error = None;
        });

        rpc.agent_list(move |result| {
            sig.with_mut(|s| {
                s.loading = false;
                match result {
                    Ok(agents) => { s.agents = agents; }
                    Err(e) => { s.error = Some(e); }
                }
            });
        });
    });

    let (loading, error, agents, selected, sub_tab) = {
        let s = agents_signal.read();
        (s.loading, s.error.clone(), s.agents.clone(), s.selected.clone(), s.sub_tab)
    };

    // Loading state (only show loading overlay if no cached data)
    if loading && agents.is_empty() {
        return rsx! {
            div { class: "flex-1 overflow-y-auto p-2",
                div { class: "flex items-center justify-center h-full text-[#666]", "Loading agents..." }
            }
        };
    }

    // Error state
    if let Some(ref e) = error {
        return rsx! {
            div { class: "flex-1 overflow-y-auto p-2",
                div { class: "flex items-center justify-center h-full text-[#ff6060] p-5 text-center",
                    "Error: {e}"
                }
            }
        };
    }

    // Empty state
    if agents.is_empty() {
        return rsx! {
            div { class: "flex-1 overflow-y-auto p-2",
                div { class: "flex items-center justify-center h-full text-[#666]", "No agents discovered" }
            }
        };
    }

    // Find selected agent metadata
    let selected_agent = agents.iter().find(|a| Some(&a.id) == selected.as_ref());

    rsx! {
        div { class: "flex flex-col h-full",
            // Agent card grid
            div { class: "flex flex-wrap gap-2 p-2 border-b border-[#333355] overflow-y-auto max-h-[180px] min-h-[60px]",
                for agent in &agents {
                    AgentCard {
                        key: "{agent.id}",
                        agent: agent.clone(),
                        is_selected: selected.as_ref() == Some(&agent.id),
                        agents_signal: agents_signal.clone(),
                    }
                }
            }
            // Selected agent info bar
            if let Some(agent) = selected_agent {
                div { class: "flex items-center gap-2 px-3 py-1.5 bg-[#1a2a44] border-b border-[#333355]",
                    span { class: "font-bold text-[13px] text-[#e0e0e0]", "{agent.name}" }
                    TypeBadge { type_: agent.type_.clone() }
                    span { class: "text-[12px] text-[#888] flex-1 truncate", "{agent.description}" }
                }
            }
            // Sub-tab bar (only when agent selected)
            if selected.is_some() {
                div { class: "flex border-b border-[#333355] bg-[#252540]",
                    SubTabButton {
                        label: "Conversation",
                        active: sub_tab == AgentSubTab::Conversation,
                        agents_signal: agents_signal.clone(),
                        target_tab: AgentSubTab::Conversation,
                    }
                    SubTabButton {
                        label: "Sessions",
                        active: sub_tab == AgentSubTab::Sessions,
                        agents_signal: agents_signal.clone(),
                        target_tab: AgentSubTab::Sessions,
                    }
                }
            }
            // Sub-tab content
            if selected.is_some() {
                div { class: "flex-1 overflow-hidden",
                    match sub_tab {
                        AgentSubTab::Conversation => rsx! {
                            ConversationView {}
                        },
                        AgentSubTab::Sessions => rsx! {
                            SessionsPanel {}
                        },
                    }
                }
            }
        }
    }
}

/// Agent card component — shows in a grid, clickable to select.
#[component]
fn AgentCard(
    agent: AgentListEntry,
    is_selected: bool,
    agents_signal: Signal<AgentsState>,
) -> Element {
    let agent_id = agent.id.clone();

    let card_class = if is_selected {
        "flex items-center gap-2 px-3 py-2 rounded-md cursor-pointer border border-[#80a0ff] bg-[#1a2a44] min-w-[180px] max-w-[260px]"
    } else {
        "flex items-center gap-2 px-3 py-2 rounded-md cursor-pointer border border-[#2a2a44] bg-[#1e1e36] hover:bg-[#222240] min-w-[180px] max-w-[260px]"
    };

    let scope_color = match agent.scope.as_str() {
        "Server" => "#c0c040",
        "Repo" => "#4080ff",
        "User" => "#40c040",
        _ => "#888",
    };

    rsx! {
        div {
            class: "{card_class}",
            onclick: move |_: Event<MouseData>| {
                agents_signal.with_mut(|s| {
                    if s.selected.as_deref() == Some(&agent_id) {
                        s.selected = None;
                    } else {
                        s.selected = Some(agent_id.clone());
                    }
                });
            },
            // Status dot (green = registered/available)
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

/// Small badge showing the agent type.
#[component]
fn TypeBadge(type_: String) -> Element {
    let color = match type_.as_str() {
        "llm" => "#a060c0",
        "tool" => "#4080c0",
        "chain" => "#40c080",
        _ => "#888",
    };
    rsx! {
        span {
            class: "text-[10px] px-1.5 py-0.5 rounded-[3px] font-bold",
            style: "background: {color}; color: #1a1a2e;",
            "{type_}"
        }
    }
}

/// Sub-tab button for the agent panel's Conversation/Sessions tabs.
#[component]
fn SubTabButton(
    label: String,
    active: bool,
    agents_signal: Signal<AgentsState>,
    target_tab: AgentSubTab,
) -> Element {
    let cls = if active {
        "px-3 py-1.5 text-[12px] font-semibold cursor-pointer bg-[#1a1a2e] text-[#e0e0e0] border-b-2 border-[#80a0ff]"
    } else {
        "px-3 py-1.5 text-[12px] cursor-pointer text-[#888] hover:text-[#ccc] hover:bg-[#2a2a44] border-b-2 border-transparent"
    };
    rsx! {
        button {
            class: "{cls}",
            onclick: move |_: Event<MouseData>| {
                agents_signal.write_unchecked().sub_tab = target_tab;
            },
            "{label}"
        }
    }
}
