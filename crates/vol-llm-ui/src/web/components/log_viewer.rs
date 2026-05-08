//! Log run viewer with event details.

use dioxus::prelude::*;

use crate::web::components::app::AppState;

/// Log viewer panel.
///
/// Shows either the list of log runs or the entries of a selected run.
#[component]
pub fn LogViewer() -> Element {
    let state: AppState = use_context();
    let (selected_run, entries, run_logs) = {
        let ui = state.ui_state.peek();
        (
            ui.log_viewer_selected_run.clone(),
            ui.log_viewer_entries.len(),
            ui.log_viewer_run_logs.len(),
        )
    };

    match selected_run {
        Some(run_id) => render_log_entries(&run_id, entries, state),
        None => render_run_list(run_logs, state),
    }
}

fn render_run_list(count: usize, state: AppState) -> Element {
    if count == 0 {
        return rsx! {
            div { class: "log-viewer",
                div { class: "log-empty", "No log files found." }
            }
        };
    }

    let items = (0..count).map(|index| {
        let s = state.clone();
        rsx! {
            LogRunItem { index, state: s }
        }
    }).collect::<Vec<_>>();

    rsx! {
        div { class: "log-viewer log-run-list",
            {items.into_iter()}
        }
    }
}

#[component]
fn LogRunItem(state: AppState, index: usize) -> Element {
    let run = {
        let ui = state.ui_state.peek();
        match ui.log_viewer_run_logs.get(index) {
            Some(e) => e.clone(),
            None => return rsx! {},
        }
    };

    let short_id = if run.run_id.len() > 12 {
        format!("{}...", &run.run_id[..9])
    } else {
        run.run_id.clone()
    };

    rsx! {
        div { class: "log-run-item",
            span { class: "log-run-item-id", "{short_id}" }
            span { class: "log-run-item-count", " {run.event_count} events" }
            span { class: "log-run-item-count", "  {run.last_event} ({run.last_event_time})" }
        }
    }
}

fn render_log_entries(run_id: &str, count: usize, state: AppState) -> Element {
    if count == 0 {
        return rsx! {
            div { class: "log-viewer",
                div { class: "log-empty", "No events in this run." }
            }
        };
    }

    let run_id = run_id.to_string();
    let items = (0..count).map(|index| {
        let s = state.clone();
        rsx! {
            LogEntryItem { index, state: s }
        }
    }).collect::<Vec<_>>();

    rsx! {
        div { class: "log-viewer",
            div { style: "margin-bottom: 8px; font-size: 12px; color: #888;", "Log: {run_id}" }
            {items.into_iter()}
        }
    }
}

#[component]
fn LogEntryItem(state: AppState, index: usize) -> Element {
    let entry = {
        let ui = state.ui_state.peek();
        match ui.log_viewer_entries.get(index) {
            Some(e) => e.clone(),
            None => return rsx! {},
        }
    };

    let color = match entry.event_type.as_str() {
        "AgentStart" | "AgentComplete" => "#40c040",
        "ToolCallBegin" | "ToolCallComplete" => "#c0c040",
        "ToolCallError" | "AgentAborted" => "#c04040",
        _ => "#e0e0e0",
    };

    rsx! {
        div { class: "log-entry",
            span { class: "log-entry-time", "[{entry.timestamp}] " }
            span { class: "log-entry-type", style: "color: {color};", "{entry.event_type}" }
            span { style: "color: {color};", " -- {entry.summary}" }
        }
    }
}
