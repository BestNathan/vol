//! Left panel showing tool calls with status indicators.

use dioxus::prelude::*;

use crate::web::components::app::{AppState, status_class, status_label};

/// Tools panel listing all tool calls with status badges.
#[component]
pub fn ToolsPanel() -> Element {
    let state: AppState = use_context();
    let count = state.ui_state.peek().tool_calls.len();

    rsx! {
        div { class: "tools-panel",
            div { class: "tools-panel-header",
                "Tools Called ({count})"
            }
            div { class: "tools-panel-list",
                if count == 0 {
                    div { style: "padding: 10px; color: #666; text-align: center;", "No tool calls yet" }
                } else {
                    {render_tool_items(state, count).into_iter()}
                }
            }
        }
    }
}

#[component]
fn ToolItem(state: AppState, index: usize) -> Element {
    let (seq, tool_name, arg_preview, status, duration_ms) = {
        let ui = state.ui_state.peek();
        match ui.tool_calls.get(index) {
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

    let scls = status_class(status.clone());
    let label = status_label(status);
    let dur = duration_ms
        .map(|ms| format!(" {}ms", ms))
        .unwrap_or_default();

    rsx! {
        div { class: "tool-item",
            div {
                span { class: "tool-item-name",
                    "{seq}. [{tool_name}]"
                }
                span { class: "tool-item-status {scls}", "{label}" }
                if !dur.is_empty() {
                    span { style: "color: #888; font-size: 11px; margin-left: 6px;", "{dur}" }
                }
            }
            if !arg_preview.is_empty() {
                div { class: "tool-item-arg", "{arg_preview}" }
            }
        }
    }
}

fn render_tool_items(state: AppState, count: usize) -> Vec<Element> {
    (0..count).map(|index| {
        let s = state.clone();
        rsx! {
            ToolItem { index, state: s }
        }
    }).collect()
}
