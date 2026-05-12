//! Left panel showing tool calls with status indicators.

use dioxus::prelude::*;
use crate::state::{ToolState, ToolCallEntry, ToolCallStatus, UiEvent, UiEventKind, SubscriptionSet};
use crate::web::components::app::AppState;

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

    use_hook(move || {
        let bus = app_state.event_bus.clone();
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
            div { class: "px-2.5 pt-1 pb-2 text-[12px] font-semibold text-[#888] uppercase tracking-[0.5px]", "Tools Called ({count})" }
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
