//! Workspace panel showing the file tree (legacy flat view).

use dioxus::prelude::*;

use crate::web::components::app::AppState;

/// Flatten a WorkspaceTreeNode tree into (name, is_dir, indent) tuples.
fn flatten_tree(node: &crate::state::WorkspaceTreeNode, indent: usize) -> Vec<(String, bool, usize)> {
    let mut result = Vec::new();
    for child in &node.children {
        result.push((child.name.clone(), child.is_dir, indent));
        if child.is_dir {
            result.extend(flatten_tree(child, indent + 1));
        }
    }
    result
}

/// Workspace panel showing the file tree.
#[component]
pub fn WorkspacePanel() -> Element {
    let state: AppState = use_context();
    let (entries, loaded) = {
        let ui = state.signal.read();
        (flatten_tree(&ui.workspace, 0), ui.workspace.loaded)
    };

    if entries.is_empty() && !loaded {
        return rsx! {
            div { class: "workspace-panel",
                div { class: "workspace-empty", "Workspace directory empty or unavailable" }
            }
        };
    }

    let items = entries.iter().enumerate().map(|(index, (name, is_dir, indent))| {
        let n = name.clone();
        let d = *is_dir;
        let i = *indent;
        rsx! {
            WorkspaceItem { name: n, is_dir: d, indent: i, key: "{index}" }
        }
    }).collect::<Vec<_>>();

    rsx! {
        div { class: "workspace-panel",
            {items.into_iter()}
        }
    }
}

#[component]
fn WorkspaceItem(name: String, is_dir: bool, indent: usize) -> Element {
    if is_dir {
        let display = format!("{}[DIR] {}", "  ".repeat(indent), name);
        rsx! {
            div { class: "workspace-entry workspace-dir", "{display}" }
        }
    } else {
        let display = format!("{}[FILE] {}", "  ".repeat(indent), name);
        rsx! {
            div { class: "workspace-entry workspace-file", "{display}" }
        }
    }
}
