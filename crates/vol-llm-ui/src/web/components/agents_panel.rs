//! Agents panel showing all registered agents with expandable details.

use dioxus::prelude::*;

use crate::state::{AgentListEntry, AgentsState};

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
            div { class: "agents-panel",
                div { class: "agents-panel-loading", "Loading agents..." }
            }
        };
    }

    if let Some(ref e) = error {
        return rsx! {
            div { class: "agents-panel",
                div { class: "agents-panel-error",
                    "Error: {e}"
                }
            }
        };
    }

    if agents.is_empty() {
        return rsx! {
            div { class: "agents-panel",
                div { class: "agents-panel-empty", "No agents discovered" }
            }
        };
    }

    let items: Vec<Element> = agents.iter().enumerate().map(|(i, agent)| {
        let is_expanded = expanded.contains(&i);
        rsx! { AgentItem { agent: agent.clone(), index: i, is_expanded, agents_signal } }
    }).collect();

    rsx! {
        div { class: "agents-panel",
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
        div { class: "agent-item",
            div {
                class: "agent-item-header",
                onclick: move |_: Event<MouseData>| {
                    agents_signal.with_mut(|s| {
                        if s.expanded.contains(&index) {
                            s.expanded.remove(&index);
                        } else {
                            s.expanded.insert(index);
                        }
                    });
                },
                span { class: "agent-item-chevron", "\u{25be}" }
                span { class: "agent-item-name", "{agent.name}" }
                span {
                    class: "agent-item-scope",
                    style: "background: {scope_color}; color: #1a1a2e;",
                    "{agent.scope}"
                }
            }
            div { class: "agent-item-desc", "{agent.description}" }
            if is_expanded {
                div { class: "agent-item-detail",
                    div { class: "agent-detail-row",
                        span { class: "agent-detail-label", "ID: " }
                        span { class: "agent-detail-value", "{agent.id}" }
                    }
                    div { class: "agent-detail-row",
                        span { class: "agent-detail-label", "Type: " }
                        span { class: "agent-detail-value", "{agent.type_}" }
                    }
                    div { class: "agent-detail-row",
                        span { class: "agent-detail-label", "Scope: " }
                        span { class: "agent-detail-value", "{agent.scope}" }
                    }
                }
            }
        }
    }
}
