//! Capability Drawer — fixed right-side panel for selecting agent capabilities.
//! Replaces the absolute-positioned CapabilityDropdown with a proper overlay.

use crate::state::{
    AgentsState, CapabilityDrawerState, CapabilityOverlayState, GlobalState, ToggleSavingState,
};
use crate::web::components::app::AppState;
use dioxus::prelude::*;
use std::collections::{HashMap, HashSet};

#[component]
pub fn CapabilityDrawer() -> Element {
    let app_state: AppState = use_context();
    let agents: Signal<AgentsState> = use_context();
    let global: Signal<GlobalState> = use_context();
    let mut drawer_state: Signal<CapabilityDrawerState> = use_context();

    // Local state: selected sets (initialized from effective on load)
    let sel_tools: Signal<HashSet<String>> = use_signal(HashSet::new);
    let sel_skills: Signal<HashSet<String>> = use_signal(HashSet::new);
    let sel_mcps: Signal<HashSet<String>> = use_signal(HashSet::new);

    // Capability data loaded from backend. Shared with the CapabilityBar via
    // context (provided in App), so `handle_toggle`'s effective_* updates also
    // refresh the bar's summary counts.
    let cap_signal: Signal<CapabilityOverlayState> = use_context();

    // Load capabilities when the drawer opens.
    // The effect reads `drawer_state`, so it re-runs on any drawer state change
    // (search typing, collapses, saving feedback); the `loaded` guard ensures we
    // only fetch once per open session. The close handlers below reset `loaded`,
    // so the next open always fetches fresh data.
    let app_for_effect = app_state.clone();
    let agents_for_effect = agents;
    let global_for_effect = global;
    use_effect(move || {
        if !drawer_state.read().open || drawer_state.read().loaded {
            return;
        }
        let agent_id = agents_for_effect
            .read()
            .selected
            .clone()
            .unwrap_or_default();
        if agent_id.is_empty() {
            return;
        }
        let session_id = global_for_effect.read().session_id.clone();

        // Use DP client only (same pattern as existing CapabilityBar)
        let client = app_for_effect
            .active_node_id
            .read()
            .as_ref()
            .and_then(|nid| {
                app_for_effect
                    .dp_pool
                    .read()
                    .get(nid)
                    .map(|c| c.client.clone())
            })
            .unwrap_or_else(|| app_for_effect.rpc_client.clone());

        let mut cap = cap_signal;
        let mut drawer = drawer_state;
        let mut st = sel_tools;
        let mut ss = sel_skills;
        let mut sm = sel_mcps;

        client.agent_get_capabilities(&agent_id, &session_id, move |result| match result {
            Ok(r) => {
                cap.with_mut(|c| {
                    c.effective_tools.clone_from(&r.effective_tools);
                    c.available_tools.clone_from(&r.available_tools);
                    c.base_tools.clone_from(&r.base_tools);
                    c.effective_skills.clone_from(&r.effective_skills);
                    c.available_skills.clone_from(&r.available_skills);
                    c.base_skills.clone_from(&r.base_skills);
                    c.effective_mcp_servers.clone_from(&r.effective_mcp_servers);
                    c.available_mcp_servers.clone_from(&r.available_mcp_servers);
                    c.base_mcp_servers.clone_from(&r.base_mcp_servers);
                    c.loading = false;
                });
                st.set(r.effective_tools.iter().cloned().collect());
                ss.set(r.effective_skills.iter().cloned().collect());
                sm.set(r.effective_mcp_servers.iter().cloned().collect());
                drawer.with_mut(|d| {
                    d.loaded = true;
                    d.load_error = None;
                });
            }
            Err(e) => {
                drawer.with_mut(|d| {
                    d.loaded = true;
                    d.load_error = Some(e);
                });
            }
        });
    });

    let ds = drawer_state.read();
    if !ds.open {
        return rsx! {};
    }
    let search = ds.search.clone();
    let loaded = ds.loaded;
    let load_error = ds.load_error.clone();
    drop(ds);

    let cap = cap_signal.read();
    let avail_tools = cap.available_tools.clone();
    let avail_skills = cap.available_skills.clone();
    let avail_mcps = cap.available_mcp_servers.clone();
    let base_tools = cap.base_tools.clone();
    let base_skills = cap.base_skills.clone();
    let base_mcps = cap.base_mcp_servers.clone();
    drop(cap);

    let ds_read = drawer_state.read();
    let collapsed = ds_read.collapsed_sections.clone();
    let saving_states = ds_read.saving_states.clone();
    drop(ds_read);

    rsx! {
        // Backdrop overlay
        div {
            class: "fixed inset-0 bg-black/50 z-40",
            onclick: move |_| {
                drawer_state.with_mut(|d| {
                    d.open = false;
                    d.loaded = false;
                    d.load_error = None;
                });
            },
        }
        // Drawer panel
        div {
            class: "fixed right-0 top-0 h-full w-80 bg-[#1a1a2e] border-l border-[#3a3a55] z-50 flex flex-col shadow-2xl",
            DrawerHeader {
                on_close: move |_| {
                    drawer_state.with_mut(|d| {
                        d.open = false;
                        d.loaded = false;
                        d.load_error = None;
                    });
                },
            }
            div { class: "flex-1 overflow-y-auto",
                if !loaded {
                    div { class: "p-4 text-[#888] text-[13px] text-center", "Loading..." }
                } else if let Some(ref err) = load_error {
                    div { class: "p-4 text-[#c04040] text-[13px] text-center", "Error: {err}" }
                } else {
                    // Provider section (read-only Phase 1)
                    ProviderSection {
                        drawer_state,
                    }

                    div { class: "border-t border-[#2a2a44] my-1" }

                    // Search
                    SearchInput {
                        search: search.clone(),
                        on_input: move |val: String| {
                            drawer_state.with_mut(|d| d.search = val);
                        },
                    }

                    div { class: "border-t border-[#2a2a44] my-1" }

                    // Tools group
                    SectionGroup {
                        title: "Tools".to_string(),
                        group_key: "tools".to_string(),
                        items: avail_tools,
                        selected: sel_tools.read().clone(),
                        base_set: base_tools,
                        saving_states: saving_states.clone(),
                        collapsed: collapsed.clone(),
                        search: search.clone(),
                        on_toggle: {
                            let st = sel_tools;
                            let ss = sel_skills;
                            let sm = sel_mcps;
                            let cs = cap_signal;
                            let ag = agents;
                            let gl = global;
                            let ap = app_state.clone();
                            let ds = drawer_state;
                            move |(name, enabled): (String, bool)| {
                                handle_toggle(
                                    "tools", &name, enabled,
                                    st, ss, sm,
                                    cs, ag, gl,
                                    ap.clone(), ds,
                                );
                            }
                        },
                        on_collapse: {
                            let mut ds = drawer_state;
                            move |_| {
                                ds.with_mut(|d| {
                                    if d.collapsed_sections.contains("Tools") {
                                        d.collapsed_sections.remove("Tools");
                                    } else {
                                        d.collapsed_sections.insert("Tools".to_string());
                                    }
                                });
                            }
                        },
                    }

                    div { class: "border-t border-[#2a2a44] my-1" }

                    // Skills group
                    SectionGroup {
                        title: "Skills".to_string(),
                        group_key: "skills".to_string(),
                        items: avail_skills,
                        selected: sel_skills.read().clone(),
                        base_set: base_skills,
                        saving_states: saving_states.clone(),
                        collapsed: collapsed.clone(),
                        search: search.clone(),
                        on_toggle: {
                            let st = sel_tools;
                            let ss = sel_skills;
                            let sm = sel_mcps;
                            let cs = cap_signal;
                            let ag = agents;
                            let gl = global;
                            let ap = app_state.clone();
                            let ds = drawer_state;
                            move |(name, enabled): (String, bool)| {
                                handle_toggle(
                                    "skills", &name, enabled,
                                    st, ss, sm,
                                    cs, ag, gl,
                                    ap.clone(), ds,
                                );
                            }
                        },
                        on_collapse: {
                            let mut ds = drawer_state;
                            move |_| {
                                ds.with_mut(|d| {
                                    if d.collapsed_sections.contains("Skills") {
                                        d.collapsed_sections.remove("Skills");
                                    } else {
                                        d.collapsed_sections.insert("Skills".to_string());
                                    }
                                });
                            }
                        },
                    }

                    div { class: "border-t border-[#2a2a44] my-1" }

                    // MCP group
                    SectionGroup {
                        title: "MCP Servers".to_string(),
                        group_key: "mcps".to_string(),
                        items: avail_mcps,
                        selected: sel_mcps.read().clone(),
                        base_set: base_mcps,
                        saving_states,
                        collapsed,
                        search,
                        on_toggle: {
                            let st = sel_tools;
                            let ss = sel_skills;
                            let sm = sel_mcps;
                            let cs = cap_signal;
                            let ag = agents;
                            let gl = global;
                            let ap = app_state;
                            let ds = drawer_state;
                            move |(name, enabled): (String, bool)| {
                                handle_toggle(
                                    "mcps", &name, enabled,
                                    st, ss, sm,
                                    cs, ag, gl,
                                    ap.clone(), ds,
                                );
                            }
                        },
                        on_collapse: {
                            let mut ds = drawer_state;
                            move |_| {
                                ds.with_mut(|d| {
                                    if d.collapsed_sections.contains("MCP Servers") {
                                        d.collapsed_sections.remove("MCP Servers");
                                    } else {
                                        d.collapsed_sections.insert("MCP Servers".to_string());
                                    }
                                });
                            }
                        },
                    }
                }
            }
        }
    }
}

