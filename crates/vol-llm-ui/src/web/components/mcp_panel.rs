use dioxus::prelude::*;

use crate::state::{McpDialogState, McpState, McpSubtab};
use crate::web::components::app::AppState;

/// Key used to store the serialized McpState in NodeDataCache.
const CACHE_KEY: &str = "mcp_state";

#[component]
pub fn McpPanel() -> Element {
    let app_state: AppState = use_context();
    let active_node = app_state.active_node_id;
    let cache = app_state.node_data_cache;
    let dialog_signal: Signal<McpDialogState> = use_context();

    // Subtab selection persists across cache loads, kept in local signal.
    let active_subtab = use_signal(|| McpSubtab::Servers);

    // Load from cache or trigger DP fetch whenever active_node changes.
    let app_state_for_effect = app_state.clone();
    use_effect(move || {
        let node_id = active_node.read().clone();
        if let Some(ref nid) = node_id {
            let cached = {
                let c = cache.read();
                c.get(nid).and_then(|d| d.data.get(CACHE_KEY).cloned())
            };

            if cached.is_some() {
                // Already cached — nothing to fetch.
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

            // Mark as loading in cache immediately so render shows spinner.
            {
                let loading_state = McpState::new();
                let v = serde_json::to_value(&loading_state).unwrap_or_default();
                let mut c = cache_mut.write();
                let node_data = c.get_or_insert(&cache_nid);
                node_data.data.insert(CACHE_KEY.to_string(), v);
            }

            // Fire all five MCP list calls in parallel.  Each writes its slice
            // into the shared McpState inside the cache; the last one to
            // finish clears the loading flag.
            let remaining = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(5));

            let finish_one = {
                let remaining = remaining.clone();
                let cache_nid = cache_nid.clone();
                let mut cache_ref = cache_mut;
                move || {
                    if remaining.fetch_sub(1, std::sync::atomic::Ordering::SeqCst) == 1 {
                        // Last callback — clear loading flag.
                        let mut c = cache_ref.write();
                        if let Some(d) = c.get_mut(&cache_nid) {
                            if let Some(v) = d.data.get_mut(CACHE_KEY) {
                                if let Some(obj) = v.as_object_mut() {
                                    obj.insert("loading".to_string(), serde_json::json!(false));
                                }
                            }
                        }
                    }
                }
            };

            // --- servers ---
            {
                let client = client.clone();
                let cache_nid = cache_nid.clone();
                let mut cache_ref = cache_mut.clone();
                let target = target_nid.clone();
                let mut done = finish_one.clone();
                client.mcp_list_servers(move |result| {
                    let current = active_node.read().clone();
                    if current != Some(target) {
                        log::warn!("Node switched, discarding stale mcp_list_servers response");
                        return;
                    }
                    let mut c = cache_ref.write();
                    if let Some(d) = c.get_mut(&cache_nid) {
                        if let Some(v) = d.data.get_mut(CACHE_KEY) {
                            if let Some(obj) = v.as_object_mut() {
                                match result {
                                    Ok(servers) => {
                                        obj.insert(
                                            "servers".to_string(),
                                            serde_json::to_value(servers).unwrap_or_default(),
                                        );
                                    }
                                    Err(e) => {
                                        obj.insert("error".to_string(), serde_json::json!(e));
                                    }
                                }
                            }
                        }
                    }
                    done();
                });
            }

            // --- tools ---
            {
                let client = client.clone();
                let cache_nid = cache_nid.clone();
                let mut cache_ref = cache_mut.clone();
                let target = target_nid.clone();
                let mut done = finish_one.clone();
                client.mcp_list_tools(None, move |result| {
                    let current = active_node.read().clone();
                    if current != Some(target) {
                        log::warn!("Node switched, discarding stale mcp_list_tools response");
                        return;
                    }
                    if let Ok(tools) = result {
                        let mut c = cache_ref.write();
                        if let Some(d) = c.get_mut(&cache_nid) {
                            if let Some(v) = d.data.get_mut(CACHE_KEY) {
                                if let Some(obj) = v.as_object_mut() {
                                    obj.insert(
                                        "tools".to_string(),
                                        serde_json::to_value(tools).unwrap_or_default(),
                                    );
                                }
                            }
                        }
                    }
                    done();
                });
            }

            // --- resources ---
            {
                let client = client.clone();
                let cache_nid = cache_nid.clone();
                let mut cache_ref = cache_mut.clone();
                let target = target_nid.clone();
                let mut done = finish_one.clone();
                client.mcp_list_resources(None, move |result| {
                    let current = active_node.read().clone();
                    if current != Some(target) {
                        log::warn!("Node switched, discarding stale mcp_list_resources response");
                        return;
                    }
                    if let Ok(resources) = result {
                        let mut c = cache_ref.write();
                        if let Some(d) = c.get_mut(&cache_nid) {
                            if let Some(v) = d.data.get_mut(CACHE_KEY) {
                                if let Some(obj) = v.as_object_mut() {
                                    obj.insert(
                                        "resources".to_string(),
                                        serde_json::to_value(resources).unwrap_or_default(),
                                    );
                                }
                            }
                        }
                    }
                    done();
                });
            }

            // --- resource_templates ---
            {
                let client = client.clone();
                let cache_nid = cache_nid.clone();
                let mut cache_ref = cache_mut.clone();
                let target = target_nid.clone();
                let mut done = finish_one.clone();
                client.mcp_list_resource_templates(None, move |result| {
                    let current = active_node.read().clone();
                    if current != Some(target) {
                        log::warn!(
                            "Node switched, discarding stale mcp_list_resource_templates response"
                        );
                        return;
                    }
                    if let Ok(templates) = result {
                        let mut c = cache_ref.write();
                        if let Some(d) = c.get_mut(&cache_nid) {
                            if let Some(v) = d.data.get_mut(CACHE_KEY) {
                                if let Some(obj) = v.as_object_mut() {
                                    obj.insert(
                                        "resource_templates".to_string(),
                                        serde_json::to_value(templates).unwrap_or_default(),
                                    );
                                }
                            }
                        }
                    }
                    done();
                });
            }

            // --- prompts ---
            {
                let cache_nid = cache_nid.clone();
                let mut cache_ref = cache_mut;
                let target = target_nid;
                let mut done = finish_one;
                client.mcp_list_prompts(None, move |result| {
                    let current = active_node.read().clone();
                    if current != Some(target) {
                        log::warn!("Node switched, discarding stale mcp_list_prompts response");
                        return;
                    }
                    if let Ok(prompts) = result {
                        let mut c = cache_ref.write();
                        if let Some(d) = c.get_mut(&cache_nid) {
                            if let Some(v) = d.data.get_mut(CACHE_KEY) {
                                if let Some(obj) = v.as_object_mut() {
                                    obj.insert(
                                        "prompts".to_string(),
                                        serde_json::to_value(prompts).unwrap_or_default(),
                                    );
                                }
                            }
                        }
                    }
                    done();
                });
            }
        }
    });

    // Read McpState from cache for the active node.
    let mcp_data: Option<McpStateJson> = {
        let node_id = active_node.read().clone();
        node_id.and_then(|nid| {
            let c = cache.read();
            c.get(&nid)
                .and_then(|d| d.data.get(CACHE_KEY).cloned())
                .and_then(|v| serde_json::from_value::<McpStateJson>(v).ok())
        })
    };

    let (loading, error) = match &mcp_data {
        Some(d) => (d.loading, d.error.clone()),
        None => (false, None),
    };

    // Always use the local signal for subtab — the cache can lag.
    let display_active = active_subtab.read().clone();

    rsx! {
        div { class: "flex-1 overflow-y-auto p-2",
            if active_node.read().is_none() {
                div { class: "text-[#666] text-center p-4 text-[13px]", "No node selected" }
            } else if mcp_data.is_none() {
                // Brief flash before first effect runs.
                div { class: "text-[#666] text-center p-4 text-[13px]", "Loading MCP data..." }
            } else if loading {
                div { class: "text-[#666] text-center p-4 text-[13px]", "Loading MCP data..." }
            } else {
                div {
                    // Sub-tab buttons
                    div { class: "flex gap-1 mb-2",
                        McpSubtabButton { active_subtab, subtab: McpSubtab::Servers, label: "Servers" }
                        McpSubtabButton { active_subtab, subtab: McpSubtab::Tools, label: "Tools" }
                        McpSubtabButton { active_subtab, subtab: McpSubtab::Resources, label: "Resources" }
                        McpSubtabButton { active_subtab, subtab: McpSubtab::Prompts, label: "Prompts" }
                    }
                    // Sub-tab content
                    match display_active {
                        McpSubtab::Servers => rsx! { ServerList { app_state: app_state.clone(), cache, active_node, error } },
                        McpSubtab::Tools => rsx! { ToolList { mcp_data: mcp_data.clone(), dialog_signal } },
                        McpSubtab::Resources => rsx! { ResourceList { mcp_data: mcp_data.clone(), dialog_signal } },
                        McpSubtab::Prompts => rsx! { PromptList { mcp_data: mcp_data.clone(), dialog_signal } },
                    }
                }
            }

        }
    }
}

