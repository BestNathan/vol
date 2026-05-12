//! Tools tab with expandable tool call details.

use dioxus::prelude::*;
use crate::state::{ToolState, ToolCallEntry, ToolCallStatus, UiEvent};

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
    let signal: Signal<ToolState> = use_context();

    let count = signal.read().calls.len();
    if count == 0 {
        return rsx! { div { class: "flex-1 overflow-y-auto p-2", div { class: "flex items-center justify-center h-full text-[#666]", "No tool calls yet" } } };
    }
    let items: Vec<Element> = (0..count).map(|idx| {
        let s = signal.clone();
        rsx! { ToolCallItem { signal: s, index: idx } }
    }).collect();
    rsx! { div { class: "flex-1 overflow-y-auto p-2", {items.into_iter()} } }
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
