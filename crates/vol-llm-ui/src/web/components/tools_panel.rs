//! Left panel showing system tools and tool call history.

use crate::state::{
    AgentsState, CapabilityOverlayState, GlobalState, SubscriptionSet, ToolCallEntry,
    ToolCallStatus, ToolState, UiEvent, UiEventKind,
};
use crate::web::client::JsonRpcClient;
use crate::web::components::app::AppState;
use dioxus::prelude::*;
use std::collections::HashSet;

#[derive(Debug, Clone, serde::Deserialize)]
struct ToolDef {
    name: String,
    description: Option<String>,
    #[allow(dead_code)]
    parameters: Option<serde_json::Value>,
}

struct ToolPanelState {
    tools: Vec<ToolDef>,
    loading: bool,
    error: Option<String>,
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

pub fn status_label(s: ToolCallStatus) -> &'static str {
    match s {
        ToolCallStatus::Running => "...",
        ToolCallStatus::Success => "OK",
        ToolCallStatus::Error => "ERR",
        ToolCallStatus::Skipped => "SKIP",
    }
}

/// Update ToolState from an EventBus event.
fn reduce_tool_state(s: &mut ToolState, event: &UiEvent) {
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
pub fn ToolsPanel() -> Element {
    let app_state: AppState = use_context();
    let call_signal = use_signal(|| ToolState::new());
    let tool_signal = use_signal(|| ToolPanelState {
        tools: vec![],
        loading: false,
        error: None,
        call_result: None,
    });
    // Use agent_client() which routes to DP pool in CP mode.
    let client: JsonRpcClient = app_state.agent_client();

    // Capability overlay state
    let cap_signal: Signal<CapabilityOverlayState> = use_signal(CapabilityOverlayState::new);
    let selected_tools_signal: Signal<HashSet<String>> = use_signal(HashSet::new);
    let cap_dirty: Signal<bool> = use_signal(|| false);
    let global: Signal<GlobalState> = use_context();
    let agents: Signal<AgentsState> = use_context();

    // Load capabilities on mount
    let client_for_cap = client.clone();
    let global_for_cap = global.clone();
    let agents_for_cap = agents.clone();
    use_hook(move || {
        let mut cap_signal = cap_signal;
        let agent_id = agents_for_cap.read().selected.clone().unwrap_or_default();
        let session_id = global_for_cap.read().session_id.clone();
        if agent_id.is_empty() {
            cap_signal.with_mut(|s| s.loading = false);
            return;
        }
        let sig = cap_signal.clone();
        let mut sel = selected_tools_signal.clone();
        let mut dirty = cap_dirty.clone();
        client_for_cap.agent_get_capabilities(&agent_id, &session_id, move |result| {
            let mut sig = sig;
            sig.with_mut(|s| match result {
                Ok(cap) => {
                    s.effective_tools.clone_from(&cap.effective_tools);
                    s.available_tools.clone_from(&cap.available_tools);
                    s.base_tools.clone_from(&cap.base_tools);
                    s.effective_skills = cap.effective_skills;
                    s.available_skills = cap.available_skills;
                    s.base_skills = cap.base_skills;
                    s.effective_mcp_servers = cap.effective_mcp_servers;
                    s.available_mcp_servers = cap.available_mcp_servers;
                    s.base_mcp_servers = cap.base_mcp_servers;
                    s.loading = false;
                    s.dirty = false;
                    let hs: HashSet<String> = cap.effective_tools.into_iter().collect();
                    sel.set(hs);
                    dirty.set(false);
                }
                Err(e) => {
                    s.loading = false;
                    log::error!("Failed to load tool capabilities: {e}");
                }
            });
        });
    });

    // Load tools on mount (follow sessions panel pattern: use_hook, not use_effect)
    let client_for_load = client.clone();
    use_hook(move || {
        let mut sig = tool_signal.clone();

        sig.with_mut(|s| {
            s.loading = true;
            s.error = None;
        });

        client_for_load.tool_list(move |result| {
            let mut sig = sig;
            sig.with_mut(|s| {
                s.loading = false;
                match result {
                    Ok(tools) => {
                        s.tools = tools
                            .iter()
                            .filter_map(|t| serde_json::from_value::<ToolDef>(t.clone()).ok())
                            .collect();
                    }
                    Err(e) => {
                        s.error = Some(e);
                    }
                }
            });
        });
    });

    // Subscribe to WsConnected for re-fetch on reconnect
    let event_bus = app_state.event_bus.clone();
    let client_for_reconnect = client.clone();
    let ts_for_reconnect = tool_signal.clone();
    use_hook(move || {
        let _sub = event_bus.subscribe(UiEventKind::WsConnected, move |_| {
            let cl = client_for_reconnect.clone();
            let sig_reconnect = ts_for_reconnect.clone();
            cl.tool_list(move |result| {
                let mut sig = sig_reconnect.clone();
                sig.with_mut(|s| {
                    s.loading = false;
                    match result {
                        Ok(tools) => {
                            s.tools = tools
                                .iter()
                                .filter_map(|t| serde_json::from_value::<ToolDef>(t.clone()).ok())
                                .collect();
                        }
                        Err(e) => {
                            s.error = Some(e);
                        }
                    }
                });
            });
        });
    });

    // Subscribe to tool call events
    let event_bus2 = app_state.event_bus.clone();
    use_hook(move || {
        let mut set = SubscriptionSet::new(event_bus2.clone());
        for kind in [
            UiEventKind::ToolCallBegin,
            UiEventKind::ToolCallComplete,
            UiEventKind::ToolCallError,
            UiEventKind::ToolCallSkipped,
        ] {
            set.subscribe(&event_bus2, kind, {
                let signal = call_signal.clone();
                move |event| {
                    reduce_tool_state(&mut *signal.write_unchecked(), event);
                }
            });
        }
        std::sync::Arc::new(set)
    });

    // Read state for rendering
    let (tools, loading, error, call_result) = {
        let s = tool_signal.read();
        (
            s.tools.clone(),
            s.loading,
            s.error.clone(),
            s.call_result.clone(),
        )
    };
    let call_count = call_signal.read().calls.len();

    // Read capability state BEFORE rsx! (no let bindings inside element children)
    let cap_state = cap_signal.read();
    let cap_loading = cap_state.loading;
    let cap_eff = cap_state.effective_tools.clone();
    let cap_base = cap_state.base_tools.clone();
    let cap_avail = cap_state.available_tools.clone();
    let cap_dirt = *cap_dirty.read();
    let cap_has = !cap_avail.is_empty() || !cap_base.is_empty();

    rsx! {
        div { class: "flex-1 overflow-y-auto p-2",
            // System Tools section
            div { class: "mb-3",
                div { class: "flex items-center justify-between mb-2",
                    div { class: "px-2.5 pt-1 pb-2 text-[12px] font-semibold text-[#888] uppercase tracking-[0.5px]",
                        "System Tools ({tools.len()})"
                    }
                    button {
                        class: "px-2 py-0.5 text-[12px] bg-[#3a3a55] text-[#ccc] rounded hover:bg-[#4a4a65]",
                        onclick: {
                            let client = client.clone();
                            let ts = tool_signal.clone();
                            move |_| {
                                let mut ts_mut = ts.clone();
                                ts_mut.with_mut(|s| { s.loading = true; s.error = None; });
                                let ts2 = ts.clone();
                                client.tool_list(move |result| {
                                    let mut ts2 = ts2;
                                    ts2.with_mut(|s| {
                                        s.loading = false;
                                        match result {
                                            Ok(tools) => {
                                                s.tools = tools.iter()
                                                    .filter_map(|t| serde_json::from_value::<ToolDef>(t.clone()).ok())
                                                    .collect();
                                            }
                                            Err(e) => { s.error = Some(e); }
                                        }
                                    });
                                });
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
                                    let client = client.clone();
                                    let ts = tool_signal.clone();
                                    let name = tool.name.clone();
                                    move |_| {
                                        let args_val = serde_json::json!({});
                                        let ts2 = ts.clone();
                                        client.tool_call(&name, &args_val, move |result| {
                                            let mut ts2 = ts2;
                                            ts2.with_mut(|s| {
                                                match result {
                                                    Ok(val) => s.call_result = Some(
                                                        serde_json::to_string_pretty(&val).unwrap_or_else(|_| val.to_string())
                                                    ),
                                                    Err(e) => s.call_result = Some(format!("Error: {e}")),
                                                }
                                            });
                                        });
                                    }
                                },
                                "Run"
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

            // Divider
            div { class: "border-t border-[#333] my-2" }

            // Call History section
            div {
                div { class: "px-2.5 pt-1 pb-2 text-[12px] font-semibold text-[#888] uppercase tracking-[0.5px]", "Call History ({call_count})" }
                div { class: "font-mono text-[13px]",
                    if call_count == 0 {
                        div { class: "p-2.5 text-[#666] text-center", "No tool calls yet" }
                    } else {
                        {(0..call_count).map(|idx| { let s = call_signal.clone(); rsx! { ToolItem { signal: s, index: idx } } }).collect::<Vec<Element>>().into_iter()}
                    }
                }
            }

            // Capability Overlay section
            div { class: "border-t border-[#333] my-2" }
            div {
                div { class: "px-2.5 pt-1 pb-2 text-[12px] font-semibold text-[#888] uppercase tracking-[0.5px]",
                    "Capability Overlay"
                }
                if cap_loading {
                    div { class: "text-[12px] text-[#888] px-2", "Loading..." }
                } else if cap_has {
                    div { class: "flex gap-2 mb-2",
                        button {
                            class: "px-2 py-0.5 text-[12px] bg-[#3a3a55] text-[#ccc] rounded hover:bg-[#4a4a65] disabled:opacity-40 disabled:cursor-not-allowed",
                            disabled: !cap_dirt,
                            onclick: {
                                let cl = client.clone();
                                let sel = selected_tools_signal.clone();
                                let dirty = cap_dirty.clone();
                                let cs = cap_signal.clone();
                                let ag = agents.clone();
                                let gl = global.clone();
                                move |_| {
                                    let mut sel = sel;
                                    let mut dirty = dirty;
                                    let mut cs = cs;
                                    let agent_id = ag.read().selected.clone().unwrap_or_default();
                                    let session_id = gl.read().session_id.clone();
                                    if agent_id.is_empty() { return; }
                                    let tools: Vec<String> = sel.read().iter().cloned().collect();
                                    cl.agent_update_capabilities(&agent_id, &session_id, tools, vec![], vec![], move |result| {
                                        cs.with_mut(|s| {
                                            match result {
                                                Ok(upd) => { s.effective_tools = upd.effective_tools; s.dirty = false; dirty.set(false); }
                                                Err(e) => { log::error!("Failed to update capabilities: {e}"); }
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
                                let sel = selected_tools_signal.clone();
                                let cs = cap_signal.clone();
                                let dirty = cap_dirty.clone();
                                move |_| {
                                    let mut sel = sel;
                                    let mut dirty = dirty;
                                    let base = cs.read().base_tools.clone();
                                    sel.set(base.into_iter().collect());
                                    dirty.set(true);
                                }
                            },
                            "Reset to default"
                        }
                    }
                    for tool_name in &cap_eff {
                        {
                            let tn = tool_name.clone();
                            let chk = selected_tools_signal.read().contains(&tn);
                            rsx! {
                                div { class: "flex items-center gap-2 py-0.5 px-2",
                                    input {
                                        r#type: "checkbox",
                                        checked: chk,
                                        oninput: {
                                            let sel = selected_tools_signal.clone();
                                            let dirty = cap_dirty.clone();
                                            let name = tn.clone();
                                            move |_| {
                                                let mut sel = sel;
                                                let mut dirty = dirty;
                                                let mut hs = sel.write();
                                                if hs.contains(&name) { hs.remove(&name); } else { hs.insert(name.clone()); }
                                                dirty.set(true);
                                            }
                                        },
                                    }
                                    span { class: "text-[13px] text-[#e0e0e0]", "{tn}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn ToolItem(signal: Signal<ToolState>, index: usize) -> Element {
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
    let label = status_label(status);
    let dur_s = dur.map(|ms| format!(" {}ms", ms)).unwrap_or_default();
    rsx! {
        div { class: "border-b border-[#2a2a44] py-0.5",
            div {
                span { class: "font-semibold text-[13px] text-[#e0e0e0]", "{seq}. [{name}]" }
                span { class: "text-[12px] font-bold ml-2 {scls}", "{label}{dur_s}" }
            }
            if !arg.is_empty() { div { class: "text-[12px] text-[#888] mt-0.5 pl-1 font-mono", "{arg}" } }
        }
    }
}