/// Deserializable subset of McpState — used to read cached JSON.
/// We do NOT deserialize `active_subtab` / `loading` / `error` into McpState;
/// those are handled separately.
#[derive(serde::Deserialize, Clone, Debug, PartialEq)]
struct McpStateJson {
    #[serde(default)]
    servers: Vec<crate::state::McpServerInfo>,
    #[serde(default)]
    tools: Vec<crate::state::McpToolInfo>,
    #[serde(default)]
    resources: Vec<crate::state::McpResourceInfo>,
    #[serde(default)]
    resource_templates: Vec<crate::state::McpResourceTemplateInfo>,
    #[serde(default)]
    prompts: Vec<crate::state::McpPromptInfo>,
    #[serde(default)]
    loading: bool,
    #[serde(default)]
    error: Option<String>,
}

#[component]
fn McpSubtabButton(
    mut active_subtab: Signal<McpSubtab>,
    subtab: McpSubtab,
    label: String,
) -> Element {
    let active = active_subtab.read().clone() == subtab;
    let class = if active {
        "px-3 py-1 bg-[#1a1a2e] text-[#e0e0e0] rounded text-[12px] cursor-pointer border border-[#80a0ff]"
    } else {
        "px-3 py-1 bg-transparent text-[#888] rounded text-[12px] cursor-pointer hover:text-[#ccc] hover:bg-[#2a2a44]"
    };
    rsx! {
        button {
            class,
            onclick: move |_| { *active_subtab.write_unchecked() = subtab; },
            "{label}"
        }
    }
}

