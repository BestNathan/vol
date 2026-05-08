//! Workspace file tree browser.

use dioxus::prelude::*;

use crate::web::components::app::AppState;

/// Workspace panel showing the file tree.
#[component]
pub fn WorkspacePanel() -> Element {
    let state: AppState = use_context();
    let count = state.ui_state.peek().workspace.entries.len();

    if count == 0 {
        return rsx! {
            div { class: "workspace-panel",
                div { class: "workspace-empty", "Workspace directory empty or unavailable" }
            }
        };
    }

    let items = (0..count).map(|index| {
        let s = state.clone();
        rsx! {
            WorkspaceItem { index, state: s }
        }
    }).collect::<Vec<_>>();

    rsx! {
        div { class: "workspace-panel",
            {items.into_iter()}
        }
    }
}

#[component]
fn WorkspaceItem(state: AppState, index: usize) -> Element {
    let (is_dir, name, indent, modified) = {
        let ui = state.ui_state.peek();
        match ui.workspace.entries.get(index) {
            Some(e) => (
                e.is_dir,
                e.path.split('/').last().unwrap_or(&e.path).to_string(),
                e.indent,
                e.modified,
            ),
            None => return rsx! {},
        }
    };

    if is_dir {
        let display = format!("{}[DIR] {}", "  ".repeat(indent), name);
        rsx! {
            div { class: "workspace-entry workspace-dir", "{display}" }
        }
    } else {
        let mod_marker = if modified { " M" } else { "" };
        let display = format!("{}[FILE] {}{}", "  ".repeat(indent), name, mod_marker);
        let cls = if modified {
            "workspace-entry workspace-file-modified"
        } else {
            "workspace-entry workspace-file"
        };
        rsx! {
            div { class: cls, "{display}" }
        }
    }
}