#[component]
fn DrawerHeader(on_close: EventHandler<()>) -> Element {
    rsx! {
        div { class: "flex items-center justify-between px-4 py-3 border-b border-[#3a3a55] flex-shrink-0",
            span { class: "text-[14px] font-semibold text-[#e0e0e0]", "Capabilities" }
            button {
                class: "text-[18px] text-[#888] hover:text-[#ccc] leading-none",
                onclick: move |_| on_close.call(()),
                "\u{2715}"
            }
        }
    }
}

#[component]
fn ProviderSection(drawer_state: Signal<CapabilityDrawerState>) -> Element {
    let ds = drawer_state.read();
    let collapsed = ds.collapsed_sections.contains("Provider");
    drop(ds);

    rsx! {
        div { class: "px-3 py-2",
            // Collapsible header
            div {
                class: "flex items-center justify-between cursor-pointer hover:bg-[#2a2a44] rounded px-1 py-0.5",
                onclick: move |_| {
                    drawer_state.with_mut(|d| {
                        if d.collapsed_sections.contains("Provider") {
                            d.collapsed_sections.remove("Provider");
                        } else {
                            d.collapsed_sections.insert("Provider".to_string());
                        }
                    });
                },
                span { class: "text-[11px] font-semibold text-[#888] uppercase tracking-[0.5px]",
                    "Provider"
                }
                span { class: "text-[10px] text-[#666]",
                    if collapsed { "\u{25b8}" } else { "\u{25be}" }
                }
            }
            if !collapsed {
                div { class: "mt-2 space-y-2",
                    // Provider dropdown (disabled)
                    div { class: "flex flex-col gap-1",
                        span { class: "text-[11px] text-[#666]", "Provider" }
                        div {
                            class: "w-full px-2 py-1.5 bg-[#12121e] border border-[#2a2a44] rounded text-[12px] text-[#555] cursor-not-allowed",
                            "Not configurable via UI"
                        }
                    }
                    // Model dropdown (disabled)
                    div { class: "flex flex-col gap-1",
                        span { class: "text-[11px] text-[#666]", "Model" }
                        div {
                            class: "w-full px-2 py-1.5 bg-[#12121e] border border-[#2a2a44] rounded text-[12px] text-[#555] cursor-not-allowed",
                            "Set in agent definition"
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn SearchInput(search: String, on_input: EventHandler<String>) -> Element {
    rsx! {
        div { class: "px-3 py-2",
            div { class: "relative",
                span { class: "absolute left-2 top-1/2 -translate-y-1/2 text-[#666] text-[12px] pointer-events-none",
                    "\u{1F50D}"
                }
                input {
                    class: "w-full pl-7 pr-2 py-1.5 bg-[#12121e] border border-[#2a2a44] rounded text-[12px] text-[#ccc] placeholder-[#555] focus:outline-none focus:border-[#80a0ff]",
                    r#type: "text",
                    placeholder: "Search capabilities...",
                    value: "{search}",
                    oninput: move |evt: Event<FormData>| {
                        on_input.call(evt.value());
                    },
                }
            }
        }
    }
}

#[component]
fn SectionGroup(
    title: String,
    group_key: String,
    items: Vec<serde_json::Value>,
    selected: HashSet<String>,
    base_set: Vec<String>,
    saving_states: HashMap<String, ToggleSavingState>,
    collapsed: HashSet<String>,
    search: String,
    on_toggle: EventHandler<(String, bool)>,
    on_collapse: EventHandler<()>,
) -> Element {
    let section_key = title.clone();
    let is_collapsed = collapsed.contains(&section_key);
    let base_set_clone = base_set;
    let search_lower = search.to_lowercase();
    let matches_search = move |name: &str| -> bool {
        search_lower.is_empty() || name.to_lowercase().contains(&search_lower)
    };

    // Extract name from JSON value and filter
    let filtered: Vec<(String, bool)> = items
        .iter()
        .filter_map(|v| {
            let name = v
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .to_string();
            if name.is_empty() || !matches_search(&name) {
                return None;
            }
            let is_base = base_set_clone.contains(&name);
            Some((name, is_base))
        })
        .collect();

    rsx! {
        div { class: "px-3 py-1",
            // Header row
            div {
                class: "flex items-center justify-between cursor-pointer hover:bg-[#2a2a44] rounded px-1 py-1",
                onclick: move |_| on_collapse.call(()),
                span { class: "text-[11px] font-semibold text-[#888] uppercase tracking-[0.5px]",
                    "{title} ({filtered.len()})"
                }
                span { class: "text-[10px] text-[#666]",
                    if is_collapsed { "\u{25b8}" } else { "\u{25be}" }
                }
            }
            // Items
            if !is_collapsed {
                if filtered.is_empty() {
                    div { class: "text-[11px] text-[#666] px-2 py-1", "No matching capabilities" }
                } else {
                    for (name, is_base) in &filtered {
                        CapabilityToggle {
                            name: name.clone(),
                            is_base: *is_base,
                            checked: selected.contains(name),
                            saving_state: saving_states
                                .get(&format!("{group_key}:{name}"))
                                .cloned(),
                            on_toggle: {
                                let ot = on_toggle;
                                let n = name.clone();
                                let checked = selected.contains(name);
                                move |_| {
                                    ot.call((n.clone(), !checked));
                                }
                            },
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn CapabilityToggle(
    name: String,
    is_base: bool,
    checked: bool,
    saving_state: Option<ToggleSavingState>,
    on_toggle: EventHandler<()>,
) -> Element {
    let name_color = if is_base {
        "text-[#e0e0e0]"
    } else {
        "text-[#80a0ff]"
    };

    let (status_icon, status_class) = match saving_state {
        Some(ToggleSavingState::Saving) => ("\u{25CC}", "text-[#c0a040] animate-spin"),
        Some(ToggleSavingState::Saved) => ("\u{2713}", "text-[#40c040]"),
        Some(ToggleSavingState::Error(_)) => ("\u{26A0}", "text-[#c04040]"),
        None => ("", ""),
    };

    let error_msg = match &saving_state {
        Some(ToggleSavingState::Error(msg)) => Some(msg.clone()),
        _ => None,
    };

    rsx! {
        div { class: "flex items-center gap-2 py-1 px-1 hover:bg-[#222240] rounded",
            // Toggle switch
            button {
                class: if checked {
                    "w-8 h-4 rounded-full relative transition-colors bg-[#4080ff] flex-shrink-0"
                } else {
                    "w-8 h-4 rounded-full relative transition-colors bg-[#3a3a55] flex-shrink-0"
                },
                onclick: move |_| on_toggle.call(()),
                div {
                    class: if checked {
                        "absolute top-0.5 right-0.5 w-3 h-3 rounded-full bg-white transition-all"
                    } else {
                        "absolute top-0.5 left-0.5 w-3 h-3 rounded-full bg-[#888] transition-all"
                    },
                }
            }
            // Name
            span { class: "text-[12px] {name_color} flex-1 truncate", "{name}" }
            // Saving state indicator
            if !status_icon.is_empty() {
                if let Some(ref err) = error_msg {
                    span {
                        class: "text-[12px] {status_class} cursor-help flex-shrink-0",
                        title: "{err}",
                        "{status_icon}"
                    }
                } else {
                    span { class: "text-[12px] {status_class} flex-shrink-0", "{status_icon}" }
                }
            }
        }
    }
}

/// Handle a toggle flip — update local state, call backend, show feedback.
#[allow(clippy::too_many_arguments)]
fn handle_toggle(
    group: &str,
    name: &str,
    enabled: bool,
    mut sel_tools: Signal<HashSet<String>>,
    mut sel_skills: Signal<HashSet<String>>,
    mut sel_mcps: Signal<HashSet<String>>,
    mut cap_signal: Signal<CapabilityOverlayState>,
    agents: Signal<AgentsState>,
    global: Signal<GlobalState>,
    app_state: AppState,
    mut drawer_state: Signal<CapabilityDrawerState>,
) {
    // The RPC callback below is `'static`, so group/name must be owned.
    let group = group.to_string();
    let name = name.to_string();
    let state_key = format!("{group}:{name}");

    // Update local selection
    match group.as_str() {
        "tools" => {
            sel_tools.with_mut(|s| {
                if enabled {
                    s.insert(name.clone());
                } else {
                    s.remove(&name);
                }
            });
        }
        "skills" => {
            sel_skills.with_mut(|s| {
                if enabled {
                    s.insert(name.clone());
                } else {
                    s.remove(&name);
                }
            });
        }
        "mcps" => {
            sel_mcps.with_mut(|s| {
                if enabled {
                    s.insert(name.clone());
                } else {
                    s.remove(&name);
                }
            });
        }
        _ => return,
    }

    // Mark saving
    drawer_state.with_mut(|d| {
        d.saving_states
            .insert(state_key.clone(), ToggleSavingState::Saving);
    });

    // Build current selection sets
    let tools: Vec<String> = sel_tools.read().iter().cloned().collect();
    let skills: Vec<String> = sel_skills.read().iter().cloned().collect();
    let mcps: Vec<String> = sel_mcps.read().iter().cloned().collect();

    let agent_id = agents.read().selected.clone().unwrap_or_default();
    let session_id = global.read().session_id.clone();
    if agent_id.is_empty() {
        return;
    }

    let client = app_state
        .active_node_id
        .read()
        .as_ref()
        .and_then(|nid| app_state.dp_pool.read().get(nid).map(|c| c.client.clone()))
        .unwrap_or_else(|| app_state.rpc_client.clone());

    client.agent_update_capabilities(&agent_id, &session_id, tools, skills, mcps, move |result| {
        // Rapid-toggle race guard: if the user toggled this item again while
        // this request was in flight, the current selection no longer matches
        // this request's intent — discard the stale response (success or error)
        // so an older response cannot clobber the newer state.
        let current_enabled = match group.as_str() {
            "tools" => sel_tools.read().contains(&name),
            "skills" => sel_skills.read().contains(&name),
            "mcps" => sel_mcps.read().contains(&name),
            _ => return,
        };
        if current_enabled != enabled {
            return;
        }
        match result {
            Ok(upd) => {
                // Update effective from server response
                cap_signal.with_mut(|c| {
                    c.effective_tools.clone_from(&upd.effective_tools);
                    c.effective_skills.clone_from(&upd.effective_skills);
                    c.effective_mcp_servers
                        .clone_from(&upd.effective_mcp_servers);
                });
                // Mark saved, then clear after 1.5s so the checkmark ages out
                drawer_state.with_mut(|d| {
                    d.saving_states
                        .insert(state_key.clone(), ToggleSavingState::Saved);
                });
                let mut ds_for_clear = drawer_state;
                let clear_key = state_key;
                wasm_bindgen_futures::spawn_local(async move {
                    gloo_timers::future::TimeoutFuture::new(1500).await;
                    ds_for_clear.with_mut(|d| {
                        if let Some(ToggleSavingState::Saved) = d.saving_states.get(&clear_key) {
                            d.saving_states.remove(&clear_key);
                        }
                    });
                });
            }
            Err(e) => {
                // Rollback local selection
                match group.as_str() {
                    "tools" => {
                        sel_tools.with_mut(|s| {
                            if enabled {
                                s.remove(&name);
                            } else {
                                s.insert(name.clone());
                            }
                        });
                    }
                    "skills" => {
                        sel_skills.with_mut(|s| {
                            if enabled {
                                s.remove(&name);
                            } else {
                                s.insert(name.clone());
                            }
                        });
                    }
                    "mcps" => {
                        sel_mcps.with_mut(|s| {
                            if enabled {
                                s.remove(&name);
                            } else {
                                s.insert(name.clone());
                            }
                        });
                    }
                    _ => {}
                }
                drawer_state.with_mut(|d| {
                    d.saving_states
                        .insert(state_key, ToggleSavingState::Error(e));
                });
            }
        }
    });
}