#[component]
fn ServerList(
    app_state: AppState,
    cache: Signal<crate::state::NodeDataCache>,
    active_node: Signal<Option<String>>,
    error: Option<String>,
) -> Element {
    let (servers, reconnecting): (Vec<crate::state::McpServerInfo>, Vec<String>) = {
        let node_id = active_node.read().clone();
        node_id
            .and_then(|nid| {
                let c = cache.read();
                c.get(&nid).and_then(|d| {
                    let v = d.data.get(CACHE_KEY)?;
                    let servers: Vec<crate::state::McpServerInfo> = v
                        .get("servers")
                        .and_then(|sv| serde_json::from_value(sv.clone()).ok())
                        .unwrap_or_default();
                    let reconnecting: Vec<String> = v
                        .get("reconnecting")
                        .and_then(|rv| serde_json::from_value(rv.clone()).ok())
                        .unwrap_or_default();
                    Some((servers, reconnecting))
                })
            })
            .unwrap_or_default()
    };

    if servers.is_empty() && error.is_none() {
        return rsx! {
            div { class: "text-[#666] text-center p-4 text-[13px]", "No MCP servers configured" }
        };
    }

    let desktop_servers = servers;
    let reconnect_list = reconnecting;
    let cache_error: Option<String> = {
        let node_id = active_node.read().clone();
        node_id
            .and_then(|nid| {
                let c = cache.read();
                c.get(&nid).and_then(|d| {
                    let v = d.data.get(CACHE_KEY)?;
                    v.get("error")
                        .and_then(|ev| ev.as_str())
                        .map(|s| s.to_string())
                })
            })
            .or(error)
    };

    rsx! {
        div {
            // Mobile: server cards
            div { class: "sm:hidden flex flex-col gap-2",
                {desktop_servers.iter().map(|s| {
                    let status_color = match s.status.as_str() {
                        "connected" => "#40c040",
                        "connecting" => "#f0c040",
                        "disconnected" => "#888",
                        _ => "#c04040",
                    };
                    let show_reconnect = s.status != "connected" && s.status != "connecting";
                    let is_reconnecting = reconnect_list.contains(&s.name);
                    let srv_name = s.name.clone();
                    let srv_status = if is_reconnecting { "Reconnecting...".to_string() } else { s.status.clone() };
                    let app = app_state.clone();
                    rsx! {
                        div { class: "rounded-lg border border-[#333355] bg-[#20203a] p-3",
                            div { class: "flex items-center justify-between",
                                div { class: "flex items-center gap-2 min-w-0",
                                    span { class: "w-2 h-2 rounded-full flex-shrink-0", style: "background-color: {status_color};" }
                                    span { class: "text-[13px] text-[#e0e0e0] truncate", "{srv_name}" }
                                }
                                if is_reconnecting {
                                    span { class: "text-[11px] text-[#f0c040] animate-pulse flex-shrink-0 ml-2", "{srv_status}" }
                                } else {
                                    span { class: "text-[11px] text-[#666] flex-shrink-0 ml-2", "{srv_status}" }
                                }
                            }
                            if show_reconnect && !is_reconnecting {
                                button {
                                    class: "mt-2 w-full px-2 py-1 bg-[#2a2a44] text-[#aaa] rounded text-[11px] hover:text-[#e0e0e0]",
                                    onclick: move |_| {
                                        let name = srv_name.clone();
                                        let app = app.clone();
                                        do_mcp_reconnect(app, &name);
                                    },
                                    "Reconnect"
                                }
                            } else if is_reconnecting {
                                span { class: "mt-2 block text-center text-[11px] text-[#888] animate-pulse", "..." }
                            }
                        }
                    }
                }).collect::<Vec<Element>>().into_iter()}
            }
            // Desktop: server rows
            div { class: "hidden sm:block font-mono text-[13px]",
                {desktop_servers.into_iter().map(|s| {
                    let is_reconnecting = reconnect_list.contains(&s.name);
                    let app = app_state.clone();
                    rsx! { ServerRow { app_state: app, server: s, reconnecting: is_reconnecting } }
                }).collect::<Vec<Element>>().into_iter()}
                if let Some(ref e) = cache_error {
                    div { class: "text-[#c04040] p-2 text-[12px] bg-[#2a1a1a] border border-[#c04040] rounded mt-2", "Error: {e}" }
                }
            }
        }
    }
}

