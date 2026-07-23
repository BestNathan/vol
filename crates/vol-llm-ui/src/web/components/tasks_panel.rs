use dioxus::prelude::*;

use super::task_dep_graph::TaskDepGraph;
use crate::state::TaskState;
use crate::web::client::TaskEntry;

pub(crate) fn status_color(status: &str) -> &'static str {
    match status {
        "pending" => "#888",
        "running" => "#4080ff",
        "completed" => "#40c040",
        "failed" => "#ff4040",
        "killed" => "#ff8800",
        _ => "#888",
    }
}

/// Serializable state cached per-node for instant switching.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct TasksCacheState {
    tasks: Vec<TaskEntry>,
    loading: bool,
    error: Option<String>,
}

impl Default for TasksCacheState {
    fn default() -> Self {
        Self {
            tasks: Vec::new(),
            loading: true,
            error: None,
        }
    }
}

/// Key used to store the tasks list in NodeDataCache.
const CACHE_KEY: &str = "tasks";

#[component]
pub fn TasksPanel(assignee_filter: Option<String>) -> Element {
    let app: crate::web::components::app::AppState = use_context();
    // Local UI state (not cached) — filter/selection persist only within session.
    let task_state = use_signal(|| TaskState::new());
    let graph_target = use_signal(|| None::<u64>);

    let active_node = app.active_node_id;
    let cache = app.node_data_cache;
    let initial_assignee = assignee_filter.clone();

    // Load tasks from cache or trigger DP fetch when active_node changes.
    let app_for_effect = app.clone();
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
            let client = app_for_effect
                .dp_pool
                .read()
                .get(nid)
                .map(|c| c.client.clone())
                .unwrap_or_else(|| app_for_effect.rpc_client.clone());

            let mut cache_mut = cache;
            let target_nid = nid.clone();
            let cache_nid = nid.clone();
            let assignee = initial_assignee.clone();

            // Mark as loading in cache immediately.
            {
                let loading_state = TasksCacheState::default();
                let v = serde_json::to_value(&loading_state).unwrap_or_default();
                let mut c = cache_mut.write();
                let node_data = c.get_or_insert(&cache_nid);
                node_data.data.insert(CACHE_KEY.to_string(), v);
            }

            client.task_list(None, assignee.as_deref(), move |result| {
                let current_nid = active_node.read().clone();
                if current_nid != Some(target_nid) {
                    log::warn!("Node switched, discarding stale task_list response");
                    return;
                }
                let mut c = cache_mut.write();
                if let Some(d) = c.get_mut(&cache_nid) {
                    if let Some(v) = d.data.get_mut(CACHE_KEY) {
                        if let Some(obj) = v.as_object_mut() {
                            match result {
                                Ok(tasks) => {
                                    obj.insert(
                                        "tasks".to_string(),
                                        serde_json::to_value(tasks).unwrap_or_default(),
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
    let event_bus = app.event_bus.clone();
    let assignee_for_reconnect = assignee_filter.clone();
    let app_for_hook = app.clone();
    use_hook(move || {
        let _sub = event_bus.subscribe(crate::state::UiEventKind::WsConnected, move |_| {
            let node_id = active_node.read().clone();
            if let Some(ref nid) = node_id {
                // Invalidate cache so use_effect re-fetches.
                let mut c = cache.write_unchecked();
                c.invalidate(nid);

                let client = app_for_hook
                    .dp_pool
                    .read()
                    .get(nid)
                    .map(|c| c.client.clone())
                    .unwrap_or_else(|| app_for_hook.rpc_client.clone());

                let cache_mut = cache;
                let target_nid = nid.clone();
                let cache_nid = nid.clone();
                let assignee = assignee_for_reconnect.clone();

                // Mark loading.
                {
                    let loading_state = TasksCacheState::default();
                    let v = serde_json::to_value(&loading_state).unwrap_or_default();
                    let mut c = cache_mut.write_unchecked();
                    let node_data = c.get_or_insert(&cache_nid);
                    node_data.data.insert(CACHE_KEY.to_string(), v);
                }

                client.task_list(None, assignee.as_deref(), move |result| {
                    let current_nid = active_node.read().clone();
                    if current_nid != Some(target_nid) {
                        log::warn!("Node switched, discarding stale task_list response");
                        return;
                    }
                    let mut c = cache_mut.write_unchecked();
                    if let Some(d) = c.get_mut(&cache_nid) {
                        if let Some(v) = d.data.get_mut(CACHE_KEY) {
                            if let Some(obj) = v.as_object_mut() {
                                match result {
                                    Ok(tasks) => {
                                        obj.insert(
                                            "tasks".to_string(),
                                            serde_json::to_value(tasks).unwrap_or_default(),
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
    let (tasks, loading, error) = {
        let node_id = active_node.read().clone();
        node_id
            .and_then(|nid| {
                let c = cache.read();
                c.get(&nid).and_then(|d| {
                    d.data
                        .get(CACHE_KEY)
                        .and_then(|v| serde_json::from_value::<TasksCacheState>(v.clone()).ok())
                })
            })
            .map(|s| (s.tasks, s.loading, s.error))
            .unwrap_or_default()
    };
    let selected_task_id = task_state.read().selected_task;
    let status_filter = task_state.read().status_filter.clone();

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

    // Filter tasks by selected status
    let filtered: Vec<_> = if let Some(ref sf) = status_filter {
        if sf == "all" {
            tasks.clone()
        } else {
            tasks.iter().filter(|t| t.status == *sf).cloned().collect()
        }
    } else {
        tasks.clone()
    };

    // Empty + error
    if tasks.is_empty() && error.is_some() {
        let err = error.as_deref().unwrap_or("unknown");
        let app_retry = app.clone();
        let a = assignee_filter.clone();
        return rsx! { div { class: "flex-1 overflow-y-auto p-3",
            div { class: "flex flex-col items-center justify-center h-full gap-3 text-center",
                div { class: "text-[#ff6060] text-[14px]", "Failed to load tasks" }
                div { class: "text-[#888] text-[12px] max-w-[300px] break-words", "{err}" }
                button {
                    class: "px-4 py-1.5 bg-[#3a3a55] text-[#ccc] rounded text-[13px] hover:bg-[#4a4a65]",
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
                            let a2 = a.clone();

                            // Mark loading.
                            {
                                let loading_state = TasksCacheState::default();
                                let v = serde_json::to_value(&loading_state).unwrap_or_default();
                                let mut c = cache_mut.write();
                                let node_data = c.get_or_insert(&cache_nid);
                                node_data.data.insert(CACHE_KEY.to_string(), v);
                            }

                            client.task_list(None, a2.as_deref(), move |result| {
                                let current_nid = app_retry.active_node_id.read().clone();
                                if current_nid != Some(target_nid) {
                                    log::warn!("Node switched, discarding stale task_list response");
                                    return;
                                }
                                let mut c = cache_mut.write();
                                if let Some(d) = c.get_mut(&cache_nid) {
                                    if let Some(v) = d.data.get_mut(CACHE_KEY) {
                                        if let Some(obj) = v.as_object_mut() {
                                            match result {
                                                Ok(tasks) => {
                                                    obj.insert("tasks".to_string(), serde_json::to_value(tasks).unwrap_or_default());
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
        }};
    }

    // Loading
    if loading && tasks.is_empty() {
        return rsx! { div { class: "flex-1 overflow-y-auto p-2",
            div { class: "flex items-center justify-center h-full text-[#666] text-[14px]", "Loading tasks..." }
        }};
    }

    rsx! {
        div { class: "flex flex-col flex-1 min-h-0 overflow-hidden",
            // Status filter bar
            div { class: "flex gap-1 p-2 border-b border-[#333355] flex-shrink-0 overflow-x-auto",
                for label in &["all", "pending", "running", "completed"] {
                    {
                        let lbl = label.to_string();
                        let is_active = status_filter.as_deref() == Some(label);
                        let cls = if is_active {
                            "px-2 py-0.5 rounded text-[11px] cursor-pointer bg-[#80a0ff] text-[#1a1a2e] whitespace-nowrap"
                        } else {
                            "px-2 py-0.5 rounded text-[11px] cursor-pointer bg-[#2a2a44] text-[#888] hover:bg-[#3a3a55] whitespace-nowrap"
                        };
                        rsx! {
                            button {
                                class: "{cls}",
                                onclick: {
                                    let mut s = task_state;
                                    let lbl2 = lbl.clone();
                                    move |_| { s.with_mut(|t| t.status_filter = Some(lbl2.clone())); }
                                },
                                "{lbl}"
                            }
                        }
                    }
                }
            }

            // Task list
            div { class: "flex-1 overflow-y-auto",
                if filtered.is_empty() {
                    div { class: "flex items-center justify-center h-full text-[#666] text-[13px]",
                        "No tasks found"
                    }
                }
                for task in &filtered {
                    {
                        let is_selected = selected_task_id == Some(task.id);
                        let row_cls = if is_selected {
                            "border-b border-[#333355] p-2 cursor-pointer bg-[#1a2a44]"
                        } else {
                            "border-b border-[#333355] p-2 cursor-pointer hover:bg-[#222240]"
                        };
                        let color = status_color(&task.status);
                        let task_id = task.id;
                        let task_id2 = task.id;
                        let task_id3 = task.id;
                        let mut graph_open = graph_target;
                        rsx! {
                            div {
                                key: "{task.id}",
                                class: "{row_cls}",
                                onclick: {
                                    let mut s = task_state;
                                    move |_| {
                                        s.with_mut(|t| {
                                            t.selected_task = if t.selected_task == Some(task_id2) { None } else { Some(task_id2) };
                                        });
                                    }
                                },
                                div { class: "flex items-center gap-2",
                                    span { class: "text-[11px] text-[#555] font-mono whitespace-nowrap", "t{task_id}" }
                                    span {
                                        class: "text-[10px] px-1 rounded font-bold whitespace-nowrap",
                                        style: "background: {color}; color: #1a1a2e;",
                                        "{task.status}"
                                    }
                                    span { class: "text-[13px] text-[#e0e0e0] truncate", "{task.subject}" }
                                    div { class: "flex items-center gap-2 ml-auto",
                                        if let Some(ref assignee) = task.assignee {
                                            span { class: "text-[11px] text-[#666] whitespace-nowrap", "{assignee}" }
                                        }
                                        button {
                                            class: "text-[11px] text-[#80a0ff] hover:text-[#a0c0ff] px-1 rounded whitespace-nowrap",
                                            title: "View dependency graph",
                                            onclick: move |evt| {
                                                evt.stop_propagation();
                                                graph_open.set(Some(task_id3));
                                            },
                                            "⇄ deps"
                                        }
                                    }
                                }
                                // Expanded detail
                                if is_selected {
                                    div { class: "mt-2 pl-4 text-[12px] text-[#aaa] flex flex-col gap-1",
                                        if !task.description.is_empty() {
                                            div { class: "text-[#ccc] mb-1", "{task.description}" }
                                        }
                                        if !task.dependencies.is_empty() {
                                            div { class: "text-[#888]",
                                                "Dependencies: "
                                                for (i, dep) in task.dependencies.iter().enumerate() {
                                                    span { "t{dep}" }
                                                    if i < task.dependencies.len() - 1 { span { ", " } }
                                                }
                                            }
                                        }
                                        if !task.blocks.is_empty() {
                                            div { class: "text-[#888]",
                                                "Blocks: "
                                                for (i, blk) in task.blocks.iter().enumerate() {
                                                    span { "t{blk}" }
                                                    if i < task.blocks.len() - 1 { span { ", " } }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if let Some(center) = graph_target() {
                {
                    let mut graph_close = graph_target;
                    rsx! {
                        TaskDepGraph {
                            tasks: tasks.clone(),
                            center,
                            on_close: move |_| graph_close.set(None),
                        }
                    }
                }
            }
        }
    }
}
