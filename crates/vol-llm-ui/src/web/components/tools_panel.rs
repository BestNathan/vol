//! Left panel showing tool calls with status indicators.

use dioxus::prelude::*;
use crate::state::{ToolState, ToolCallEntry, ToolCallStatus, UiEvent, UiEventKind, SubscriptionSet};
use crate::web::client::{ConnectionState, JsonRpcClient};
use crate::web::components::app::AppState;

#[derive(Debug, Clone, serde::Deserialize)]
struct ToolDef {
    name: String,
    description: Option<String>,
    parameters: Option<serde_json::Value>,
}

struct ToolPanelState {
    tools: Vec<ToolDef>,
    loading: bool,
    error: Option<String>,
    call_result: Option<String>,
}

fn update_status(calls: &mut Vec<ToolCallEntry>, name: &str, status: ToolCallStatus, dur: Option<u64>) {
    for e in calls.iter_mut().rev() {
        if e.tool_name == name && matches!(e.status, ToolCallStatus::Running) {
            e.status = status; e.duration_ms = dur; break;
        }
    }
}

fn arg_preview(arguments: &str) -> String {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(arguments) {
        if let Some(c) = v.get("command").and_then(|v| v.as_str()) {
            return if c.chars().count() > 80 { format!("Command: {}...", c.chars().take(77).collect::<String>()) } else { format!("Command: {}", c) };
        }
        if let Some(p) = v.get("path").and_then(|v| v.as_str()) { return format!("Path: {}", p); }
        if let Some(f) = v.get("file_path").and_then(|v| v.as_str()) { return format!("File: {}", f); }
        if arguments.chars().count() > 80 { return format!("Args: {}...", arguments.chars().take(77).collect::<String>()); }
        return format!("Args: {}", arguments);
    }
    String::new()
}

pub fn status_label(s: ToolCallStatus) -> &'static str {
    match s { ToolCallStatus::Running => "...", ToolCallStatus::Success => "OK", ToolCallStatus::Error => "ERR", ToolCallStatus::Skipped => "SKIP" }
}

/// Update ToolState from an EventBus event.
fn reduce_tool_state(s: &mut ToolState, event: &UiEvent) {
    match event {
        UiEvent::ToolCallBegin { tool_name, arguments } => {
            let seq = s.calls.len() as u32 + 1;
            s.calls.push(ToolCallEntry { sequence: seq, tool_name: tool_name.clone(), arg_preview: arg_preview(arguments), status: ToolCallStatus::Running, duration_ms: None });
            s.scroll = s.calls.len() as u16;
        }
        UiEvent::ToolCallComplete { tool_name, duration_ms, .. } => update_status(&mut s.calls, tool_name, ToolCallStatus::Success, *duration_ms),
        UiEvent::ToolCallError { tool_name, duration_ms, .. } => update_status(&mut s.calls, tool_name, ToolCallStatus::Error, *duration_ms),
        UiEvent::ToolCallSkipped { tool_name, duration_ms, .. } => update_status(&mut s.calls, tool_name, ToolCallStatus::Skipped, *duration_ms),
        _ => {}
    }
}