/// Trigger a reconnect via the DP client (falling back to CP), then re-fetch
/// servers + tools + resources + templates + prompts and write the results
/// back into NodeDataCache.
fn do_mcp_reconnect(app: AppState, server_name: &str) {
    let nid = app.active_node_id.read().clone();
    let Some(ref nid_str) = nid else {
        return;
    };
    let client = nid
        .as_ref()
        .and_then(|id| app.dp_pool.read().get(id).map(|c| c.client.clone()))
        .unwrap_or_else(|| app.rpc_client.clone());

    let name = server_name.to_string();
    let target_nid = nid.clone();
    let cache_mut = app.node_data_cache;
    let mut cache_mut = cache_mut; // WASM: Signal::write needs &mut self
    let cache_nid = nid_str.clone();

    // Set reconnecting state so the button shows loading.
    {
        let mut c = cache_mut.write();
        if let Some(d) = c.get_mut(&cache_nid) {
            if let Some(v) = d.data.get_mut(CACHE_KEY) {
                if let Some(obj) = v.as_object_mut() {
                    let reconnecting: Vec<String> = obj
                        .get("reconnecting")
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                        .unwrap_or_default();
                    let mut new_list = reconnecting;
                    if !new_list.contains(&name) {
                        new_list.push(name.clone());
                    }
                    obj.insert(
                        "reconnecting".to_string(),
                        serde_json::to_value(new_list).unwrap_or_default(),
                    );
                }
            }
        }
    }

    // Build all clones for the move callback BEFORE calling mcp_reconnect.
    let cb_client = client.clone();
    let cb_name = name.clone();
    let cb_cache_mut = cache_mut;
    let cb_app = app.clone();
    let cb_cache_nid = cache_nid.clone();
    let cb_target_nid = target_nid.clone();

    let reconnect_name = name.clone();
    client.mcp_reconnect(&reconnect_name, move |result| {
        let mut cache_mut = cb_cache_mut;
        let name = cb_name;
        let client = cb_client;
        let app = cb_app;
        let cache_nid = cb_cache_nid;
        let target_nid = cb_target_nid;
        // Clear reconnecting state regardless of outcome.
        {
            let mut c = cache_mut.write();
            if let Some(d) = c.get_mut(&cache_nid) {
                if let Some(v) = d.data.get_mut(CACHE_KEY) {
                    if let Some(obj) = v.as_object_mut() {
                        let reconnecting: Vec<String> = obj
                            .get("reconnecting")
                            .and_then(|v| serde_json::from_value(v.clone()).ok())
                            .unwrap_or_default();
                        let new_list: Vec<String> =
                            reconnecting.into_iter().filter(|n| n != &name).collect();
                        obj.insert(
                            "reconnecting".to_string(),
                            serde_json::to_value(new_list).unwrap_or_default(),
                        );
                    }
                }
            }
        }

        match result {
            Ok(true) => {
                log::info!("MCP server '{}' reconnected successfully", name);
            }
            Ok(false) => {
                log::warn!("MCP server '{}' reconnect returned false", name);
                // Write error into cache so the UI can show it.
                let mut c = cache_mut.write();
                if let Some(d) = c.get_mut(&cache_nid) {
                    if let Some(v) = d.data.get_mut(CACHE_KEY) {
                        if let Some(obj) = v.as_object_mut() {
                            obj.insert(
                                "error".to_string(),
                                serde_json::json!(format!("Reconnect failed for '{}'", name)),
                            );
                        }
                    }
                }
                return;
            }
            Err(e) => {
                log::error!("MCP server '{}' reconnect error: {}", name, e);
                let mut c = cache_mut.write();
                if let Some(d) = c.get_mut(&cache_nid) {
                    if let Some(v) = d.data.get_mut(CACHE_KEY) {
                        if let Some(obj) = v.as_object_mut() {
                            obj.insert(
                                "error".to_string(),
                                serde_json::json!(format!("Reconnect error for '{}': {}", name, e)),
                            );
                        }
                    }
                }
                return;
            }
        }

        let current_nid = app.active_node_id.read().clone();
        if current_nid != target_nid {
            log::warn!("Node switched, discarding stale mcp_reconnect result");
            return;
        }

        // ── Re-fetch all MCP data ──────────────────────────────────
        let remaining = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(5));
        let finish_one = {
            let cache_nid = cache_nid.clone();
            let mut cache_ref = cache_mut.clone();
            let target = target_nid.clone();
            let remaining = remaining.clone();
            move || {
                let mut cache_ref = cache_ref;
                if remaining.fetch_sub(1, std::sync::atomic::Ordering::SeqCst) == 1 {
                    let mut c = cache_ref.write();
                    if let Some(d) = c.get_mut(&cache_nid) {
                        if let Some(v) = d.data.get_mut(CACHE_KEY) {
                            if let Some(obj) = v.as_object_mut() {
                                obj.insert("error".to_string(), serde_json::Value::Null);
                            }
                        }
                    }
                }
            }
        };

        // servers
        {
            let client = client.clone();
            let cache_nid_s = cache_nid.clone();
            let mut cache_ref = cache_mut.clone();
            let target = target_nid.clone();
            let done = finish_one.clone();
            client.mcp_list_servers(move |r| {
                let mut done = done;
                let mut cache_ref = cache_ref;
                if app.active_node_id.read().clone() != target {
                    return;
                }
                if let Ok(servers) = r {
                    let mut c = cache_ref.write();
                    if let Some(d) = c.get_mut(&cache_nid_s) {
                        if let Some(v) = d.data.get_mut(CACHE_KEY) {
                            if let Some(obj) = v.as_object_mut() {
                                obj.insert(
                                    "servers".to_string(),
                                    serde_json::to_value(servers).unwrap_or_default(),
                                );
                            }
                        }
                    }
                }
                done();
            });
        }

        // tools
        {
            let client = client.clone();
            let cache_nid_t = cache_nid.clone();
            let mut cache_ref = cache_mut.clone();
            let target = target_nid.clone();
            let done = finish_one.clone();
            client.mcp_list_tools(None, move |r| {
                if app.active_node_id.read().clone() != target {
                    return;
                }
                if let Ok(tools) = r {
                    let mut c = cache_ref.write();
                    if let Some(d) = c.get_mut(&cache_nid_t) {
                        if let Some(v) = d.data.get_mut(CACHE_KEY) {
                            if let Some(obj) = v.as_object_mut() {
                                obj.insert(
                                    "tools".to_string(),
                                    serde_json::to_value(tools).unwrap_or_default(),
                                );
                            }
                        }
                    }
                }
                done();
            });
        }

        // resources
        {
            let client = client.clone();
            let cache_nid_r = cache_nid.clone();
            let mut cache_ref = cache_mut.clone();
            let target = target_nid.clone();
            let done = finish_one.clone();
            client.mcp_list_resources(None, move |r| {
                if app.active_node_id.read().clone() != target {
                    return;
                }
                if let Ok(resources) = r {
                    let mut c = cache_ref.write();
                    if let Some(d) = c.get_mut(&cache_nid_r) {
                        if let Some(v) = d.data.get_mut(CACHE_KEY) {
                            if let Some(obj) = v.as_object_mut() {
                                obj.insert(
                                    "resources".to_string(),
                                    serde_json::to_value(resources).unwrap_or_default(),
                                );
                            }
                        }
                    }
                }
                done();
            });
        }

        // resource_templates
        {
            let client = client.clone();
            let cache_nid_rt = cache_nid.clone();
            let mut cache_ref = cache_mut.clone();
            let target = target_nid.clone();
            let done = finish_one.clone();
            client.mcp_list_resource_templates(None, move |r| {
                if app.active_node_id.read().clone() != target {
                    return;
                }
                if let Ok(templates) = r {
                    let mut c = cache_ref.write();
                    if let Some(d) = c.get_mut(&cache_nid_rt) {
                        if let Some(v) = d.data.get_mut(CACHE_KEY) {
                            if let Some(obj) = v.as_object_mut() {
                                obj.insert(
                                    "resource_templates".to_string(),
                                    serde_json::to_value(templates).unwrap_or_default(),
                                );
                            }
                        }
                    }
                }
                done();
            });
        }

        // prompts
        {
            let cache_nid_p = cache_nid;
            let mut cache_ref = cache_mut;
            let target = target_nid;
            let done = finish_one;
            client.mcp_list_prompts(None, move |r| {
                if app.active_node_id.read().clone() != target {
                    return;
                }
                if let Ok(prompts) = r {
                    let mut c = cache_ref.write();
                    if let Some(d) = c.get_mut(&cache_nid_p) {
                        if let Some(v) = d.data.get_mut(CACHE_KEY) {
                            if let Some(obj) = v.as_object_mut() {
                                obj.insert(
                                    "prompts".to_string(),
                                    serde_json::to_value(prompts).unwrap_or_default(),
                                );
                            }
                        }
                    }
                }
                done();
            });
        }
    });
}

