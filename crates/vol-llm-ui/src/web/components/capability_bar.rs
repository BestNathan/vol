//! Capability summary bar — sits between conversation and input area.
//! Shows "🛠 N tools · N skills · N MCPs  [✎]" — click ✎ opens the right-side drawer.

use crate::state::{AgentsState, CapabilityDrawerState, CapabilityOverlayState, GlobalState};
use crate::web::components::app::AppState;
use dioxus::prelude::*;

#[component]
pub fn CapabilityBar() -> Element {
    let app_state: AppState = use_context();
    let global: Signal<GlobalState> = use_context();
    let agents: Signal<AgentsState> = use_context();
    let mut drawer_state: Signal<CapabilityDrawerState> = use_context();

    // Shared with the drawer (provided in App) — the drawer writes effective_*
    // on instant-apply toggle success, so the bar's summary counts stay fresh.
    let mut cap_signal: Signal<CapabilityOverlayState> = use_context();

    // Load capabilities when selected agent changes (for summary counts)
    let agents_for_cap = agents.clone();
    let global_for_cap = global.clone();
    let app_for_cap = app_state.clone();
    use_effect(move || {
        let agent_id = agents_for_cap.read().selected.clone().unwrap_or_default();
        let session_id = global_for_cap.read().session_id.clone();
        if agent_id.is_empty() {
            // No agent selected — clear the loading flag so the bar does not
            // stay stuck on "Loading capabilities...".
            cap_signal.with_mut(|s| s.loading = false);
            return;
        }
        let client = app_for_cap
            .active_node_id
            .read()
            .as_ref()
            .and_then(|nid| {
                app_for_cap.dp_pool.read().get(nid).map(|c| c.client.clone())
            })
            .unwrap_or_else(|| app_for_cap.rpc_client.clone());

        // Mark loading so stale counts from the previous agent are not shown
        // while this fetch is in flight.
        cap_signal.with_mut(|s| s.loading = true);
        let mut sig = cap_signal.clone();
        client.agent_get_capabilities(&agent_id, &session_id, move |result| {
            sig.with_mut(|s| match result {
                Ok(cap) => {
                    s.effective_tools.clone_from(&cap.effective_tools);
                    s.effective_skills.clone_from(&cap.effective_skills);
                    s.effective_mcp_servers.clone_from(&cap.effective_mcp_servers);
                    s.loading = false;
                }
                Err(e) => {
                    s.loading = false;
                    log::error!("Failed to load capabilities: {e}");
                }
            });
        });
    });

    let cap = cap_signal.read();
    let n_tools = cap.effective_tools.len();
    let n_skills = cap.effective_skills.len();
    let n_mcps = cap.effective_mcp_servers.len();
    let loading = cap.loading;
    drop(cap);

    let agent_selected = agents.read().selected.is_some();

    rsx! {
        div {
            class: "flex items-center gap-2 px-3 py-1.5 border-t border-[#2a2a44] bg-[#181825] text-[12px]",
            if loading {
                span { class: "text-[#666]", "Loading capabilities..." }
            } else {
                span { class: "text-[#888]",
                    "\u{1F6E0} {n_tools} tools \u{00B7} {n_skills} skills \u{00B7} {n_mcps} MCPs"
                }
                button {
                    class: if agent_selected {
                        "ml-1 px-1.5 py-0.5 text-[11px] bg-[#2a2a44] text-[#aaa] rounded hover:bg-[#3a3a55] hover:text-[#ccc]"
                    } else {
                        "ml-1 px-1.5 py-0.5 text-[11px] bg-[#2a2a44] text-[#555] rounded cursor-not-allowed"
                    },
                    disabled: !agent_selected,
                    onclick: move |_| {
                        if agent_selected {
                            drawer_state.with_mut(|d| d.open = true);
                        }
                    },
                    "\u{270E}"
                }
            }
        }
    }
}
