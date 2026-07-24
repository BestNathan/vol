//! Node detail panel — CP-scoped view for a single node's metadata, load,
//! agents, and capabilities.

use std::rc::Rc;
use std::time::Duration;

use dioxus::prelude::*;
use gloo_timers::future::sleep;

use crate::web::client::{AgentListEntry, CapabilitySnapshot, NodeRecord};
use crate::web::components::app::AppState;

/// Aggregate state for the node detail view.
#[derive(Debug, Clone, Default)]
pub struct NodeDetailState {
    pub node: Option<NodeRecord>,
    pub agents: Vec<AgentListEntry>,
    pub capabilities: Option<CapabilitySnapshot>,
    pub loading: bool,
    pub error: Option<String>,
}

/// Detail panel for a single node, fetched via control-plane RPCs.
///
/// Displays four sections:
/// - **Overview** — node_id, name, version, status, last_seen
/// - **Resource Usage** — running / queued from `NodeLoad`
/// - **Agents on this Node** — agents filtered from `agent.list` by `node_id`
/// - **Capabilities** — tools / skills / MCP counts from `capability_list`
#[component]
pub fn NodeDetailPanel(node_id: String) -> Element {
    let mut app = use_context::<AppState>();
    let state = use_signal(NodeDetailState::default);

    // TODO(spec 7.5): spec mockup shows ws_url but NodeRecord doesn't carry it
    // in the current protocol.  A protocol extension is needed to populate this
    // field — out of scope for the auto-refresh work.

    // Cancellation token: a Weak<()> that becomes dangling when the component
    // unmounts (use_hook drops the strong Rc).  The polling loop checks this on
    // every iteration and exits gracefully.
    let (_alive_strong, alive_weak) = use_hook(|| {
        let strong = Rc::new(());
        let weak = Rc::downgrade(&strong);
        // Return the strong ref so use_hook keeps it alive for the component's
        // lifetime.  When the component unmounts, use_hook drops it, making
        // weak.upgrade() return None.
        (strong, weak)
    });

    use_effect(move || {
        let mut s = state;
        s.with_mut(|s| {
            *s = NodeDetailState {
                loading: true,
                error: None,
                ..NodeDetailState::default()
            };
        });

        let cp = app.cp_client.clone();
        let nid = node_id.clone();
        let alive_check = alive_weak.clone();
        wasm_bindgen_futures::spawn_local(async move {
            // 1. Fetch node detail
            let (tx, rx) = futures_channel::oneshot::channel();
            cp.node_get(&nid, move |result| {
                let _ = tx.send(result);
            });

            match rx.await {
                Ok(Ok(Some(node))) => {
                    s.with_mut(|s| {
                        s.node = Some(node);
                        s.loading = false;
                    });
                }
                Ok(Ok(None)) => {
                    s.with_mut(|s| {
                        s.error = Some("Node not found".into());
                        s.loading = false;
                    });
                    return;
                }
                Ok(Err(e)) => {
                    s.with_mut(|s| {
                        s.error = Some(e);
                        s.loading = false;
                    });
                    return;
                }
                Err(_) => {
                    s.with_mut(|s| {
                        s.error = Some("Channel closed".into());
                        s.loading = false;
                    });
                    return;
                }
            }

            // 2. Fetch agents on this node
            let (tx2, rx2) = futures_channel::oneshot::channel();
            cp.agent_list(move |result| {
                let _ = tx2.send(result);
            });

            if let Ok(Ok(agents)) = rx2.await {
                let node_agents: Vec<_> = agents
                    .into_iter()
                    .filter(|a| a.node_id.as_deref() == Some(&nid))
                    .collect();
                s.with_mut(|s| {
                    s.agents = node_agents;
                });
            }

            // 3. Fetch capabilities for this node
            let (tx3, rx3) = futures_channel::oneshot::channel();
            cp.capability_list(Some(&nid), move |result| {
                let _ = tx3.send(result);
            });

            if let Ok(Ok(caps)) = rx3.await {
                s.with_mut(|s| {
                    s.capabilities = caps.into_iter().next();
                });
            }

            // 4. Auto-refresh polling loop (spec §7.5 item 4):
            //    Re-fetch node_get every 5 s to keep resource-usage counts fresh.
            //    Exits when the component unmounts (alive_check.upgrade() → None).
            loop {
                sleep(Duration::from_secs(5)).await;

                // Stop polling if the component has been unmounted.
                if alive_check.upgrade().is_none() {
                    break;
                }

                let (tx_poll, rx_poll) = futures_channel::oneshot::channel();
                cp.node_get(&nid, move |result| {
                    let _ = tx_poll.send(result);
                });

                match rx_poll.await {
                    Ok(Ok(Some(node))) => {
                        s.with_mut(|s| {
                            s.node = Some(node);
                        });
                    }
                    Ok(Ok(None)) => {
                        // Node disappeared — surface an error and stop polling.
                        s.with_mut(|s| {
                            s.error = Some("Node not found".into());
                        });
                        break;
                    }
                    Ok(Err(_e)) => {
                        // Transient error — keep polling; the UI retains the
                        // last-known-good data.
                    }
                    Err(_) => {
                        // Channel closed — stop polling.
                        break;
                    }
                }

                // Re-check after the RPC round-trip in case we were dropped
                // while awaiting.
                if alive_check.upgrade().is_none() {
                    break;
                }
            }
        });
    });

    let s = state.read();

    rsx! {
        div { class: "flex flex-col h-full p-3 overflow-auto",
            // Back button
            button {
                class: "self-start flex items-center gap-1 px-2 py-1 mb-3 text-xs text-[#80a0ff] bg-transparent border border-[#333355] rounded cursor-pointer hover:bg-[#2a2a44]",
                onclick: move |_| {
                    app.viewing_node_detail.set(None);
                },
                "← Back"
            }

            if s.loading {
                div { class: "text-[#888] text-sm", "Loading node detail..." }
            } else if let Some(ref err) = s.error {
                div { class: "text-red-400 text-sm", "Error: {err}" }
            } else if let Some(ref node) = s.node {
                // Overview section
                OverviewSection { node: node.clone() }

                // Resource Usage section
                ResourceSection { node: node.clone() }

                // Agents section
                AgentsSection {
                    agents: s.agents.clone(),
                }

                // Capabilities section
                CapabilitiesSection {
                    capabilities: s.capabilities.clone(),
                }
            }
        }
    }
}