#[component]
fn ServerRow(
    app_state: AppState,
    server: crate::state::McpServerInfo,
    reconnecting: bool,
) -> Element {
    let status_color = match server.status.as_str() {
        "connected" => "#40c040",
        "connecting" => "#f0c040",
        "disconnected" => "#888",
        _ => "#c04040",
    };
    let show_reconnect = server.status != "connected" && server.status != "connecting";

    rsx! {
        div { class: "flex items-center justify-between py-1.5 border-b border-[#2a2a44]",
            div { class: "flex items-center gap-2 min-w-0",
                span { class: "w-2 h-2 rounded-full inline-block flex-shrink-0", style: "background-color: {status_color};" }
                span { class: "text-[13px] text-[#e0e0e0] truncate", "{server.name}" }
                if reconnecting {
                    span { class: "text-[11px] text-[#f0c040] animate-pulse flex-shrink-0", "Reconnecting..." }
                } else {
                    span { class: "text-[11px] text-[#666] flex-shrink-0", "{server.status}" }
                }
            }
            if show_reconnect && !reconnecting {
                button {
                    class: "px-2 py-0.5 bg-[#2a2a44] text-[#aaa] rounded text-[11px] cursor-pointer hover:text-[#e0e0e0]",
                    onclick: move |_| {
                        let srv = server.name.clone();
                        let app = app_state.clone();
                        do_mcp_reconnect(app, &srv);
                    },
                    "Reconnect"
                }
            } else if reconnecting {
                span { class: "text-[11px] text-[#888] animate-pulse flex-shrink-0", "..." }
            }
        }
    }
}

