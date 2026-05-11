//! File content preview shown in the Workspace tab when files are open.

use dioxus::prelude::*;

use crate::state::{OpenFileTab, WorkspaceState};

/// File content viewer showing open file tabs.
#[component]
pub fn FileContentView() -> Element {
    let ws: Signal<WorkspaceState> = use_context();
    let (open_files, selected) = {
        let ui = ws.read();
        (ui.open_files.clone(), ui.selected_file_tab)
    };

    if open_files.is_empty() {
        return rsx! {
            div { class: "file-content-empty",
                "Click a file in the explorer to open it"
            }
        };
    }

    let tab_elements: Vec<Element> = open_files
        .iter()
        .enumerate()
        .map(|(i, tab)| render_tab(i, tab, ws))
        .collect();

    rsx! {
        div { class: "file-content-view",
            div { class: "file-tab-bar",
                {tab_elements.into_iter()}
            }
            {if let Some(idx) = selected {
                if let Some(tab) = open_files.get(idx) {
                    match (&tab.content, &tab.error) {
                        (Some(content), _) => rsx! { FileContentDisplay { content } },
                        (None, Some(error)) => rsx! {
                            div { class: "file-content-error", "Error: {error}" }
                        },
                        (None, None) => rsx! {
                            div { class: "file-content-loading", "Loading..." }
                        },
                    }
                } else {
                    rsx! {}
                }
            } else {
                rsx! {}
            }}
        }
    }
}

fn render_tab(i: usize, tab: &OpenFileTab, ws: Signal<WorkspaceState>) -> Element {
    let name = tab.path.split('/').last().unwrap_or(&tab.path).to_string();
    let icon = crate::web::components::file_tree::file_icon(false, &name);
    let path = tab.path.clone();

    let is_selected = {
        let ui = ws.read();
        Some(i) == ui.selected_file_tab
    };
    let tab_cls = if is_selected { "file-tab active" } else { "file-tab" };

    let mut sig_select = ws;
    let select_onclick = {
        move |_: Event<MouseData>| {
            sig_select.with_mut(|s| {
                s.selected_file_tab = Some(i);
            });
        }
    };

    let close_path = path.clone();
    let mut sig_close = ws;
    let close_onclick = move |evt: Event<MouseData>| {
        evt.stop_propagation();
        sig_close.with_mut(|s| {
            if let Some(pos) = s.open_files.iter().position(|t| t.path == close_path) {
                s.open_files.remove(pos);
                if s.open_files.is_empty() {
                    s.selected_file_tab = None;
                } else if s.selected_file_tab == Some(pos) {
                    let new_len = s.open_files.len();
                    s.selected_file_tab = Some(pos.min(new_len.saturating_sub(1)));
                } else if s.selected_file_tab.map(|s| s > pos).unwrap_or(false) {
                    s.selected_file_tab = s.selected_file_tab.map(|s| s - 1);
                }
            }
        });
    };

    rsx! {
        div {
            class: tab_cls,
            key: "{path}",
            onclick: select_onclick,
            span { class: "file-tab-icon", "{icon}" }
            span { class: "file-tab-name", "{name}" }
            span {
                class: "file-tab-close",
                onclick: close_onclick,
                "\u{2715}"
            }
        }
    }
}

#[component]
fn FileContentDisplay(content: String) -> Element {
    rsx! {
        pre { class: "file-content",
            {content}
        }
    }
}