/// Overview — node_id, name, version, status, last_seen.
#[component]
fn OverviewSection(node: NodeRecord) -> Element {
    let status_color = if node.status == "online" {
        "text-green-400"
    } else {
        "text-red-400"
    };

    let last_seen_label = node
        .last_seen_at_ms
        .map(|ms| format_age_ms(ms))
        .unwrap_or_else(|| "never".to_string());

    rsx! {
        div { class: "mb-4",
            h3 { class: "text-sm font-semibold text-[#e0e0e0] mb-2 border-b border-[#333355] pb-1",
                "Overview"
            }
            div { class: "grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm",
                span { class: "text-[#888]", "Node ID:" }
                span { class: "text-[#e0e0e0] font-mono text-xs", "{node.node_id}" }

                span { class: "text-[#888]", "Name:" }
                span { class: "text-[#e0e0e0]", "{node.name}" }

                span { class: "text-[#888]", "Version:" }
                span { class: "text-[#e0e0e0]", "v{node.version}" }

                span { class: "text-[#888]", "Status:" }
                span { class: "{status_color}", "{node.status}" }

                span { class: "text-[#888]", "Last Seen:" }
                span { class: "text-[#e0e0e0]", "{last_seen_label}" }

                span { class: "text-[#888]", "Cap Revision:" }
                span { class: "text-[#e0e0e0]", "{node.capability_revision}" }
            }
        }
    }
}

/// Resource Usage — running / queued counts from NodeLoad.
#[component]
fn ResourceSection(node: NodeRecord) -> Element {
    rsx! {
        div { class: "mb-4",
            h3 { class: "text-sm font-semibold text-[#e0e0e0] mb-2 border-b border-[#333355] pb-1",
                "Resource Usage"
            }
            div { class: "flex gap-6",
                div { class: "flex flex-col items-center px-4 py-2 rounded bg-[#1a1a2e] border border-[#2a2a44]",
                    span { class: "text-2xl font-bold text-[#80a0ff]", "{node.load.running}" }
                    span { class: "text-xs text-[#888]", "Running" }
                }
                div { class: "flex flex-col items-center px-4 py-2 rounded bg-[#1a1a2e] border border-[#2a2a44]",
                    span { class: "text-2xl font-bold text-[#f0c040]", "{node.load.queued}" }
                    span { class: "text-xs text-[#888]", "Queued" }
                }
            }
        }
    }
}

/// Agents on this Node — filtered list from agent.list.
#[component]
fn AgentsSection(agents: Vec<AgentListEntry>) -> Element {
    rsx! {
        div { class: "mb-4",
            h3 { class: "text-sm font-semibold text-[#e0e0e0] mb-2 border-b border-[#333355] pb-1",
                "Agents on this Node ({agents.len()})"
            }
            if agents.is_empty() {
                div { class: "text-[#888] text-sm", "No agents on this node" }
            } else {
                div { class: "flex flex-col gap-1",
                    for agent in agents.iter() {
                        AgentRow { agent: agent.clone() }
                    }
                }
            }
        }
    }
}

