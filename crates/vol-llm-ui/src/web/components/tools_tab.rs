//! Tools tab with expandable tool call details.

use dioxus::prelude::*;

use crate::web::components::app::{AppState, status_class, status_label};

/// Tools tab content showing expandable tool calls.
#[component]
pub fn ToolsTabContent() -> Element {
    let state: AppState = use_context();
    let count = state.ui_state.borrow().tool_calls.len();

    if count == 0 {
        return rsx! {
            div { class: "tools-tab",
                div { class: "tools-tab-empty", "No tool calls yet" }
            }
        };
    }

    let items: Vec<Element> = (0..count).map(|idx| {
        let s = state.clone();
        rsx! { ToolCallItem { index: idx, state: s } }
    }).collect();

    rsx! {
        div { class: "tools-tab",
            {items.into_iter()}
        }
    }
}

/// A single tool call row, expandable to show input/output.
#[component]
fn ToolCallItem(index: usize, state: AppState) -> Element {
    let (seq, tool_name, arg_preview, status, duration_ms) = {
        let ui = state.ui_state.borrow();
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

    let is_expanded = state.ui_state.borrow().expanded_tool_calls.contains(&index);
    let scls = status_class(status.clone());
    let label = status_label(status);
    let dur = duration_ms.map(|ms| format!("{ms}ms")).unwrap_or_default();

    rsx! {
        div { class: "tool-call-item",
            div {
                class: "tool-call-header",
                onclick: move |_: Event<MouseData>| {
                    let ui = state.ui_state.clone();
                    let mut ver = state.version;
                    let idx = index;
                    if let Ok(mut s) = ui.try_borrow_mut() {
                        if s.expanded_tool_calls.contains(&idx) {
                            s.expanded_tool_calls.remove(&idx);
                        } else {
                            s.expanded_tool_calls.insert(idx);
                        }
                    }
                    let v = (*ver.peek()).wrapping_add(1);
                    ver.set(v);
                },
                span { class: "tool-call-seq", "{seq}." }
                span { class: "tool-call-name", "[{tool_name}]" }
                span { class: "tool-call-status {scls}", "{label}" }
                if !dur.is_empty() {
                    span { class: "tool-call-duration", "{dur}" }
                }
                span { class: "tool-call-chevron", "▾" }
            }
            if is_expanded {
                div { class: "tool-call-detail",
                    div {
                        span { class: "tool-detail-label", "Input: " }
                        "{arg_preview}"
                    }
                }
            }
        }
    }
}
