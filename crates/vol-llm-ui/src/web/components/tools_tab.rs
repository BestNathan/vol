//! Tools tab with system tool listing and tool call history.

use dioxus::prelude::*;
use crate::state::{ToolState, ToolCallEntry, ToolCallStatus, UiEvent, UiEventKind};
use crate::web::client::JsonRpcClient;
use crate::web::components::app::AppState;

/// Safely write to a Signal in an async callback.
///
/// When a component unmounts (e.g. tab switch), its signals are dropped.
/// Async callbacks (WebSocket reconnect, RPC responses) may still fire
/// after unmount and panic on `with_mut()`. This function catches that
/// panic and silently returns false instead of crashing the WASM module.
fn safe_write<T>(mut sig: Signal<T>, f: impl FnOnce(&mut T)) -> bool {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        sig.with_mut(f);
    })).is_ok()
}

#[derive(Debug, Clone, serde::Deserialize)]
struct ToolDef {
    name: String,
    description: Option<String>,
    #[allow(dead_code)]
    parameters: Option<serde_json::Value>,
}

struct ToolsPanelState {
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

pub fn reduce_tool_state(s: &mut ToolState, event: &UiEvent) {
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
pub fn ToolsTabContent() -> Element {
    let app_state: AppState = use_context();
    let call_signal: Signal<ToolState> = use_context();
    let tool_state = use_signal(|| ToolsPanelState { tools: vec![], loading: false, error: None, call_result: None });
    let client: JsonRpcClient = app_state.rpc_client.clone();

    // Load tools on mount
    let client_for_load = client.clone();
    use_hook(move || {
        let mut sig = tool_state;
        sig.with_mut(|s| { s.loading = true; s.error = None; });
        client_for_load.tool_list(move |result| {
            safe_write(sig, |s| {
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
    });

    // Re-fetch on reconnect
    let event_bus = app_state.event_bus.clone();
    let client_for_reconnect = client.clone();
    let ts_for_reconnect = tool_state;
    use_hook(move || {
        let _sub = event_bus.subscribe(UiEventKind::WsConnected, move |_| {
            let cl = client_for_reconnect.clone();
            let sig = ts_for_reconnect;
            cl.tool_list(move |result| {
                safe_write(sig, |s| {
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
        });
    });

    // Read state
    let (tools, loading, error, call_result) = {
        let s = tool_state.read();
        (s.tools.clone(), s.loading, s.error.clone(), s.call_result.clone())
    };
    let call_count = call_signal.read().calls.len();

    // Pre-build call history items
    let call_items: Vec<Element> = (0..call_count).map(|idx| {
        let s = call_signal.clone();
        rsx! { ToolCallItem { signal: s, index: idx } }
    }).collect();

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
                                let client = client.clone();
                                let ts = tool_state;
                                move |_| {
                                    let ts = ts;
                                    safe_write(ts, |s| { s.loading = true; s.error = None; });
                                    let ts2 = ts;
                                    client.tool_list(move |result| {
                                        safe_write(ts2, |s| {
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
                                        let ts = tool_state;
                                        let name = tool.name.clone();
                                        move |_| {
                                            let args_val = serde_json::json!({});
                                            let ts = ts;
                                            client.tool_call(&name, &args_val, move |result| {
                                                safe_write(ts, |s| {
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
                {call_items.into_iter()}
            }
        }
    }
}

#[component]
fn ToolCallItem(signal: Signal<ToolState>, index: usize) -> Element {
    let is_expanded = signal.read().expanded.contains(&index);
    let (seq, name, arg, status, dur) = {
        let ui = signal.read();
        match ui.calls.get(index) {
            Some(e) => (e.sequence, e.tool_name.clone(), e.arg_preview.clone(), e.status.clone(), e.duration_ms),
            None => return rsx! {},
        }
    };
    let scls = match status { ToolCallStatus::Running => "text-[#c0c040]", ToolCallStatus::Success => "text-[#40c040]", ToolCallStatus::Error => "text-[#c04040]", ToolCallStatus::Skipped => "text-[#888]" };
    let label = match status { ToolCallStatus::Running => "...", ToolCallStatus::Success => "OK", ToolCallStatus::Error => "ERR", ToolCallStatus::Skipped => "SKIP" };
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
