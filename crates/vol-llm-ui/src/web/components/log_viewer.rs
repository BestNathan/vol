//! Log run viewer with event details.
//!
//! State is cached per-node in `NodeDataCache` for instant switching.
//! Fetches log data via DP client's log.list and log.read methods.

use crate::web::client::{LogLine, LogRunSummary};
use crate::web::components::app::AppState;
use dioxus::prelude::*;

/// Key used to store the serialized log viewer state in NodeDataCache.
const CACHE_KEY: &str = "log_viewer";

#[component]
pub fn LogViewer() -> Element {
    let app: AppState = use_context();
    let active_node = app.active_node_id;
    let cache = app.node_data_cache;

    // Load logs from cache or trigger DP fetch when active_node changes.
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
            let client = app
                .dp_pool
                .read()
                .get(nid)
                .map(|c| c.client.clone())
                .unwrap_or_else(|| app.rpc_client.clone());

            let mut cache_mut = cache;
            let target_nid = nid.clone();
            let cache_nid = nid.clone();

            // Mark as loading in cache immediately.
            {
                let loading_state = LogViewerCacheState::default();
                let v = serde_json::to_value(&loading_state).unwrap_or_default();
                let mut c = cache_mut.write();
                let node_data = c.get_or_insert(&cache_nid);
                node_data.data.insert(CACHE_KEY.to_string(), v);
            }

            client.log_list(move |result| {
                let current_nid = active_node.read().clone();
                if current_nid != target_nid {
                    log::warn!("Node switched, discarding stale log_list response");
                    return;
                }
                let mut c = cache_mut.write();
                if let Some(d) = c.get_mut(&cache_nid) {
                    if let Some(v) = d.data.get_mut(CACHE_KEY) {
                        if let Some(obj) = v.as_object_mut() {
                            match result {
                                Ok(runs) => {
                                    obj.insert(
                                        "run_logs".to_string(),
                                        serde_json::to_value(runs).unwrap_or_default(),
                                    );
                                    obj.insert("loading".to_string(), serde_json::json!(false));
                                }
                                Err(e) => {
                                    log::error!("log_list failed: {}", e);
                                    obj.insert("loading".to_string(), serde_json::json!(false));
                                    obj.insert("error".to_string(), serde_json::json!(e));
                                }
                            }
                        }
                    }
                }
            });
        }
    });

    // Read state from cache.
    let has_active_node = active_node.read().is_some();
    let state = {
        let node_id = active_node.read().clone();
        node_id.and_then(|nid| {
            let c = cache.read();
            c.get(&nid).and_then(|d| {
                d.data
                    .get(CACHE_KEY)
                    .and_then(|v| serde_json::from_value::<LogViewerCacheState>(v.clone()).ok())
            })
        })
    };

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

    let (selected, entries, run_logs, loading) = state
        .map(|s| {
            (
                s.selected_run.clone(),
                s.entries,
                s.run_logs.len(),
                s.loading,
            )
        })
        .unwrap_or((None, Vec::new(), 0, false));

    if loading && run_logs == 0 {
        return rsx! {
            div { class: "flex-1 overflow-y-auto p-3",
                div { class: "flex items-center justify-center h-full text-[#666] text-[12px]",
                    "Loading logs..."
                }
            }
        };
    }

    match selected {
        Some(run_id) => render_log_entries(&run_id, entries, cache, active_node, loading),
        None => render_run_list(run_logs, cache, active_node, app),
    }
}

/// Serializable state cached per-node for instant switching.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct LogViewerCacheState {
    selected_run: Option<String>,
    entries: Vec<LogLine>,
    run_logs: Vec<LogRunSummary>,
    loading: bool,
    #[serde(default)]
    error: Option<String>,
}

impl Default for LogViewerCacheState {
    fn default() -> Self {
        Self {
            selected_run: None,
            entries: Vec::new(),
            run_logs: Vec::new(),
            loading: true,
            error: None,
        }
    }
}