#[component]
fn ToolList(mcp_data: Option<McpStateJson>, dialog_signal: Signal<McpDialogState>) -> Element {
    let tools = mcp_data.map(|d| d.tools).unwrap_or_default();
    log::info!("ToolList rendering: {} tools", tools.len());
    if tools.is_empty() {
        return rsx! {
            div { class: "text-[#666] text-center p-4 text-[13px]", "No tools available" }
        };
    }

    let mut groups: std::collections::BTreeMap<String, Vec<_>> = std::collections::BTreeMap::new();
    for t in &tools {
        groups.entry(t.server.clone()).or_default().push(t.clone());
    }

    rsx! {
        div {
            // Mobile: compact tool cards (no server grouping)
            div { class: "sm:hidden flex flex-col gap-2",
                {tools.iter().map(|t| {
                    let dsig = dialog_signal.clone();
                    let tool = t.clone();
                    rsx! {
                        div { class: "rounded-lg border border-[#333355] bg-[#20203a] p-3",
                            div { class: "flex items-center justify-between",
                                div { class: "min-w-0",
                                    div { class: "truncate text-[14px] font-bold text-[#e0e0e0]", "{tool.name}" }
                                    div { class: "text-[11px] text-[#666] mt-0.5", "{tool.server}" }
                                    if let Some(ref desc) = tool.description {
                                        div { class: "text-[11px] text-[#777] truncate mt-0.5", "{desc}" }
                                    }
                                }
                                button {
                                    class: "px-2 py-0.5 text-[11px] bg-[#4080ff] text-white rounded hover:bg-[#5090ff] flex-shrink-0 ml-2",
                                    onclick: move |_| {
                                        let t = tool.clone();
                                        dsig.write_unchecked().tool_call_dialog = Some(crate::state::McpToolCallState {
                                            server: t.server.clone(),
                                            tool_name: t.name.clone(),
                                            arguments_json: t.input_schema.as_ref().map(|v| serde_json::to_string_pretty(v).unwrap_or_default()).unwrap_or_else(|| "{}".to_string()),
                                            input_schema: t.input_schema.clone(),
                                            result: None,
                                            error: None,
                                            loading: false,
                                        });
                                    },
                                    "Call"
                                }
                            }
                        }
                    }
                }).collect::<Vec<Element>>().into_iter()}
            }
            // Desktop: grouped layout
            div { class: "hidden sm:block font-mono text-[13px]",
                {groups.into_iter().map(|(server, tools)| {
                    rsx! {
                        div { class: "mb-2",
                            div { class: "text-[12px] text-[#888] font-semibold mb-1", "{server} ({tools.len()} tools)" }
                            {tools.into_iter().map(|t| {
                                let dsig = dialog_signal.clone();
                                rsx! { ToolCard { signal: dsig, tool: t } }
                            }).collect::<Vec<Element>>().into_iter()}
                        }
                    }
                }).collect::<Vec<Element>>().into_iter()}
            }
        }
    }
}

#[component]
fn ToolCard(mut signal: Signal<McpDialogState>, tool: crate::state::McpToolInfo) -> Element {
    rsx! {
        div { class: "bg-[#252540] rounded p-2 mb-1",
            div { class: "flex items-center justify-between",
                div {
                    div { class: "text-[13px] text-[#e0e0e0]", "{tool.name}" }
                    if let Some(ref desc) = tool.description {
                        div { class: "text-[11px] text-[#888]", "{desc}" }
                    }
                }
                button {
                    class: "px-2 py-0.5 bg-[#3a3a55] text-[#aaa] rounded text-[11px] cursor-pointer hover:text-[#e0e0e0]",
                    onclick: move |_| {
                        let t = tool.clone();
                        signal.write_unchecked().tool_call_dialog = Some(crate::state::McpToolCallState {
                            server: t.server.clone(),
                            tool_name: t.name.clone(),
                            arguments_json: t.input_schema.as_ref().map(|v| serde_json::to_string_pretty(v).unwrap_or_default()).unwrap_or_else(|| "{}".to_string()),
                            input_schema: t.input_schema.clone(),
                            result: None,
                            error: None,
                            loading: false,
                        });
                    },
                    "Call"
                }
            }
        }
    }
}

