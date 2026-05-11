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
        return rsx! { div { class: "tools-tab", div { class: "tools-tab-empty", "No tool calls yet" } } };
    }
    let items: Vec<Element> = (0..count).map(|idx| {
        let s = signal.clone();
        rsx! { ToolCallItem { signal: s, index: idx } }
    }).collect();
    rsx! { div { class: "tools-tab", {items.into_iter()} } }
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
    let scls = match status { ToolCallStatus::Running => "status-running", ToolCallStatus::Success => "status-success", ToolCallStatus::Error => "status-error", ToolCallStatus::Skipped => "status-skipped" };
    let label = match status { ToolCallStatus::Running => "...", ToolCallStatus::Success => "OK", ToolCallStatus::Error => "ERR", ToolCallStatus::Skipped => "SKIP" };
    let dur_s = dur.map(|ms| format!("{ms}ms")).unwrap_or_default();
    rsx! {
        div { class: "tool-call-item",
            div { class: "tool-call-header",
                onclick: move |_: Event<MouseData>| {
                    let mut state = signal.write_unchecked();
                    if state.expanded.contains(&index) {
                        state.expanded.remove(&index);
                    } else {
                        state.expanded.insert(index);
                    }
                },
                span { class: "tool-call-seq", "{seq}." }
                span { class: "tool-call-name", "[{name}]" }
                span { class: "tool-call-status {scls}", "{label}" }
                if !dur_s.is_empty() { span { class: "tool-call-duration", "{dur_s}" } }
                span { class: "tool-call-chevron", "▾" }
            }
            if is_expanded { div { class: "tool-call-detail", div { span { class: "tool-detail-label", "Input: " } "{arg}" } } }
        }
    }
}