fn render_run_list(
    count: usize,
    cache: Signal<crate::state::NodeDataCache>,
    active_node: Signal<Option<String>>,
    app: AppState,
) -> Element {
    if count == 0 {
        return rsx! { div { class: "flex-1 overflow-y-auto p-2.5", div { class: "flex items-center justify-center h-full text-[#666]", "No log files found." } } };
    }
    let items = (0..count)
        .map(|i| {
            let c = cache.clone();
            let an = active_node.clone();
            let app_clone = app.clone();
            rsx! { LogRunItem { cache: c, active_node: an, app: app_clone, index: i } }
        })
        .collect::<Vec<_>>();
    rsx! { div { class: "flex-1 overflow-y-auto p-2.5 font-mono text-[13px]", {items.into_iter()} } }
}

#[component]
fn LogRunItem(
    cache: Signal<crate::state::NodeDataCache>,
    active_node: Signal<Option<String>>,
    app: AppState,
    index: usize,
) -> Element {
    let run = {
        let node_id = active_node.read().clone();
        node_id.and_then(|nid| {
            let c = cache.read();
            c.get(&nid).and_then(|d| {
                d.data
                    .get(CACHE_KEY)
                    .and_then(|v| serde_json::from_value::<LogViewerCacheState>(v.clone()).ok())
            })
        })
    }
    .and_then(|s| s.run_logs.into_iter().nth(index));

    let Some(run) = run else {
        return rsx! {};
    };
    let short = if run.run_id.len() > 12 {
        format!("{}...", &run.run_id[..9])
    } else {
        run.run_id.clone()
    };

    let run_id_click = run.run_id.clone();
    let cache_click = cache.clone();
    let active_node_click = active_node.clone();
    let app_click = app.clone();

    rsx! {
        div {
            class: "py-0.5 text-[#ccc] cursor-pointer hover:bg-[#333]",
            onclick: move |_| {
                let nid = active_node_click.read().clone();
                if let Some(node_id) = nid {
                    // Update selected_run in cache
                    {
                        let mut c = cache_click.write();
                        if let Some(d) = c.get_mut(&node_id) {
                            if let Some(v) = d.data.get_mut(CACHE_KEY) {
                                if let Some(obj) = v.as_object_mut() {
                                    obj.insert("selected_run".to_string(), serde_json::json!(run_id_click));
                                    obj.insert("loading".to_string(), serde_json::json!(true));
                                    obj.insert("entries".to_string(), serde_json::json!(Vec::<LogLine>::new()));
                                }
                            }
                        }
                    }

                    // Fetch log entries
                    let client = app_click
                        .dp_pool
                        .read()
                        .get(&node_id)
                        .map(|c| c.client.clone())
                        .unwrap_or_else(|| app_click.rpc_client.clone());

                    let target_nid = node_id.clone();
                    let cache_mut = cache_click;

                    client.log_read(&run_id_click, move |result| {
                        let current_nid = active_node_click.read().clone();
                        if current_nid != Some(target_nid.clone()) {
                            log::warn!("Node switched, discarding stale log_read response");
                            return;
                        }
                        let mut c = cache_mut.write();
                        if let Some(d) = c.get_mut(&target_nid) {
                            if let Some(v) = d.data.get_mut(CACHE_KEY) {
                                if let Some(obj) = v.as_object_mut() {
                                    match result {
                                        Ok(entries) => {
                                            obj.insert("entries".to_string(), serde_json::to_value(entries).unwrap_or_default());
                                            obj.insert("loading".to_string(), serde_json::json!(false));
                                        }
                                        Err(e) => {
                                            log::error!("log_read failed: {}", e);
                                            obj.insert("loading".to_string(), serde_json::json!(false));
                                            obj.insert("error".to_string(), serde_json::json!(e));
                                        }
                                    }
                                }
                            }
                        }
                    });
                }
            },
            span { class: "text-[#c0c0c0]", "{short}" }
            span { class: "text-[#888]", " {run.event_count} events" }
            span { class: "text-[#888]", "  {run.last_event} ({run.last_event_time})" }
        }
    }
}