/// Single agent row in the agents section.
#[component]
fn AgentRow(agent: AgentListEntry) -> Element {
    let scope_str = agent.scope.as_deref().unwrap_or("unknown");
    let scope_color = match scope_str {
        "repo" => "#4080ff",
        "user" => "#40c040",
        _ => "#888",
    };

    let description_str = agent.description.as_deref().unwrap_or("");

    rsx! {
        div { class: "flex items-center gap-2 px-2 py-1.5 rounded border-b border-[#333355] hover:bg-[#2a2a44]",
            div { class: "w-2 h-2 rounded-full bg-[#40c040] flex-shrink-0" }
            div { class: "flex-1 min-w-0",
                div { class: "flex items-center gap-1.5",
                    span { class: "text-[#e0e0e0] text-sm font-medium truncate", "{agent.name}" }
                    span {
                        class: "text-[9px] px-1 py-0.5 rounded-[2px] font-bold whitespace-nowrap flex-shrink-0",
                        style: "background: {scope_color}; color: #1a1a2e;",
                        "{scope_str}"
                    }
                }
                div { class: "text-[#666] text-xs truncate", "{description_str}" }
            }
        }
    }
}

/// Capabilities — tools / skills / MCP counts from capability_list.
#[component]
fn CapabilitiesSection(capabilities: Option<CapabilitySnapshot>) -> Element {
    rsx! {
        div { class: "mb-4",
            h3 { class: "text-sm font-semibold text-[#e0e0e0] mb-2 border-b border-[#333355] pb-1",
                "Capabilities"
            }
            match capabilities {
                Some(ref caps) => rsx! {
                    div { class: "flex gap-4 flex-wrap",
                        CapBadge { label: "Agents", count: caps.agents.len(), color: "#40c040" }
                        CapBadge { label: "Tools", count: caps.tools.len(), color: "#80a0ff" }
                        CapBadge { label: "Skills", count: caps.skills.len(), color: "#f0c040" }
                        CapBadge { label: "MCP Servers", count: caps.mcp_servers.len(), color: "#c080ff" }
                    }
                    // Tools list
                    if !caps.tools.is_empty() {
                        div { class: "mt-3",
                            div { class: "text-xs text-[#888] mb-1", "Tools" }
                            div { class: "flex flex-col gap-0.5",
                                for tool in caps.tools.iter() {
                                    div { class: "text-sm text-[#e0e0e0] px-2 py-0.5 rounded hover:bg-[#2a2a44]",
                                        span { class: "font-mono text-xs", "{tool.name}" }
                                        if let Some(ref desc) = tool.description {
                                            span { class: "text-[#666] text-xs ml-2", "{desc}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    // Skills list
                    if !caps.skills.is_empty() {
                        div { class: "mt-3",
                            div { class: "text-xs text-[#888] mb-1", "Skills" }
                            div { class: "flex flex-col gap-0.5",
                                for skill in caps.skills.iter() {
                                    div { class: "text-sm text-[#e0e0e0] px-2 py-0.5 rounded hover:bg-[#2a2a44]",
                                        span { class: "font-mono text-xs", "{skill.name}" }
                                        if let Some(ref desc) = skill.description {
                                            span { class: "text-[#666] text-xs ml-2", "{desc}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    // MCP servers list
                    if !caps.mcp_servers.is_empty() {
                        div { class: "mt-3",
                            div { class: "text-xs text-[#888] mb-1", "MCP Servers" }
                            div { class: "flex flex-col gap-0.5",
                                for mcp in caps.mcp_servers.iter() {
                                    div { class: "flex items-center gap-2 px-2 py-0.5 rounded hover:bg-[#2a2a44]",
                                        span { class: "font-mono text-xs text-[#e0e0e0]", "{mcp.name}" }
                                        if let Some(ref status) = mcp.status {
                                            span { class: "text-xs text-[#888]", "({status})" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
                None => rsx! {
                    div { class: "text-[#888] text-sm", "No capability data available" }
                },
            }
        }
    }
}

/// Capability count badge.
#[component]
fn CapBadge(label: String, count: usize, color: String) -> Element {
    rsx! {
        div { class: "flex flex-col items-center px-3 py-1.5 rounded bg-[#1a1a2e] border border-[#2a2a44]",
            span { class: "text-lg font-bold", style: "color: {color};", "{count}" }
            span { class: "text-[10px] text-[#888]", "{label}" }
        }
    }
}

/// Format a millisecond timestamp as a human-readable age label.
fn format_age_ms(ms: u64) -> String {
    let now = web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let diff_secs = (now.saturating_sub(ms)) / 1000;
    if diff_secs < 60 {
        format!("{diff_secs}s ago")
    } else if diff_secs < 3600 {
        format!("{}m ago", diff_secs / 60)
    } else if diff_secs < 86400 {
        format!("{}h ago", diff_secs / 3600)
    } else {
        format!("{}d ago", diff_secs / 86400)
    }
}
