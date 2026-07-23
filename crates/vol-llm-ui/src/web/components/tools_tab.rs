//! Tools tab with system tool listing and tool call history.

use super::tool_dialog::{SystemToolDialog, SystemToolDialogState};
use crate::state::{ToolCallEntry, ToolCallStatus, ToolState, UiEvent, UiEventKind};
use crate::web::components::app::AppState;
use dioxus::prelude::*;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
struct ToolDef {
    name: String,
    description: Option<String>,
    #[allow(dead_code)]
    parameters: Option<serde_json::Value>,
}

/// Key used to store the tools list in NodeDataCache.
const CACHE_KEY: &str = "tools";

/// Serializable state cached per-node for instant switching.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct ToolsCacheState {
    tools: Vec<ToolDef>,
    loading: bool,
    error: Option<String>,
}

impl Default for ToolsCacheState {
    fn default() -> Self {
        Self {
            tools: Vec::new(),
            loading: true,
            error: None,
        }
    }
}

struct ToolsPanelState {
    call_result: Option<String>,
}

fn update_status(
    calls: &mut Vec<ToolCallEntry>,
    name: &str,
    status: ToolCallStatus,
    dur: Option<u64>,
) {
    for e in calls.iter_mut().rev() {
        if e.tool_name == name && matches!(e.status, ToolCallStatus::Running) {
            e.status = status;
            e.duration_ms = dur;
            break;
        }
    }
}

fn arg_preview(arguments: &str) -> String {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(arguments) {
        if let Some(c) = v.get("command").and_then(|v| v.as_str()) {
            return if c.chars().count() > 80 {
                format!("Command: {}...", c.chars().take(77).collect::<String>())
            } else {
                format!("Command: {}", c)
            };
        }
        if let Some(p) = v.get("path").and_then(|v| v.as_str()) {
            return format!("Path: {}", p);
        }
        if let Some(f) = v.get("file_path").and_then(|v| v.as_str()) {
            return format!("File: {}", f);
        }
        if arguments.chars().count() > 80 {
            return format!(
                "Args: {}...",
                arguments.chars().take(77).collect::<String>()
            );
        }
        return format!("Args: {}", arguments);
    }
    String::new()
}

pub fn reduce_tool_state(s: &mut ToolState, event: &UiEvent) {
    match event {
        UiEvent::ToolCallBegin {
            tool_name,
            arguments,
        } => {
            let seq = s.calls.len() as u32 + 1;
            s.calls.push(ToolCallEntry {
                sequence: seq,
                tool_name: tool_name.clone(),
                arg_preview: arg_preview(arguments),
                status: ToolCallStatus::Running,
                duration_ms: None,
            });
            s.scroll = s.calls.len() as u16;
        }
        UiEvent::ToolCallComplete {
            tool_name,
            duration_ms,
            ..
        } => update_status(
            &mut s.calls,
            tool_name,
            ToolCallStatus::Success,
            *duration_ms,
        ),
        UiEvent::ToolCallError {
            tool_name,
            duration_ms,
            ..
        } => update_status(&mut s.calls, tool_name, ToolCallStatus::Error, *duration_ms),
        UiEvent::ToolCallSkipped {
            tool_name,
            duration_ms,
            ..
        } => update_status(
            &mut s.calls,
            tool_name,
            ToolCallStatus::Skipped,
            *duration_ms,
        ),
        _ => {}
    }
}

