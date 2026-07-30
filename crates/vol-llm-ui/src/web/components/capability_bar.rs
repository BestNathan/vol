//! Capability summary bar — sits between conversation and input area.
//! Shows "🛠 N tools · N skills · N MCPs  [✎]" with a dropdown picker.

use crate::state::{AgentsState, CapabilityOverlayState, GlobalState};
use crate::web::components::app::AppState;
use dioxus::prelude::*;
use std::collections::HashSet;

#[component]
pub fn CapabilityBar() -> Element {
    let app_state: AppState = use_context();
    let global: Signal<GlobalState> = use_context();
    let agents: Signal<AgentsState> = use_context();

    let cap_signal: Signal<CapabilityOverlayState> = use_signal(CapabilityOverlayState::new);
    let mut dropdown_open: Signal<bool> = use_signal(|| false);

    // Load capabilities when selected agent changes
    let agents_for_cap = agents.clone();
    let global_for_cap = global.clone();
    let app_for_cap = app_state.clone();
    use_effect(move || {
        let mut cap_signal = cap_signal;
        let agent_id = agents_for_cap.read().selected.clone().unwrap_or_default();
        let session_id = global_for_cap.read().session_id.clone();
        if agent_id.is_empty() {
            cap_signal.with_mut(|s| s.loading = false);
            return;
        }
        let node_id = app_for_cap.active_node_id.read().clone();
        // Only use direct DP connection — CP cannot route these operations.
        // The effect re-runs when dp_pool changes, so capabilities load
        // as soon as the DP connection is established.
        let client = match node_id.as_ref().and_then(|nid| {
            app_for_cap
                .dp_pool
                .read()
                .get(nid)
                .map(|c| c.client.clone())
        }) {
            Some(c) => c,
            None => {
                cap_signal.with_mut(|s| s.loading = false);
                return;
            }
        };
        let sig = cap_signal.clone();
        client.agent_get_capabilities(&agent_id, &session_id, move |result| {
            let mut sig = sig;
            sig.with_mut(|s| match result {
                Ok(cap) => {
                    s.effective_tools.clone_from(&cap.effective_tools);
                    s.available_tools = cap.available_tools;
                    s.base_tools = cap.base_tools;
                    s.effective_skills.clone_from(&cap.effective_skills);
                    s.available_skills = cap.available_skills;
                    s.base_skills = cap.base_skills;
                    s.effective_mcp_servers
                        .clone_from(&cap.effective_mcp_servers);
                    s.available_mcp_servers = cap.available_mcp_servers;
                    s.base_mcp_servers = cap.base_mcp_servers;
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

    rsx! {
        div {
            class: "flex items-center gap-2 px-3 py-1.5 border-t border-[#2a2a44] bg-[#181825] text-[12px] relative",
            if loading {
                span { class: "text-[#666]", "Loading capabilities..." }
            } else {
                span { class: "text-[#888]",
                    "🛠 {n_tools} tools · {n_skills} skills · {n_mcps} MCPs"
                }
                button {
                    class: "ml-1 px-1.5 py-0.5 text-[11px] bg-[#2a2a44] text-[#aaa] rounded hover:bg-[#3a3a55] hover:text-[#ccc]",
                    onclick: move |_| dropdown_open.set(!dropdown_open()),
                    "✎"
                }
            }
        }
        if dropdown_open() {
            CapabilityDropdown {
                cap_signal,
                agents,
                global,
                app_state,
                dropdown_open: dropdown_open.clone(),
            }
        }
    }
}

#[component]
fn CapabilityDropdown(
    cap_signal: Signal<CapabilityOverlayState>,
    agents: Signal<AgentsState>,
    global: Signal<GlobalState>,
    app_state: AppState,
    dropdown_open: Signal<bool>,
) -> Element {
    let cap = cap_signal.read();
    let sel_tools: Signal<HashSet<String>> =
        use_signal(|| cap.effective_tools.iter().cloned().collect());
    let sel_skills: Signal<HashSet<String>> =
        use_signal(|| cap.effective_skills.iter().cloned().collect());
    let sel_mcps: Signal<HashSet<String>> =
        use_signal(|| cap.effective_mcp_servers.iter().cloned().collect());
    let dirty: Signal<bool> = use_signal(|| false);
    drop(cap);

    let cap = cap_signal.read();
    let avail_tools = cap.available_tools.clone();
    let avail_skills = cap.available_skills.clone();
    let avail_mcps = cap.available_mcp_servers.clone();
    let base_tools = cap.base_tools.clone();
    let base_skills = cap.base_skills.clone();
    let base_mcps = cap.base_mcp_servers.clone();
    drop(cap);

    let d = *dirty.read();

    rsx! {
        div {
            class: "absolute bottom-full right-0 w-80 mb-1 bg-[#1e1e2e] border border-[#3a3a55] rounded-lg shadow-xl max-h-[60vh] overflow-y-auto z-50",
            div { class: "p-3",
                // Header
                div { class: "flex items-center justify-between mb-3",
                    div { class: "text-[13px] font-semibold text-[#ccc]", "Capabilities" }
                    button {
                        class: "text-[18px] text-[#888] hover:text-[#ccc] leading-none",
                        onclick: move |_| dropdown_open.set(false),
                        "×"
                    }
                }

                // Tools group
                if !avail_tools.is_empty() {
                    div { class: "mb-3",
                        div { class: "text-[11px] font-semibold text-[#888] uppercase tracking-[0.5px] mb-1", "Tools" }
                        for tool in &avail_tools {
                            {
                                let name = tool.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let is_base = base_tools.contains(&name);
                                let label = if is_base { name.clone() } else { format!("+{}", name) };
                                let chk = sel_tools.read().contains(&name);
                                rsx! {
                                    div { class: "flex items-center gap-2 py-0.5",
                                        input {
                                            r#type: "checkbox",
                                            checked: chk,
                                            oninput: {
                                                let st = sel_tools.clone();
                                                let d = dirty.clone();
                                                let n = name.clone();
                                                move |_| {
                                                    let mut st = st;
                                                    let mut d = d;
                                                    let mut hs = st.write();
                                                    if hs.contains(&n) { hs.remove(&n); } else { hs.insert(n.clone()); }
                                                    d.set(true);
                                                }
                                            },
                                        }
                                        span { class: "text-[12px] text-[#ccc]", "{label}" }
                                    }
                                }
                            }
                        }
                    }
                    div { class: "border-t border-[#2a2a44] my-2" }
                }

                // Skills group
                if !avail_skills.is_empty() {
                    div { class: "mb-3",
                        div { class: "text-[11px] font-semibold text-[#888] uppercase tracking-[0.5px] mb-1", "Skills" }
                        for skill in &avail_skills {
                            {
                                let name = skill.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let is_base = base_skills.contains(&name);
                                let label = if is_base { name.clone() } else { format!("+{}", name) };
                                let chk = sel_skills.read().contains(&name);
                                rsx! {
                                    div { class: "flex items-center gap-2 py-0.5",
                                        input {
                                            r#type: "checkbox",
                                            checked: chk,
                                            oninput: {
                                                let ss = sel_skills.clone();
                                                let d = dirty.clone();
                                                let n = name.clone();
                                                move |_| {
                                                    let mut ss = ss;
                                                    let mut d = d;
                                                    let mut hs = ss.write();
                                                    if hs.contains(&n) { hs.remove(&n); } else { hs.insert(n.clone()); }
                                                    d.set(true);
                                                }
                                            },
                                        }
                                        span { class: "text-[12px] text-[#ccc]", "{label}" }
                                    }
                                }
                            }
                        }
                    }
                    div { class: "border-t border-[#2a2a44] my-2" }
                }

                // MCP group
                if !avail_mcps.is_empty() {
                    div { class: "mb-3",
                        div { class: "text-[11px] font-semibold text-[#888] uppercase tracking-[0.5px] mb-1", "MCP Servers" }
                        for server in &avail_mcps {
                            {
                                let name = server.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let is_base = base_mcps.contains(&name);
                                let label = if is_base { name.clone() } else { format!("+{}", name) };
                                let chk = sel_mcps.read().contains(&name);
                                rsx! {
                                    div { class: "flex items-center gap-2 py-0.5",
                                        input {
                                            r#type: "checkbox",
                                            checked: chk,
                                            oninput: {
                                                let sm = sel_mcps.clone();
                                                let d = dirty.clone();
                                                let n = name.clone();
                                                move |_| {
                                                    let mut sm = sm;
                                                    let mut d = d;
                                                    let mut hs = sm.write();
                                                    if hs.contains(&n) { hs.remove(&n); } else { hs.insert(n.clone()); }
                                                    d.set(true);
                                                }
                                            },
                                        }
                                        span { class: "text-[12px] text-[#ccc]", "{label}" }
                                    }
                                }
                            }
                        }
                    }
                    div { class: "border-t border-[#2a2a44] my-2" }
                }

                // Action buttons
                div { class: "flex gap-2 mt-2",
                    button {
                        class: "px-3 py-1 text-[12px] bg-[#4a4aff] text-white rounded hover:bg-[#5a5aff] disabled:opacity-40 disabled:cursor-not-allowed",
                        disabled: !d,
                        onclick: {
                            let st = sel_tools.clone();
                            let ss = sel_skills.clone();
                            let sm = sel_mcps.clone();
                            let cs = cap_signal.clone();
                            let ag = agents.clone();
                            let gl = global.clone();
                            let app = app_state.clone();
                            let mut dd = dropdown_open.clone();
                            let dirty_sig = dirty.clone();
                            move |_| {
                                let agent_id = ag.read().selected.clone().unwrap_or_default();
                                let session_id = gl.read().session_id.clone();
                                if agent_id.is_empty() { return; }
                                let node_id = app.active_node_id.read().clone();
                                let client = node_id
                                    .as_ref()
                                    .and_then(|nid| app.dp_pool.read().get(nid).map(|c| c.client.clone()))
                                    .unwrap_or_else(|| app.rpc_client.clone());
                                let tools: Vec<String> = st.read().iter().cloned().collect();
                                let skills: Vec<String> = ss.read().iter().cloned().collect();
                                let mcps: Vec<String> = sm.read().iter().cloned().collect();
                                let mut cs = cs;
                                let mut dirty_sig = dirty_sig;
                                client.agent_update_capabilities(&agent_id, &session_id, tools, skills, mcps, move |result| {
                                    cs.with_mut(|s| match result {
                                        Ok(upd) => {
                                            s.effective_tools.clone_from(&upd.effective_tools);
                                            s.effective_skills.clone_from(&upd.effective_skills);
                                            s.effective_mcp_servers.clone_from(&upd.effective_mcp_servers);
                                            s.dirty = false;
                                            dirty_sig.set(false);
                                        }
                                        Err(e) => { log::error!("Failed to update capabilities: {e}"); }
                                    });
                                });
                                dd.set(false);
                            }
                        },
                        "Apply"
                    }
                    button {
                        class: "px-3 py-1 text-[12px] bg-[#3a3a55] text-[#ccc] rounded hover:bg-[#4a4a65]",
                        onclick: {
                            let st = sel_tools.clone();
                            let ss = sel_skills.clone();
                            let sm = sel_mcps.clone();
                            let cs = cap_signal.clone();
                            let d = dirty.clone();
                            move |_| {
                                let mut st = st;
                                let mut ss = ss;
                                let mut sm = sm;
                                let mut d = d;
                                let c = cs.read();
                                st.set(c.base_tools.iter().cloned().collect());
                                ss.set(c.base_skills.iter().cloned().collect());
                                sm.set(c.base_mcp_servers.iter().cloned().collect());
                                d.set(true);
                            }
                        },
                        "Reset to default"
                    }
                }
            }
        }
    }
}