#[component]
fn ResourceList(mcp_data: Option<McpStateJson>, dialog_signal: Signal<McpDialogState>) -> Element {
    let resources = mcp_data
        .as_ref()
        .map(|d| d.resources.clone())
        .unwrap_or_default();
    let templates = mcp_data
        .as_ref()
        .map(|d| d.resource_templates.clone())
        .unwrap_or_default();
    if resources.is_empty() && templates.is_empty() {
        return rsx! {
            div { class: "text-[#666] text-center p-4 text-[13px]", "No resources available" }
        };
    }

    let mut resource_groups: std::collections::BTreeMap<String, Vec<_>> =
        std::collections::BTreeMap::new();
    for r in &resources {
        resource_groups
            .entry(r.server.clone())
            .or_default()
            .push(r.clone());
    }

    let mut template_groups: std::collections::BTreeMap<String, Vec<_>> =
        std::collections::BTreeMap::new();
    for t in &templates {
        template_groups
            .entry(t.server.clone())
            .or_default()
            .push(t.clone());
    }

    let all_servers: std::collections::BTreeSet<String> = resource_groups
        .keys()
        .chain(template_groups.keys())
        .cloned()
        .collect();

    rsx! {
        div {
            // Mobile: resource + template cards (flat list)
            div { class: "sm:hidden flex flex-col gap-2",
                {resources.iter().map(|r| {
                    let dsig = dialog_signal.clone();
                    let res = r.clone();
                    rsx! {
                        div { class: "rounded-lg border border-[#333355] bg-[#20203a] p-3",
                            div { class: "flex items-center justify-between",
                                div { class: "min-w-0 flex-1",
                                    div { class: "text-[13px] text-[#e0e0e0] truncate", "{res.name}" }
                                    div { class: "text-[11px] text-[#666] font-mono truncate mt-0.5", "{res.uri}" }
                                }
                                button {
                                    class: "px-2 py-0.5 text-[11px] bg-[#4080ff] text-white rounded hover:bg-[#5090ff] flex-shrink-0 ml-2",
                                    onclick: move |_| {
                                        let r = res.clone();
                                        dsig.write_unchecked().resource_viewer = Some(crate::state::McpResourceViewerState {
                                            uri: r.uri.clone(),
                                            content: None,
                                            error: None,
                                            loading: false,
                                        });
                                    },
                                    "Read"
                                }
                            }
                        }
                    }
                }).collect::<Vec<Element>>().into_iter()}
                {templates.iter().map(|t| {
                    rsx! {
                        div { class: "rounded-lg border border-[#333355] bg-[#20203a] p-3",
                            div { class: "flex items-center justify-between",
                                div { class: "min-w-0 flex-1",
                                    div { class: "text-[13px] text-[#e0e0e0]", "{t.name}" }
                                    div { class: "text-[11px] text-[#666] font-mono truncate mt-0.5", "{t.uri_template}" }
                                }
                                span { class: "text-[10px] bg-[#2a2a44] text-[#888] px-1.5 py-0.5 rounded flex-shrink-0 ml-2", "tmpl" }
                            }
                        }
                    }
                }).collect::<Vec<Element>>().into_iter()}
            }
            // Desktop: grouped layout
            div { class: "hidden sm:block font-mono text-[13px]",
                {all_servers.into_iter().map(|server| {
                    let dsig = dialog_signal.clone();
                    let res = resource_groups.remove(&server).unwrap_or_default();
                    let tmp = template_groups.remove(&server).unwrap_or_default();
                    let total = res.len() + tmp.len();
                    rsx! {
                        div { class: "mb-2",
                            div { class: "text-[12px] text-[#888] font-semibold mb-1", "{server} ({total} items)" }
                            {res.into_iter().map(|r| {
                                let dsig = dsig.clone();
                                rsx! { ResourceRow { signal: dsig, resource: r } }
                            }).collect::<Vec<Element>>().into_iter()}
                            {tmp.into_iter().map(|t| {
                                rsx! { TemplateRow { template: t } }
                            }).collect::<Vec<Element>>().into_iter()}
                        }
                    }
                }).collect::<Vec<Element>>().into_iter()}
            }
        }
    }
}

