//! Workspace panel showing the file tree (legacy flat view).

use dioxus::prelude::*;

use crate::state::{WorkspaceState, WorkspaceTreeNode};

/// Flatten a WorkspaceTreeNode tree into (name, is_dir, indent) tuples.
fn flatten_tree(node: &WorkspaceTreeNode, indent: usize) -> Vec<(String, bool, usize)> {
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
    let ws: Signal<WorkspaceState> = use_context();
    let (entries, loaded) = {
        let ui = ws.read();
        (flatten_tree(&ui.workspace, 0), ui.workspace.loaded)
    };

    if entries.is_empty() && !loaded {
        return rsx! {
            div { class: "flex-1 overflow-y-auto p-2.5",
                div { class: "flex items-center justify-center h-full text-[#666]", "Workspace directory empty or unavailable" }
            }
        };
    }

    let items = entries
        .iter()
        .enumerate()
        .map(|(index, (name, is_dir, indent))| {
            let n = name.clone();
            let d = *is_dir;
            let i = *indent;
            rsx! {
                WorkspaceItem { name: n, is_dir: d, indent: i, key: "{index}" }
            }
        })
        .collect::<Vec<_>>();

    rsx! {
        div { class: "flex-1 overflow-y-auto p-2.5",
            {items.into_iter()}
        }
    }
}

#[component]
fn WorkspaceItem(name: String, is_dir: bool, indent: usize) -> Element {
    if is_dir {
        let display = format!("{}[DIR] {}", "  ".repeat(indent), name);
        rsx! {
            div { class: "py-0.5 font-mono text-[13px] text-[#6090ff] font-bold", "{display}" }
        }
    } else {
        let display = format!("{}[FILE] {}", "  ".repeat(indent), name);
        rsx! {
            div { class: "py-0.5 font-mono text-[13px] text-[#e0e0e0]", "{display}" }
        }
    }
}
