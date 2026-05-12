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
            div { class: "flex items-center justify-center h-full text-[#666]",
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
        div { class: "flex-1 flex flex-col overflow-hidden",
            div { class: "flex bg-[#1e1e38] border-b border-[#2a2a44] flex-shrink-0 overflow-x-auto",
                {tab_elements.into_iter()}
            }
            {if let Some(idx) = selected {
                if let Some(tab) = open_files.get(idx) {
                    match (&tab.content, &tab.error) {
                        (Some(content), _) => rsx! { FileContentDisplay { content } },
                        (None, Some(error)) => rsx! {
                            div { class: "p-3 text-[#ff6060] font-bold", "Error: {error}" }
                        },
                        (None, None) => rsx! {
                            div { class: "flex items-center justify-center h-full text-[#888]", "Loading..." }
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
    let tab_cls = if is_selected {
        "px-2 py-1 text-[12px] text-[#e0e0e0] bg-[#1a1a2e] flex items-center gap-1 cursor-pointer border-b-2 border-b-[#80a0ff] whitespace-nowrap"
    } else {
        "px-2 py-1 text-[12px] text-[#777] flex items-center gap-1 cursor-pointer border-b-2 border-transparent whitespace-nowrap hover:text-[#bbb] hover:bg-[#222240]"
    };

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
            span { class: "text-[13px]", "{icon}" }
            span { class: "max-w-[150px] overflow-hidden text-ellipsis", "{name}" }
            span {
                class: "text-[10px] text-[#555] px-0.5 rounded-[2px] leading-none hover:text-[#ff6060] hover:bg-[#3a2020]",
                onclick: close_onclick,
                "\u{2715}"
            }
        }
    }
}

#[component]
fn FileContentDisplay(content: String) -> Element {
    rsx! {
        pre { class: "flex-1 overflow-auto p-3 font-mono text-[12px] leading-[1.6] text-[#c8c8e0] bg-[#1a1a2e] whitespace-pre m-0",
            {content}
        }
    }
}