#[component]
fn ResourceRow(
    mut signal: Signal<McpDialogState>,
    resource: crate::state::McpResourceInfo,
) -> Element {
    rsx! {
        div { class: "flex items-center justify-between py-1 border-b border-[#2a2a44]",
            div { class: "flex-1 min-w-0",
                div { class: "text-[13px] text-[#e0e0e0] truncate", "{resource.name}" }
                div { class: "text-[11px] text-[#666] truncate", "{resource.uri}" }
            }
            button {
                class: "px-2 py-0.5 bg-[#3a3a55] text-[#aaa] rounded text-[11px] cursor-pointer hover:text-[#e0e0e0] ml-2 flex-shrink-0",
                onclick: move |_| {
                    let r = resource.clone();
                    signal.write_unchecked().resource_viewer = Some(crate::state::McpResourceViewerState {
                        uri: r.uri.clone(),
                        content: None,
                        error: None,
                        loading: false,
                    });
                },
                "Read"
            }
        }
    }
}

#[component]
fn TemplateRow(template: crate::state::McpResourceTemplateInfo) -> Element {
    rsx! {
        div { class: "flex items-center py-1 border-b border-[#2a2a44] text-[#888]",
            div { class: "flex-1 min-w-0",
                div { class: "text-[13px]", "{template.name}" }
                div { class: "text-[11px] text-[#666] truncate", "{template.uri_template}" }
            }
            span { class: "text-[10px] bg-[#2a2a44] px-1 rounded ml-2", "template" }
        }
    }
}

#[component]
fn PromptList(mcp_data: Option<McpStateJson>, dialog_signal: Signal<McpDialogState>) -> Element {
    let prompts = mcp_data.map(|d| d.prompts).unwrap_or_default();
    if prompts.is_empty() {
        return rsx! {
            div { class: "text-[#666] text-center p-4 text-[13px]", "No prompts available" }
        };
    }

    let mut groups: std::collections::BTreeMap<String, Vec<_>> = std::collections::BTreeMap::new();
    for p in &prompts {
        groups.entry(p.server.clone()).or_default().push(p.clone());
    }

    rsx! {
        div {
            // Mobile: prompt cards
            div { class: "sm:hidden flex flex-col gap-2",
                {prompts.iter().map(|p| {
                    let dsig = dialog_signal.clone();
                    let prompt = p.clone();
                    rsx! {
                        div { class: "rounded-lg border border-[#333355] bg-[#20203a] p-3",
                            div { class: "flex items-center justify-between",
                                div { class: "min-w-0",
                                    div { class: "truncate text-[14px] font-bold text-[#e0e0e0]", "{prompt.name}" }
                                    div { class: "text-[11px] text-[#666] mt-0.5", "{prompt.server}" }
                                    if let Some(ref desc) = prompt.description {
                                        div { class: "text-[11px] text-[#777] truncate mt-0.5", "{desc}" }
                                    }
                                }
                                button {
                                    class: "px-2 py-0.5 text-[11px] bg-[#4080ff] text-white rounded hover:bg-[#5090ff] flex-shrink-0 ml-2",
                                    onclick: move |_| {
                                        let p = prompt.clone();
                                        dsig.write_unchecked().prompt_viewer = Some(crate::state::McpPromptViewerState {
                                            server: p.server.clone(),
                                            prompt_name: p.name.clone(),
                                            args_json: "{}".to_string(),
                                            result: None,
                                            error: None,
                                            loading: false,
                                        });
                                    },
                                    "Get"
                                }
                            }
                        }
                    }
                }).collect::<Vec<Element>>().into_iter()}
            }
            // Desktop: grouped layout
            div { class: "hidden sm:block font-mono text-[13px]",
                {groups.into_iter().map(|(server, prompts)| {
                    rsx! {
                        div { class: "mb-2",
                            div { class: "text-[12px] text-[#888] font-semibold mb-1", "{server} ({prompts.len()} prompts)" }
                            {prompts.into_iter().map(|p| {
                                let dsig = dialog_signal.clone();
                                rsx! { PromptRow { signal: dsig, prompt: p } }
                            }).collect::<Vec<Element>>().into_iter()}
                        }
                    }
                }).collect::<Vec<Element>>().into_iter()}
            }
        }
    }
}

#[component]
fn PromptRow(mut signal: Signal<McpDialogState>, prompt: crate::state::McpPromptInfo) -> Element {
    rsx! {
        div { class: "flex items-center justify-between py-1 border-b border-[#2a2a44]",
            div {
                div { class: "text-[13px] text-[#e0e0e0]", "{prompt.name}" }
                if let Some(ref desc) = prompt.description {
                    div { class: "text-[11px] text-[#888]", "{desc}" }
                }
            }
            button {
                class: "px-2 py-0.5 bg-[#3a3a55] text-[#aaa] rounded text-[11px] cursor-pointer hover:text-[#e0e0e0]",
                onclick: move |_| {
                    let p = prompt.clone();
                    signal.write_unchecked().prompt_viewer = Some(crate::state::McpPromptViewerState {
                        server: p.server.clone(),
                        prompt_name: p.name.clone(),
                        args_json: "{}".to_string(),
                        result: None,
                        error: None,
                        loading: false,
                    });
                },
                "Get"
            }
        }
    }
}