#[component]
pub fn ToolsPanel() -> Element {
    let app_state: AppState = use_context();
    let signal = use_signal(|| ToolState::new());
    let tool_state = use_signal(|| ToolPanelState { tools: vec![], loading: false, error: None, call_result: None });
    let client: JsonRpcClient = app_state.rpc_client.clone();

    // EventBus for subscriptions (clone before hooks consume it)
    let event_bus = app_state.event_bus.clone();
    let event_bus2 = event_bus.clone();

    // Fetch tools: runs initially via use_effect, runs on reconnect via EventBus
    let fetch_tools = {
        let client = client.clone();
        let ts = tool_state.clone();
        move || {
            if client.state() == ConnectionState::Connected {
                ts.write_unchecked().loading = true;
                client.tool_list({
                    let ts2 = ts.clone();
                    move |result| {
                        let mut s = ts2.write_unchecked();
                        s.loading = false;
                        match result {
                            Ok(tools) => {
                                s.tools = tools.iter()
                                    .filter_map(|t| serde_json::from_value::<ToolDef>(t.clone()).ok())
                                    .collect();
                            }
                            Err(e) => { s.error = Some(e); }
                        }
                    }
                });
            }
        }
    };
    let fetch_tools_for_effect = fetch_tools.clone();

    // Subscribe to WsConnected to re-fetch on reconnect
    use_hook(move || {
        let sub = event_bus.subscribe(UiEventKind::WsConnected, move |_| { fetch_tools(); });
        std::sync::Arc::new(sub)
    });

    // Initial fetch on mount
    use_effect(move || { fetch_tools_for_effect(); });

    // Tool call event subscriptions
    use_hook(move || {
        let bus = event_bus2;
        let mut set = SubscriptionSet::new(bus.clone());
        for kind in [UiEventKind::ToolCallBegin, UiEventKind::ToolCallComplete, UiEventKind::ToolCallError, UiEventKind::ToolCallSkipped] {
            set.subscribe(&bus, kind, {
                let signal = signal.clone();
                move |event| {
                    reduce_tool_state(&mut *signal.write_unchecked(), event);
                }
            });
        }
        std::sync::Arc::new(set)
    });

    let count = signal.read().calls.len();
    rsx! {
        div { class: "flex-1 overflow-y-auto p-2",
            // System Tools section
            div { class: "mb-3",
                div { class: "flex items-center justify-between mb-2",
                    div { class: "px-2.5 pt-1 pb-2 text-[12px] font-semibold text-[#888] uppercase tracking-[0.5px]", "System Tools" }
                    button {
                        class: "px-2 py-0.5 text-[12px] bg-[#3a3a55] text-[#ccc] rounded hover:bg-[#4a4a65]",
                        onclick: {
                            let client = client.clone();
                            let ts = tool_state.clone();
                            move |_| {
                                ts.write_unchecked().loading = true;
                                ts.write_unchecked().error = None;
                                let ts_clone = ts.clone();
                                client.tool_list(move |result: Result<Vec<serde_json::Value>, String>| {
                                    let mut s = ts_clone.write_unchecked();
                                    s.loading = false;
                                    match result {
                                        Ok(tools) => {
                                            s.tools = tools.iter()
                                                .filter_map(|t: &serde_json::Value| serde_json::from_value::<ToolDef>(t.clone()).ok())
                                                .collect();
                                        }
                                        Err(e) => s.error = Some(e),
                                    }
                                });
                            }
                        },
                        "Fetch Tools"
                    }
                }
                {tool_state.read().loading.then(|| rsx! { div { class: "text-[12px] text-[#888] px-2", "Loading..." } })}
                {tool_state.read().error.as_ref().map(|e| rsx! { div { class: "text-[12px] text-[#c04040] px-2", "{e}" } })}
                for tool in &tool_state.read().tools {
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
                                    let ts = tool_state.clone();
                                    let name = tool.name.clone();
                                    move |_| {
                                        let args_val = serde_json::json!({});
                                        let ts_clone = ts.clone();
                                        client.tool_call(&name, &args_val, move |result: Result<serde_json::Value, String>| {
                                            let mut s = ts_clone.write_unchecked();
                                            match result {
                                                Ok(val) => s.call_result = Some(
                                                    serde_json::to_string_pretty(&val).unwrap_or_else(|_| val.to_string())
                                                ),
                                                Err(e) => s.call_result = Some(format!("Error: {e}")),
                                            }
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
            if let Some(ref result) = tool_state.read().call_result {
                div { class: "mb-2",
                    div { class: "text-[12px] font-semibold text-[#888] mb-1", "Call Result" }
                    pre { class: "text-[12px] font-mono text-[#ccc] bg-[#1a1a2e] p-2 rounded overflow-x-auto whitespace-pre-wrap", "{result}" }
                }
            }

            // Divider
            div { class: "border-t border-[#333] my-2" }

            // Call History section
            div {
                div { class: "px-2.5 pt-1 pb-2 text-[12px] font-semibold text-[#888] uppercase tracking-[0.5px]", "Call History ({count})" }
                div { class: "font-mono text-[13px]",
                    if count == 0 {
                        div { class: "p-2.5 text-[#666] text-center", "No tool calls yet" }
                    } else {
                        {(0..count).map(|idx| { let s = signal.clone(); rsx! { ToolItem { signal: s, index: idx } } }).collect::<Vec<Element>>().into_iter()}
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
            Some(e) => (e.sequence, e.tool_name.clone(), e.arg_preview.clone(), e.status.clone(), e.duration_ms),
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
