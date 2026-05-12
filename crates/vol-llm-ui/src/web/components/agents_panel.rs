//! Agents panel showing all registered agents with expandable details.

use dioxus::prelude::*;

use crate::state::AgentsState;
use crate::web::client::AgentListEntry;

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

    let (agents, expanded, loading, error) = {
        let s = agents_signal.read();
        (s.agents.clone(), s.expanded.clone(), s.loading, s.error.clone())
    };

    if loading {
        return rsx! {
            div { class: "flex-1 overflow-y-auto p-2",
                div { class: "flex items-center justify-center h-full text-[#666] p-5 text-center", "Loading agents..." }
            }
        };
    }

    if let Some(ref e) = error {
        return rsx! {
            div { class: "flex-1 overflow-y-auto p-2",
                div { class: "flex items-center justify-center h-full text-[#ff6060] p-5 text-center",
                    "Error: {e}"
                }
            }
        };
    }

    if agents.is_empty() {
        return rsx! {
            div { class: "flex-1 overflow-y-auto p-2",
                div { class: "flex items-center justify-center h-full text-[#666] p-5 text-center", "No agents discovered" }
            }
        };
    }

    let items: Vec<Element> = agents.iter().enumerate().map(|(i, agent)| {
        let is_expanded = expanded.contains(&i);
        rsx! { AgentItem { agent: agent.clone(), index: i, is_expanded, agents_signal } }
    }).collect();

    rsx! {
        div { class: "flex-1 overflow-y-auto p-2",
            {items.into_iter()}
        }
    }
}

#[component]
fn AgentItem(agent: AgentListEntry, index: usize, is_expanded: bool, agents_signal: Signal<AgentsState>) -> Element {
    let scope_color = match agent.scope.as_str() {
        "Server" => "#c0c040",
        "Repo" => "#4080ff",
        "User" => "#40c040",
        _ => "#888",
    };

    rsx! {
        div { class: "border-b border-[#2a2a44]",
            div {
                class: "flex items-center px-2.5 py-2 cursor-pointer gap-2 hover:bg-[#222240]",
                onclick: move |_: Event<MouseData>| {
                    agents_signal.with_mut(|s| {
                        if s.expanded.contains(&index) {
                            s.expanded.remove(&index);
                        } else {
                            s.expanded.insert(index);
                        }
                    });
                },
                span { class: "text-[10px] text-[#666] transition-transform duration-150", "\u{25be}" }
                span { class: "font-semibold text-[13px] text-[#e0e0e0]", "{agent.name}" }
                span {
                    class: "text-[10px] px-1.5 py-0.5 rounded-[3px] font-bold ml-auto",
                    style: "background: {scope_color}; color: #1a1a2e;",
                    "{agent.scope}"
                }
            }
            div { class: "text-[12px] text-[#888] px-2.5 pb-1.5 pl-7", "{agent.description}" }
            if is_expanded {
                div { class: "px-2.5 pb-2 pl-7 text-[12px] bg-[#16162a]",
                    div { class: "py-0.5",
                        span { class: "text-[#6090ff] font-semibold", "ID: " }
                        span { class: "text-[#ccc] font-mono", "{agent.id}" }
                    }
                    div { class: "py-0.5",
                        span { class: "text-[#6090ff] font-semibold", "Type: " }
                        span { class: "text-[#ccc] font-mono", "{agent.type_}" }
                    }
                    div { class: "py-0.5",
                        span { class: "text-[#6090ff] font-semibold", "Scope: " }
                        span { class: "text-[#ccc] font-mono", "{agent.scope}" }
                    }
                }
            }
        }
    }
}
