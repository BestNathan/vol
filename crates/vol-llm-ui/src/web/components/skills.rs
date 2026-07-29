//! Skills panel showing available skills.

use crate::state::{
    AgentsState, CapabilityOverlayState, GlobalState, SkillDialogState, UiEventKind,
};
use crate::web::client::JsonRpcClient;
use crate::web::components::app::AppState;
use dioxus::prelude::*;
use std::collections::HashSet;

/// Key used to store the serialized skills list in NodeDataCache.
const CACHE_KEY: &str = "skills";

/// Serializable state cached per-node for instant switching.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct SkillsCacheState {
    skills: Vec<SkillsCacheEntry>,
    loading: bool,
    error: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
struct SkillsCacheEntry {
    name: String,
    version: String,
    scope: String,
    description: String,
}

impl Default for SkillsCacheState {
    fn default() -> Self {
        Self {
            skills: Vec::new(),
            loading: true,
            error: None,
        }
    }
}

#[component]
pub fn SkillsPanel(mut dialog_signal: Signal<SkillDialogState>) -> Element {
    let app_state: AppState = use_context();
    let active_node = app_state.active_node_id;
    let cache = app_state.node_data_cache;

    // Capability overlay state
    let cap_signal: Signal<CapabilityOverlayState> = use_signal(CapabilityOverlayState::new);
    let selected_skills_signal: Signal<HashSet<String>> = use_signal(HashSet::new);
    let cap_dirty: Signal<bool> = use_signal(|| false);
    let global: Signal<GlobalState> = use_context();
    let agents: Signal<AgentsState> = use_context();
    let global_for_cap = global.clone();
    let agents_for_cap = agents.clone();
    let app_state_for_cap = app_state.clone();
    use_hook(move || {
        let agent_id = agents_for_cap.read().selected.clone().unwrap_or_default();
        let session_id = global_for_cap.read().session_id.clone();
        if agent_id.is_empty() {
            cap_signal.with_mut(|s| s.loading = false);
            return;
        }
        // Get client from DP pool or CP fallback.
        let node_id = active_node.read().clone();
        let client = node_id
            .as_ref()
            .and_then(|nid| {
                app_state_for_cap
                    .dp_pool
                    .read()
                    .get(nid)
                    .map(|c| c.client.clone())
            })
            .unwrap_or_else(|| app_state_for_cap.rpc_client.clone());
        let sig = cap_signal.clone();
        let sel = selected_skills_signal.clone();
        let dirty = cap_dirty.clone();
        client.agent_get_capabilities(&agent_id, &session_id, move |result| {
            let mut sig = sig;
            sig.with_mut(|s| match result {
                Ok(cap) => {
                    s.effective_skills = cap.effective_skills.clone();
                    s.available_skills = cap.available_skills;
                    s.base_skills = cap.base_skills;
                    s.effective_tools = cap.effective_tools;
                    s.available_tools = cap.available_tools;
                    s.base_tools = cap.base_tools;
                    s.effective_mcp_servers = cap.effective_mcp_servers;
                    s.available_mcp_servers = cap.available_mcp_servers;
                    s.base_mcp_servers = cap.base_mcp_servers;
                    s.loading = false;
                    s.dirty = false;
                    let hs: HashSet<String> = cap.effective_skills.into_iter().collect();
                    sel.set(hs);
                    dirty.set(false);
                }
                Err(e) => {
                    s.loading = false;
                    log::error!("Failed to load skill capabilities: {e}");
                }
            });
        });
    });

    // Load skills from cache or trigger DP fetch when active_node changes.
    let app_state_for_effect = app_state.clone();
    use_effect(move || {
        let node_id = active_node.read().clone();
        if let Some(ref nid) = node_id {
            let cached = {
                let c = cache.read();
                c.get(nid).and_then(|d| d.data.get(CACHE_KEY).cloned())
            };

            if cached.is_some() {
                return;
            }

            // Prefer DP client, fall back to CP rpc_client.
            let client = app_state_for_effect
                .dp_pool
                .read()
                .get(nid)
                .map(|c| c.client.clone())
                .unwrap_or_else(|| app_state_for_effect.rpc_client.clone());

            let mut cache_mut = cache;
            let target_nid = nid.clone();
            let cache_nid = nid.clone();

            // Mark as loading in cache immediately.
            {
                let loading_state = SkillsCacheState::default();
                let v = serde_json::to_value(&loading_state).unwrap_or_default();
                let mut c = cache_mut.write();
                let node_data = c.get_or_insert(&cache_nid);
                node_data.data.insert(CACHE_KEY.to_string(), v);
            }

            client.skill_list(move |result| {
                let current_nid = active_node.read().clone();
                if current_nid != Some(target_nid) {
                    log::warn!("Node switched, discarding stale skill_list response");
                    return;
                }
                let mut c = cache_mut.write();
                if let Some(d) = c.get_mut(&cache_nid) {
                    if let Some(v) = d.data.get_mut(CACHE_KEY) {
                        if let Some(obj) = v.as_object_mut() {
                            match result {
                                Ok(entries) => {
                                    let parsed: Vec<SkillsCacheEntry> = entries
                                        .iter()
                                        .map(|e| SkillsCacheEntry {
                                            name: e.name.clone(),
                                            version: e.version.clone(),
                                            scope: e.scope.clone(),
                                            description: e.description.clone(),
                                        })
                                        .collect();
                                    obj.insert(
                                        "skills".to_string(),
                                        serde_json::to_value(parsed).unwrap_or_default(),
                                    );
                                    obj.insert("loading".to_string(), serde_json::json!(false));
                                }
                                Err(e) => {
                                    obj.insert("error".to_string(), serde_json::json!(e));
                                    obj.insert("loading".to_string(), serde_json::json!(false));
                                }
                            }
                        }
                    }
                }
            });
        }
    });

    // Re-fetch on reconnect
    let event_bus = app_state.event_bus.clone();
    let app_state_for_hook = app_state.clone();
    use_hook(move || {
        let _sub = event_bus.subscribe(UiEventKind::WsConnected, move |_| {
            let node_id = active_node.read().clone();
            if let Some(ref nid) = node_id {
                // Invalidate cache.
                let mut c = cache.write_unchecked();
                c.invalidate(nid);

                let client = app_state_for_hook
                    .dp_pool
                    .read()
                    .get(nid)
                    .map(|c| c.client.clone())
                    .unwrap_or_else(|| app_state_for_hook.rpc_client.clone());

                let cache_mut = cache;
                let target_nid = nid.clone();
                let cache_nid = nid.clone();

                // Mark loading.
                {
                    let loading_state = SkillsCacheState::default();
                    let v = serde_json::to_value(&loading_state).unwrap_or_default();
                    let mut c = cache_mut.write_unchecked();
                    let node_data = c.get_or_insert(&cache_nid);
                    node_data.data.insert(CACHE_KEY.to_string(), v);
                }

                client.skill_list(move |result| {
                    let current_nid = active_node.read().clone();
                    if current_nid != Some(target_nid) {
                        log::warn!("Node switched, discarding stale skill_list response");
                        return;
                    }
                    let mut c = cache_mut.write_unchecked();
                    if let Some(d) = c.get_mut(&cache_nid) {
                        if let Some(v) = d.data.get_mut(CACHE_KEY) {
                            if let Some(obj) = v.as_object_mut() {
                                match result {
                                    Ok(entries) => {
                                        let parsed: Vec<SkillsCacheEntry> = entries
                                            .iter()
                                            .map(|e| SkillsCacheEntry {
                                                name: e.name.clone(),
                                                version: e.version.clone(),
                                                scope: e.scope.clone(),
                                                description: e.description.clone(),
                                            })
                                            .collect();
                                        obj.insert(
                                            "skills".to_string(),
                                            serde_json::to_value(parsed).unwrap_or_default(),
                                        );
                                        obj.insert("loading".to_string(), serde_json::json!(false));
                                    }
                                    Err(e) => {
                                        obj.insert("error".to_string(), serde_json::json!(e));
                                        obj.insert("loading".to_string(), serde_json::json!(false));
                                    }
                                }
                            }
                        }
                    }
                });
            }
        });
    });

    // Read state from cache.
    let has_active_node = active_node.read().is_some();
    let (skills, loading, error) = {
        let node_id = active_node.read().clone();
        node_id
            .and_then(|nid| {
                let c = cache.read();
                c.get(&nid).and_then(|d| {
                    d.data
                        .get(CACHE_KEY)
                        .and_then(|v| serde_json::from_value::<SkillsCacheState>(v.clone()).ok())
                })
            })
            .map(|s| (s.skills, s.loading, s.error))
            .unwrap_or_default()
    };

    let count = skills.len();

    // Early return if no node selected.
    if !has_active_node {
        return rsx! {
            div { class: "flex-1 overflow-y-auto p-3",
                div { class: "flex items-center justify-center h-full text-[#666] text-[12px]",
                    "No node selected"
                }
            }
        };
    }

    if let Some(_err) = error {
        let app_retry = app_state.clone();
        return rsx! {
            div { class: "flex-1 overflow-y-auto p-2.5",
                div { class: "flex flex-col items-center justify-center h-full text-[#c04040]",
                    "Failed to load skills"
                    button {
                        class: "mt-2 px-3 py-1 bg-[#4080ff] text-white rounded text-[12px] cursor-pointer hover:bg-[#5090ff]",
                        onclick: move |_| {
                            let node_id = app_retry.active_node_id.read().clone();
                            if let Some(ref nid) = node_id {
                                let client = app_retry
                                    .dp_pool
                                    .read()
                                    .get(nid)
                                    .map(|c| c.client.clone())
                                    .unwrap_or_else(|| app_retry.rpc_client.clone());

                                let mut cache_mut = app_retry.node_data_cache;
                                let target_nid = nid.clone();
                                let cache_nid = nid.clone();

                                // Mark loading.
                                {
                                    let loading_state = SkillsCacheState::default();
                                    let v = serde_json::to_value(&loading_state).unwrap_or_default();
                                    let mut c = cache_mut.write();
                                    let node_data = c.get_or_insert(&cache_nid);
                                    node_data.data.insert(CACHE_KEY.to_string(), v);
                                }

                                client.skill_list(move |result| {
                                    let current_nid = app_retry.active_node_id.read().clone();
                                    if current_nid != Some(target_nid) {
                                        log::warn!("Node switched, discarding stale skill_list response");
                                        return;
                                    }
                                    let mut c = cache_mut.write();
                                    if let Some(d) = c.get_mut(&cache_nid) {
                                        if let Some(v) = d.data.get_mut(CACHE_KEY) {
                                            if let Some(obj) = v.as_object_mut() {
                                                match result {
                                                    Ok(entries) => {
                                                        let parsed: Vec<SkillsCacheEntry> = entries
                                                            .iter()
                                                            .map(|e| SkillsCacheEntry {
                                                                name: e.name.clone(),
                                                                version: e.version.clone(),
                                                                scope: e.scope.clone(),
                                                                description: e.description.clone(),
                                                            })
                                                            .collect();
                                                        obj.insert("skills".to_string(), serde_json::to_value(parsed).unwrap_or_default());
                                                        obj.insert("loading".to_string(), serde_json::json!(false));
                                                    }
                                                    Err(e) => {
                                                        obj.insert("error".to_string(), serde_json::json!(e));
                                                        obj.insert("loading".to_string(), serde_json::json!(false));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                });
                            }
                        },
                        "Retry"
                    }
                }
            }
        };
    }

    if count == 0 && !loading {
        return rsx! { div { class: "flex-1 overflow-y-auto p-2.5", div { class: "flex items-center justify-center h-full text-[#666]", "No skills discovered" } } };
    }

    rsx! {
        div { class: "flex-1 overflow-y-auto p-2.5",
            div { class: "flex items-center justify-between mb-2",
                div { class: "text-[12px] text-[#888]", "Skills ({count})" }
                button {
                    class: "px-2 py-0.5 text-[12px] bg-[#3a3a55] text-[#ccc] rounded hover:bg-[#4a4a65]",
                    onclick: move |_| {
                        let node_id = app_state.active_node_id.read().clone();
                        if let Some(ref nid) = node_id {
                            let client = app_state
                                .dp_pool
                                .read()
                                .get(nid)
                                .map(|c| c.client.clone())
                                .unwrap_or_else(|| app_state.rpc_client.clone());

                            let mut cache_mut = app_state.node_data_cache;
                            let target_nid = nid.clone();
                            let cache_nid = nid.clone();

                            // Mark loading.
                            {
                                let loading_state = SkillsCacheState::default();
                                let v = serde_json::to_value(&loading_state).unwrap_or_default();
                                let mut c = cache_mut.write();
                                let node_data = c.get_or_insert(&cache_nid);
                                node_data.data.insert(CACHE_KEY.to_string(), v);
                            }

                            let app_state_inner = app_state.clone();
                            client.skill_refresh(move |result| {
                                match result {
                                    Ok(_) => {
                                        let current_nid = active_node.read().clone();
                                        if current_nid != Some(target_nid.clone()) {
                                            log::warn!("Node switched, discarding stale skill_list response");
                                            return;
                                        }
                                        // After refresh, re-fetch the list.
                                        let client2 = app_state_inner
                                            .dp_pool
                                            .read()
                                            .get(&target_nid)
                                            .map(|c| c.client.clone())
                                            .unwrap_or_else(|| app_state_inner.rpc_client.clone());
                                        let cache_nid2 = cache_nid.clone();
                                        let mut cache_ref = cache_mut.clone();
                                        client2.skill_list(move |list_result| {
                                            let current_nid = active_node.read().clone();
                                            if current_nid != Some(target_nid) {
                                                return;
                                            }
                                            let mut c = cache_ref.write();
                                            if let Some(d) = c.get_mut(&cache_nid2) {
                                                if let Some(v) = d.data.get_mut(CACHE_KEY) {
                                                    if let Some(obj) = v.as_object_mut() {
                                                        match list_result {
                                                            Ok(entries) => {
                                                                let parsed: Vec<SkillsCacheEntry> = entries
                                                                    .iter()
                                                                    .map(|e| SkillsCacheEntry {
                                                                        name: e.name.clone(),
                                                                        version: e.version.clone(),
                                                                        scope: e.scope.clone(),
                                                                        description: e.description.clone(),
                                                                    })
                                                                    .collect();
                                                                obj.insert("skills".to_string(), serde_json::to_value(parsed).unwrap_or_default());
                                                                obj.insert("loading".to_string(), serde_json::json!(false));
                                                            }
                                                            Err(e) => {
                                                                obj.insert("error".to_string(), serde_json::json!(e));
                                                                obj.insert("loading".to_string(), serde_json::json!(false));
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        });
                                    }
                                    Err(e) => {
                                        let mut c = cache_mut.write();
                                        if let Some(d) = c.get_mut(&cache_nid) {
                                            if let Some(v) = d.data.get_mut(CACHE_KEY) {
                                                if let Some(obj) = v.as_object_mut() {
                                                    obj.insert("error".to_string(), serde_json::json!(e));
                                                    obj.insert("loading".to_string(), serde_json::json!(false));
                                                }
                                            }
                                        }
                                    }
                                }
                            });
                        }
                    },
                    "Refresh"
                }
            }
            if loading {
                div { class: "text-[12px] text-[#888] mb-2", "Loading..." }
            }
            div { class: "sm:hidden flex flex-col gap-2",
                {(0..count).map(|i| {
                    let d = dialog_signal;
                    let app_clone = app_state.clone();
                    let skill = skills[i].clone();
                    rsx! { SkillCard { skill: skill, dialog_signal: d, app_state: app_clone } }
                }).collect::<Vec<Element>>().into_iter()}
            }
            table { class: "hidden sm:table w-full border-collapse",
                thead { tr {
                    th { class: "text-left px-2 py-1 border-b border-[#333355] text-[12px] text-[#888]", "Name" }
                    th { class: "text-left px-2 py-1 border-b border-[#333355] text-[12px] text-[#888]", "Version" }
                    th { class: "text-left px-2 py-1 border-b border-[#333355] text-[12px] text-[#888]", "Scope" }
                    th { class: "text-left px-2 py-1 border-b border-[#333355] text-[12px] text-[#888]", "Description" }
                } }
                tbody {
                    {(0..count).map(|i| { let d = dialog_signal; let app_clone = app_state.clone(); let skill = skills[i].clone(); rsx! { SkillRow { skill: skill, dialog_signal: d, app_state: app_clone } } }).collect::<Vec<Element>>().into_iter()}
                }
            }

            // Capability Overlay section
            div {
                div { class: "flex items-center justify-between mb-2",
                    div { class: "text-[12px] font-semibold text-[#888] uppercase tracking-[0.5px]",
                        "Capability Overlay"
                    }
                }
                let cap_loading = cap_signal.read().loading;
                if cap_loading {
                    div { class: "text-[12px] text-[#888] mb-2", "Loading..." }
                } else {
                    let effective_skills = cap_signal.read().effective_skills.clone();
                    let base_skills = cap_signal.read().base_skills.clone();
                    let is_dirty = cap_dirty.read().clone();
                    let has_data = !base_skills.is_empty() || !effective_skills.is_empty();
                    if has_data {
                        div { class: "flex gap-2 mb-2",
                            button {
                                class: "px-2 py-0.5 text-[12px] bg-[#3a3a55] text-[#ccc] rounded hover:bg-[#4a4a65] disabled:opacity-40 disabled:cursor-not-allowed",
                                disabled: !is_dirty,
                                onclick: {
                                    let sel = selected_skills_signal.clone();
                                    let dirty = cap_dirty.clone();
                                    let cap_sig = cap_signal.clone();
                                    let agents = agents.clone();
                                    let global = global.clone();
                                    move |_| {
                                        let agent_id = agents.read().selected.clone().unwrap_or_default();
                                        let session_id = global.read().session_id.clone();
                                        if agent_id.is_empty() { return; }
                                        let node_id = active_node.read().clone();
                                        let client = node_id
                                            .as_ref()
                                            .and_then(|nid| app_state.dp_pool.read().get(nid).map(|c| c.client.clone()))
                                            .unwrap_or_else(|| app_state.rpc_client.clone());
                                        let skills_list: Vec<String> = sel.read().iter().cloned().collect();
                                        let tools: Vec<String> = Vec::new();
                                        let mcps: Vec<String> = Vec::new();
                                        client.agent_update_capabilities(&agent_id, &session_id, tools, skills_list, mcps, move |result| {
                                            cap_sig.with_mut(|s| {
                                                match result {
                                                    Ok(upd) => {
                                                        s.effective_skills = upd.effective_skills;
                                                        s.dirty = false;
                                                        dirty.set(false);
                                                    }
                                                    Err(e) => {
                                                        log::error!("Failed to update capabilities: {e}");
                                                    }
                                                }
                                            });
                                        });
                                    }
                                },
                                "Apply"
                            }
                            button {
                                class: "px-2 py-0.5 text-[12px] bg-[#3a3a55] text-[#ccc] rounded hover:bg-[#4a4a65]",
                                onclick: {
                                    let sel = selected_skills_signal.clone();
                                    let cap_sig = cap_signal.clone();
                                    let dirty = cap_dirty.clone();
                                    move |_| {
                                        let base = cap_sig.read().base_skills.clone();
                                        let hs: HashSet<String> = base.into_iter().collect();
                                        sel.set(hs);
                                        dirty.set(true);
                                    }
                                },
                                "Reset to default"
                            }
                        }
                        for skill_name in &effective_skills {
                            let is_checked = selected_skills_signal.read().contains(skill_name);
                            div { class: "flex items-center gap-2 py-0.5 px-2",
                                input {
                                    input_type: "checkbox",
                                    checked: is_checked,
                                    oninput: {
                                        let sel = selected_skills_signal.clone();
                                        let dirty = cap_dirty.clone();
                                        let name = skill_name.clone();
                                        move |_| {
                                            let mut hs = sel.write();
                                            if hs.contains(&name) {
                                                hs.remove(&name);
                                            } else {
                                                hs.insert(name.clone());
                                            }
                                            dirty.set(true);
                                        }
                                    },
                                }
                                span { class: "text-[13px] text-[#e0e0e0]", "{skill_name}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn SkillCard(
    skill: SkillsCacheEntry,
    mut dialog_signal: Signal<SkillDialogState>,
    app_state: AppState,
) -> Element {
    let color = match skill.scope.as_str() {
        "User" => "#40c040",
        "Repo" => "#4080ff",
        _ => "#c0c040",
    };

    rsx! {
        div {
            class: "cursor-pointer rounded-md border border-[#333355] bg-[#20203a] p-3 active:bg-[#2a2a44]",
            onclick: move |_| {
                let node_id = app_state.active_node_id.read().clone();
                let client = node_id
                    .as_ref()
                    .and_then(|nid| app_state.dp_pool.read().get(nid).map(|c| c.client.clone()))
                    .unwrap_or_else(|| app_state.rpc_client.clone());
                let name = skill.name.clone();
                let mut d = dialog_signal.write_unchecked();
                d.open = true;
                d.skill = None;
                d.loading = true;
                client.skill_get(&name, move |result| {
                    match result {
                        Ok(detail) => {
                            d.skill = Some(detail);
                        }
                        Err(_) => {
                            d.skill = None;
                        }
                    }
                    d.loading = false;
                });
            },
            div { class: "flex items-start justify-between gap-3",
                div { class: "min-w-0",
                    div { class: "truncate text-[14px] font-bold text-[#e0e0e0]", "{skill.name}" }
                    div { class: "mt-0.5 text-[11px] text-[#777]", "v{skill.version}" }
                }
                span {
                    class: "flex-shrink-0 rounded border border-[#333355] px-2 py-0.5 text-[11px] font-semibold",
                    style: "color: {color};",
                    "{skill.scope}"
                }
            }
            if !skill.description.is_empty() {
                div { class: "mt-2 text-[12px] leading-[1.45] text-[#aaa]", "{skill.description}" }
            }
        }
    }
}

#[component]
fn SkillRow(
    skill: SkillsCacheEntry,
    mut dialog_signal: Signal<SkillDialogState>,
    app_state: AppState,
) -> Element {
    let color = match skill.scope.as_str() {
        "User" => "#40c040",
        "Repo" => "#4080ff",
        _ => "#c0c040",
    };

    rsx! {
        tr {
            class: "cursor-pointer hover:bg-[#2a2a44]",
            onclick: move |_| {
                let node_id = app_state.active_node_id.read().clone();
                let client = node_id
                    .as_ref()
                    .and_then(|nid| app_state.dp_pool.read().get(nid).map(|c| c.client.clone()))
                    .unwrap_or_else(|| app_state.rpc_client.clone());
                let name = skill.name.clone();
                let mut d = dialog_signal.write_unchecked();
                d.open = true;
                d.skill = None;
                d.loading = true;
                client.skill_get(&name, move |result| {
                    match result {
                        Ok(detail) => {
                            d.skill = Some(detail);
                        }
                        Err(_) => {
                            d.skill = None;
                        }
                    }
                    d.loading = false;
                });
            },
            td { class: "px-2 py-1 text-[13px] border-b border-[#2a2a44] text-[#e0e0e0] font-bold", "{skill.name}" }
            td { class: "px-2 py-1 text-[13px] border-b border-[#2a2a44] text-[#888]", "{skill.version}" }
            td { class: "px-2 py-1 text-[13px] border-b border-[#2a2a44]", style: "color: {color};", "{skill.scope}" }
            td { class: "px-2 py-1 text-[13px] border-b border-[#2a2a44] text-[#888]", "{skill.description}" }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn skills_panel_uses_mobile_cards_and_desktop_table() {
        let source = include_str!("skills.rs");
        let mobile_cards = ["sm:hidden", "flex", "flex-col", "gap-2"].join(" ");
        let desktop_table = ["hidden", "sm:table", "w-full"].join(" ");

        assert!(source.contains(&mobile_cards));
        assert!(source.contains(&desktop_table));
    }
}