fn render_log_entries(
    run_id: &str,
    entries: Vec<LogLine>,
    cache: Signal<crate::state::NodeDataCache>,
    active_node: Signal<Option<String>>,
    loading: bool,
) -> Element {
    let run_id = run_id.to_string();
    let cache_back = cache.clone();
    let active_node_back = active_node.clone();
    let count = entries.len();

    if loading && count == 0 {
        return rsx! {
            div { class: "flex-1 overflow-y-auto p-2.5",
                div { class: "mb-2",
                    span {
                        class: "text-[#4080ff] cursor-pointer hover:underline text-[12px]",
                        onclick: move |_| {
                            let nid = active_node_back.read().clone();
                            if let Some(node_id) = nid {
                                let mut c = cache_back.write();
                                if let Some(d) = c.get_mut(&node_id) {
                                    if let Some(v) = d.data.get_mut(CACHE_KEY) {
                                        if let Some(obj) = v.as_object_mut() {
                                            obj.insert("selected_run".to_string(), serde_json::json!(null));
                                        }
                                    }
                                }
                            }
                        },
                        "← Back to run list"
                    }
                }
                div { class: "flex items-center justify-center h-full text-[#666]", "Loading log entries..." }
            }
        };
    }
    if count == 0 {
        return rsx! {
            div { class: "flex-1 overflow-y-auto p-2.5",
                div { class: "mb-2",
                    span {
                        class: "text-[#4080ff] cursor-pointer hover:underline text-[12px]",
                        onclick: move |_| {
                            let nid = active_node_back.read().clone();
                            if let Some(node_id) = nid {
                                let mut c = cache_back.write();
                                if let Some(d) = c.get_mut(&node_id) {
                                    if let Some(v) = d.data.get_mut(CACHE_KEY) {
                                        if let Some(obj) = v.as_object_mut() {
                                            obj.insert("selected_run".to_string(), serde_json::json!(null));
                                        }
                                    }
                                }
                            }
                        },
                        "← Back to run list"
                    }
                }
                div { class: "flex items-center justify-center h-full text-[#666]", "No events in this run." }
            }
        };
    }
    let items = entries
        .into_iter()
        .map(|entry| {
            rsx! { LogEntryItem { entry: entry } }
        })
        .collect::<Vec<_>>();
    rsx! {
        div { class: "flex-1 overflow-y-auto p-2.5",
            div { class: "mb-2 flex items-center gap-3",
                span {
                    class: "text-[#4080ff] cursor-pointer hover:underline text-[12px]",
                    onclick: move |_| {
                        let nid = active_node_back.read().clone();
                        if let Some(node_id) = nid {
                            let mut c = cache_back.write();
                            if let Some(d) = c.get_mut(&node_id) {
                                if let Some(v) = d.data.get_mut(CACHE_KEY) {
                                    if let Some(obj) = v.as_object_mut() {
                                        obj.insert("selected_run".to_string(), serde_json::json!(null));
                                    }
                                }
                            }
                        }
                    },
                    "← Back to run list"
                }
                span { class: "text-[12px] text-[#888]", "Log: {run_id}" }
            }
            {items.into_iter()}
        }
    }
}

#[component]
fn LogEntryItem(entry: LogLine) -> Element {
    let color = match entry.event_type.as_str() {
        "AgentStart" | "AgentComplete" => "#40c040",
        "ToolCallBegin" | "ToolCallComplete" => "#c0c040",
        "ToolCallError" | "AgentAborted" => "#c04040",
        _ => "#e0e0e0",
    };
    rsx! { div { class: "font-mono text-[12px] py-0.5 whitespace-nowrap", span { class: "text-[#666]", "[{entry.timestamp}] " } span { class: "font-bold", style: "color: {color};", "{entry.event_type}" } span { style: "color: {color};", " -- {entry.summary}" } } }
}