#[component]
pub fn ToolsTabContent() -> Element {
    let app_state: AppState = use_context();
    let call_signal: Signal<ToolState> = use_context();
    let tool_state = use_signal(|| ToolsPanelState { call_result: None });
    let dialog_state = use_signal(|| SystemToolDialogState::new());

    let active_node = app_state.active_node_id;
    let cache = app_state.node_data_cache;

    // Load tools from cache or trigger DP fetch when active_node changes.
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
                let loading_state = ToolsCacheState::default();
                let v = serde_json::to_value(&loading_state).unwrap_or_default();
                let mut c = cache_mut.write();
                let node_data = c.get_or_insert(&cache_nid);
                node_data.data.insert(CACHE_KEY.to_string(), v);
            }

            client.tool_list(move |result| {
                let current_nid = active_node.read().clone();
                if current_nid != Some(target_nid) {
                    log::warn!("Node switched, discarding stale tool_list response");
                    return;
                }
                let mut c = cache_mut.write();
                if let Some(d) = c.get_mut(&cache_nid) {
                    if let Some(v) = d.data.get_mut(CACHE_KEY) {
                        if let Some(obj) = v.as_object_mut() {
                            match result {
                                Ok(tools) => {
                                    let parsed: Vec<ToolDef> = tools
                                        .iter()
                                        .filter_map(|t| {
                                            serde_json::from_value::<ToolDef>(t.clone()).ok()
                                        })
                                        .collect();
                                    obj.insert(
                                        "tools".to_string(),
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
                // Invalidate cache so use_effect re-fetches.
                let mut c = cache.write_unchecked();
                c.invalidate(nid);
            }
            // Trigger re-render; use_effect will re-run due to cache invalidation.
            // Since use_effect only runs on signal change, we manually trigger load here.
            if let Some(ref nid) = node_id {
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
                    let loading_state = ToolsCacheState::default();
                    let v = serde_json::to_value(&loading_state).unwrap_or_default();
                    let mut c = cache_mut.write_unchecked();
                    let node_data = c.get_or_insert(&cache_nid);
                    node_data.data.insert(CACHE_KEY.to_string(), v);
                }

                client.tool_list(move |result| {
                    let current_nid = active_node.read().clone();
                    if current_nid != Some(target_nid) {
                        log::warn!("Node switched, discarding stale tool_list response");
                        return;
                    }
                    let mut c = cache_mut.write_unchecked();
                    if let Some(d) = c.get_mut(&cache_nid) {
                        if let Some(v) = d.data.get_mut(CACHE_KEY) {
                            if let Some(obj) = v.as_object_mut() {
                                match result {
                                    Ok(tools) => {
                                        let parsed: Vec<ToolDef> = tools
                                            .iter()
                                            .filter_map(|t| {
                                                serde_json::from_value::<ToolDef>(t.clone()).ok()
                                            })
                                            .collect();
                                        obj.insert(
                                            "tools".to_string(),
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
    let (tools, loading, error) = {
        let node_id = active_node.read().clone();
        node_id
            .and_then(|nid| {
                let c = cache.read();
                c.get(&nid).and_then(|d| {
                    d.data
                        .get(CACHE_KEY)
                        .and_then(|v| serde_json::from_value::<ToolsCacheState>(v.clone()).ok())
                })
            })
            .map(|s| (s.tools, s.loading, s.error))
            .unwrap_or_default()
    };
    let call_result = tool_state.read().call_result.clone();
    let call_count = call_signal.read().calls.len();

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

    // Pre-build call history items
    let call_items: Vec<Element> = (0..call_count)
        .map(|idx| {
            let s = call_signal.clone();
            rsx! { ToolCallItem { signal: s, index: idx } }
        })
        .collect();

    rsx! {
        div { class: "flex-1 overflow-y-auto p-2",
            // System Tools section
            if !tools.is_empty() || loading {
                div { class: "mb-3",
                    div { class: "flex items-center justify-between mb-1",
                        div { class: "px-2.5 py-1 text-[12px] font-semibold text-[#888] uppercase tracking-[0.5px]",
                            "System Tools ({tools.len()})"
                        }
                        button {
                            class: "px-2 py-0.5 text-[12px] bg-[#3a3a55] text-[#ccc] rounded hover:bg-[#4a4a65]",
                            onclick: {
                                let app = app_state.clone();
                                move |_| {
                                    let node_id = app.active_node_id.read().clone();
                                    if let Some(ref nid) = node_id {
                                        let client = app
                                            .dp_pool
                                            .read()
                                            .get(nid)
                                            .map(|c| c.client.clone())
                                            .unwrap_or_else(|| app.rpc_client.clone());

                                        let mut cache_mut = app.node_data_cache;
                                        let target_nid = nid.clone();
                                        let cache_nid = nid.clone();

                                        // Mark loading.
                                        {
                                            let loading_state = ToolsCacheState::default();
                                            let v = serde_json::to_value(&loading_state).unwrap_or_default();
                                            let mut c = cache_mut.write();
                                            let node_data = c.get_or_insert(&cache_nid);
                                            node_data.data.insert(CACHE_KEY.to_string(), v);
                                        }

                                        client.tool_list(move |result| {
                                            let current_nid = app.active_node_id.read().clone();
                                            if current_nid != Some(target_nid) {
                                                log::warn!("Node switched, discarding stale tool_list response");
                                                return;
                                            }
                                            let mut c = cache_mut.write();
                                            if let Some(d) = c.get_mut(&cache_nid) {
                                                if let Some(v) = d.data.get_mut(CACHE_KEY) {
                                                    if let Some(obj) = v.as_object_mut() {
                                                        match result {
                                                            Ok(tools) => {
                                                                let parsed: Vec<ToolDef> = tools
                                                                    .iter()
                                                                    .filter_map(|t| serde_json::from_value::<ToolDef>(t.clone()).ok())
                                                                    .collect();
                                                                obj.insert("tools".to_string(), serde_json::to_value(parsed).unwrap_or_default());
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
                                }
                            },
                            "Refresh"
                        }
                    }
                    if loading {
                        div { class: "text-[12px] text-[#888] px-2", "Loading..." }
                    }
                    if let Some(ref e) = error {
                        div { class: "text-[12px] text-[#c04040] px-2 break-words", "Error: {e}" }
                    }
                    // Mobile: tool cards
                    div { class: "sm:hidden flex flex-col gap-2 mb-2",
                        for tool in &tools {
                            div { class: "rounded-lg border border-[#333355] bg-[#20203a] p-3",
                                div { class: "flex items-center justify-between",
                                    div { class: "min-w-0",
                                        div { class: "truncate text-[14px] font-bold text-[#e0e0e0]", "{tool.name}" }
                                        if let Some(ref desc) = tool.description {
                                            div { class: "mt-0.5 text-[11px] text-[#777] truncate", "{desc}" }
                                        }
                                    }
                                    button {
                                        class: "px-2 py-0.5 text-[11px] bg-[#4080ff] text-white rounded hover:bg-[#5090ff] flex-shrink-0 ml-2",
                                        onclick: {
                                            let mut ds = dialog_state;
                                            let name = tool.name.clone();
                                            let desc = tool.description.clone();
                                            let params = tool.parameters.clone();
                                            move |_| {
                                                ds.with_mut(|s| {
                                                    s.open = true;
                                                    s.tool_name = name.clone();
                                                    s.description = desc.clone();
                                                    s.parameters = params.clone();
                                                    s.result = None;
                                                    s.error = None;
                                                    s.loading = false;
                                                });
                                            }
                                        },
                                        "Run"
                                    }
                                }
                            }
                        }
                    }
                    // Desktop: tool rows
                    div { class: "hidden sm:block",
                        for tool in &tools {
                            div { class: "border-b border-[#2a2a44] py-1 px-2",
                                div { class: "flex items-center justify-between",
                                    div {
                                        span { class: "text-[13px] font-semibold text-[#e0e0e0]", "{tool.name}" }
                                        if let Some(ref desc) = tool.description {
                                            span { class: "text-[12px] text-[#888] ml-2", " - {desc}" }
                                        }
                                    }
                                    button {
                                        class: "px-1.5 py-0.5 text-[11px] bg-[#3a3a55] text-[#ccc] rounded hover:bg-[#5a5a75]",
                                        onclick: {
                                            let mut ds = dialog_state;
                                            let name = tool.name.clone();
                                            let desc = tool.description.clone();
                                            let params = tool.parameters.clone();
                                            move |_| {
                                                ds.with_mut(|s| {
                                                    s.open = true;
                                                    s.tool_name = name.clone();
                                                    s.description = desc.clone();
                                                    s.parameters = params.clone();
                                                    s.result = None;
                                                    s.error = None;
                                                    s.loading = false;
                                                });
                                            }
                                        },
                                        "Run"
                                    }
                                }
                            }
                        }
                    }
                }

                // Call result display
                if let Some(ref result) = call_result {
                    div { class: "mb-2",
                        div { class: "text-[12px] font-semibold text-[#888] mb-1", "Call Result" }
                        pre { class: "text-[12px] font-mono text-[#ccc] bg-[#1a1a2e] p-2 rounded overflow-x-auto whitespace-pre-wrap", "{result}" }
                    }
                }

                div { class: "border-t border-[#333] my-2" }
            }

            // Call History section
            if call_count == 0 {
                div { class: "flex items-center justify-center h-[200px] text-[#666]",
                    if loading {
                        "Loading tools..."
                    } else if error.is_some() {
                        "Failed to load tools"
                    } else if !tools.is_empty() {
                        "No tool calls yet — click Run on a tool above"
                    } else {
                        "No tools available"
                    }
                }
            } else {
                div { class: "px-2.5 pt-1 pb-2 text-[12px] font-semibold text-[#888] uppercase tracking-[0.5px]", "Call History ({call_count})" }
                // Mobile: history cards
                div { class: "sm:hidden flex flex-col gap-2",
                    {(0..call_count).map(|idx| {
                        let s = call_signal.clone();
                        rsx! { ToolCallHistoryCard { signal: s, index: idx } }
                    }).collect::<Vec<Element>>().into_iter()}
                }
                // Desktop: history rows
                div { class: "hidden sm:block",
                    {call_items.into_iter()}
                }
            }
        }

        SystemToolDialog { signal: dialog_state }
    }
}

#[component]
fn ToolCallItem(signal: Signal<ToolState>, index: usize) -> Element {
    let is_expanded = signal.read().expanded.contains(&index);
    let (seq, name, arg, status, dur) = {
        let ui = signal.read();
        match ui.calls.get(index) {
            Some(e) => (
                e.sequence,
                e.tool_name.clone(),
                e.arg_preview.clone(),
                e.status.clone(),
                e.duration_ms,
            ),
            None => return rsx! {},
        }
    };
    let scls = match status {
        ToolCallStatus::Running => "text-[#c0c040]",
        ToolCallStatus::Success => "text-[#40c040]",
        ToolCallStatus::Error => "text-[#c04040]",
        ToolCallStatus::Skipped => "text-[#888]",
    };
    let label = match status {
        ToolCallStatus::Running => "...",
        ToolCallStatus::Success => "OK",
        ToolCallStatus::Error => "ERR",
        ToolCallStatus::Skipped => "SKIP",
    };
    let dur_s = dur.map(|ms| format!("{ms}ms")).unwrap_or_default();
    rsx! {
        div { class: "border-b border-[#2a2a44]",
            div { class: "flex items-center px-2.5 py-2 cursor-pointer gap-2 hover:bg-[#222240]",
                onclick: move |_: Event<MouseData>| {
                    let mut state = signal.write_unchecked();
                    if state.expanded.contains(&index) {
                        state.expanded.remove(&index);
                    } else {
                        state.expanded.insert(index);
                    }
                },
                span { class: "text-[#555] text-[11px] min-w-[24px]", "{seq}." }
                span { class: "font-semibold text-[13px]", "[{name}]" }
                span { class: "text-[11px] px-1.5 py-0.5 rounded-[3px] {scls}", "{label}" }
                if !dur_s.is_empty() { span { class: "text-[11px] text-[#888] ml-auto", "{dur_s}" } }
                span { class: "text-[10px] text-[#666] ml-1", "\u{25be}" }
            }
            if is_expanded {
                div { class: "px-2.5 pb-2 pl-[42px] text-[12px] font-mono text-[#888] bg-[#16162a] whitespace-pre-wrap break-all",
                    div {
                        span { class: "text-[#6090ff] font-semibold font-sans", "Input: " }
                        "{arg}"
                    }
                }
            }
        }
    }
}

/// Mobile card for tool call history (sm:hidden).
#[component]
fn ToolCallHistoryCard(signal: Signal<ToolState>, index: usize) -> Element {
    let is_expanded = signal.read().expanded.contains(&index);
    let (seq, name, arg, status, dur) = {
        let ui = signal.read();
        match ui.calls.get(index) {
            Some(e) => (
                e.sequence,
                e.tool_name.clone(),
                e.arg_preview.clone(),
                e.status.clone(),
                e.duration_ms,
            ),
            None => return rsx! {},
        }
    };
    let scls = match status {
        ToolCallStatus::Running => "text-[#c0c040]",
        ToolCallStatus::Success => "text-[#40c040]",
        ToolCallStatus::Error => "text-[#c04040]",
        ToolCallStatus::Skipped => "text-[#888]",
    };
    let label = match status {
        ToolCallStatus::Running => "...",
        ToolCallStatus::Success => "OK",
        ToolCallStatus::Error => "ERR",
        ToolCallStatus::Skipped => "SKIP",
    };
    let dur_s = dur.map(|ms| format!("{ms}ms")).unwrap_or_default();
    rsx! {
        div {
            class: "cursor-pointer rounded-lg border border-[#333355] bg-[#20203a] p-3 active:bg-[#2a2a44]",
            onclick: move |_: Event<MouseData>| {
                let mut state = signal.write_unchecked();
                if state.expanded.contains(&index) {
                    state.expanded.remove(&index);
                } else {
                    state.expanded.insert(index);
                }
            },
            div { class: "flex items-center gap-2",
                span { class: "text-[#555] text-[11px]", "{seq}." }
                span { class: "font-semibold text-[13px] text-[#e0e0e0] truncate", "[{name}]" }
                span { class: "text-[11px] px-1.5 py-0.5 rounded-[3px] {scls}", "{label}" }
                if !dur_s.is_empty() { span { class: "text-[11px] text-[#666] ml-auto", "{dur_s}" } }
            }
            if is_expanded {
                div { class: "mt-2 pt-2 border-t border-[#2a2a44] text-[12px] font-mono text-[#888] whitespace-pre-wrap break-all",
                    span { class: "text-[#6090ff] font-semibold font-sans", "Input: " }
                    "{arg}"
                }
            }
        }
    }
}
