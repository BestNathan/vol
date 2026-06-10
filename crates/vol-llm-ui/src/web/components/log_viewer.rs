//! Log run viewer with event details.

use crate::state::LogViewerState;
use dioxus::prelude::*;

#[component]
pub fn LogViewer() -> Element {
    let signal = use_signal(|| LogViewerState::new());
    let (selected, entries, run_logs) = {
        let ui = signal.read();
        (ui.selected_run.clone(), ui.entries.len(), ui.run_logs.len())
    };
    match selected {
        Some(run_id) => render_log_entries(&run_id, entries, signal),
        None => render_run_list(run_logs, signal),
    }
}

fn render_run_list(count: usize, signal: Signal<LogViewerState>) -> Element {
    if count == 0 {
        return rsx! { div { class: "flex-1 overflow-y-auto p-2.5", div { class: "flex items-center justify-center h-full text-[#666]", "No log files found." } } };
    }
    let items = (0..count)
        .map(|i| {
            let s = signal.clone();
            rsx! { LogRunItem { signal: s, index: i } }
        })
        .collect::<Vec<_>>();
    rsx! { div { class: "flex-1 overflow-y-auto p-2.5 font-mono text-[13px]", {items.into_iter()} } }
}

#[component]
fn LogRunItem(signal: Signal<LogViewerState>, index: usize) -> Element {
    let run = signal.read().run_logs.get(index).cloned();
    let Some(run) = run else {
        return rsx! {};
    };
    let short = if run.run_id.len() > 12 {
        format!("{}...", &run.run_id[..9])
    } else {
        run.run_id.clone()
    };
    rsx! { div { class: "py-0.5 text-[#ccc]", span { class: "text-[#c0c0c0]", "{short}" } span { class: "text-[#888]", " {run.event_count} events" } span { class: "text-[#888]", "  {run.last_event} ({run.last_event_time})" } } }
}

fn render_log_entries(run_id: &str, count: usize, signal: Signal<LogViewerState>) -> Element {
    if count == 0 {
        return rsx! { div { class: "flex-1 overflow-y-auto p-2.5", div { class: "flex items-center justify-center h-full text-[#666]", "No events in this run." } } };
    }
    let run_id = run_id.to_string();
    let items = (0..count)
        .map(|i| {
            let s = signal.clone();
            rsx! { LogEntryItem { signal: s, index: i } }
        })
        .collect::<Vec<_>>();
    rsx! { div { class: "flex-1 overflow-y-auto p-2.5", div { class: "mb-2 text-[12px] text-[#888]", "Log: {run_id}" } {items.into_iter()} } }
}

#[component]
fn LogEntryItem(signal: Signal<LogViewerState>, index: usize) -> Element {
    let entry = signal.read().entries.get(index).cloned();
    let Some(entry) = entry else {
        return rsx! {};
    };
    let color = match entry.event_type.as_str() {
        "AgentStart" | "AgentComplete" => "#40c040",
        "ToolCallBegin" | "ToolCallComplete" => "#c0c040",
        "ToolCallError" | "AgentAborted" => "#c04040",
        _ => "#e0e0e0",
    };
    rsx! { div { class: "font-mono text-[12px] py-0.5 whitespace-nowrap", span { class: "text-[#666]", "[{entry.timestamp}] " } span { class: "font-bold", style: "color: {color};", "{entry.event_type}" } span { style: "color: {color};", " -- {entry.summary}" } } }
}
