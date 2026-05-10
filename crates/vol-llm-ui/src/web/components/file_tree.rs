//! Left sidebar file tree with collapsible directories.

use std::collections::BTreeMap;

use dioxus::prelude::*;

use crate::state::{ActiveTab, OpenFileTab};
use crate::web::components::app::AppState;

/// Get the icon for a file extension or directory.
pub(crate) fn file_icon(is_dir: bool, name: &str) -> &'static str {
    if is_dir {
        return "\u{1f4c2}";
    }
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "rs" => "\u{1f980}",
        "toml" | "lock" => "\u{2699}\u{fe0f}",
        "md" => "\u{1f4dd}",
        "json" => "\u{1f4ca}",
        "yaml" | "yml" => "\u{1f4dc}",
        "sh" | "bash" => "\u{1f41a}",
        "html" | "htm" => "\u{1f310}",
        "css" => "\u{1f3a8}",
        "js" | "ts" | "jsx" | "tsx" => "\u{1f4dc}",
        "txt" => "\u{1f4c4}",
        _ => "\u{1f4c4}",
    }
}

/// Tree node computed from flat workspace entries.
#[derive(Clone, PartialEq)]
enum FileTreeNode {
    Dir { name: String, path: String, children: Vec<FileTreeNode> },
    File { name: String, path: String },
}

/// Build a tree from flat workspace entries.
fn build_tree(entries: &[crate::state::WorkspaceEntry]) -> Vec<FileTreeNode> {
    build_tree_at(entries, "")
}

fn build_tree_at(entries: &[crate::state::WorkspaceEntry], prefix: &str) -> Vec<FileTreeNode> {
    let mut files = Vec::new();
    let mut dirs: BTreeMap<String, Vec<crate::state::WorkspaceEntry>> = BTreeMap::new();

    for entry in entries {
        let relative = if prefix.is_empty() {
            entry.path.as_str()
        } else if entry.path.starts_with(&format!("{}/", prefix)) {
            &entry.path[prefix.len() + 1..]
        } else {
            continue;
        };

        let first = relative.split('/').next().unwrap_or("");
        if relative.contains('/') {
            let full = if prefix.is_empty() {
                first.to_string()
            } else {
                format!("{}/{}", prefix, first)
            };
            dirs.entry(full).or_default().push(entry.clone());
        } else {
            files.push(FileTreeNode::File {
                name: entry.path.split('/').last().unwrap_or("").to_string(),
                path: entry.path.clone(),
            });
        }
    }

    let mut result = files;
    for (dir_path, dir_entries) in dirs {
        let name = dir_path.split('/').last().unwrap_or("").to_string();
        let children = build_tree_at(&dir_entries, &dir_path);
        result.push(FileTreeNode::Dir { name, path: dir_path, children });
    }
    result
}

fn bump_version(ver: &mut Signal<u64>) {
    let v = *ver.peek();
    ver.set(v.wrapping_add(1));
}

/// Render tree nodes recursively.
fn render_nodes(nodes: Vec<FileTreeNode>, state: AppState, depth: usize) -> Vec<Element> {
    nodes
        .into_iter()
        .map(|node| render_node(node, state.clone(), depth))
        .collect()
}

fn render_node(node: FileTreeNode, state: AppState, depth: usize) -> Element {
    match node {
        FileTreeNode::Dir { name, path, children } => {
            let collapsed = state.ui_state.borrow().collapsed_dirs.contains(&path);

            let child_elements = if !collapsed {
                render_nodes(children, state.clone(), depth + 1)
            } else {
                Vec::new()
            };

            let indent_px = depth * 16;
            let chevron_cls = if collapsed {
                "file-tree-chevron collapsed"
            } else {
                "file-tree-chevron"
            };

            let ui = state.ui_state.clone();
            let mut ver = state.version;
            let dir_path = path.clone();
            let dir_onclick = move |_: Event<MouseData>| {
                let p = dir_path.clone();
                if let Ok(mut s) = ui.try_borrow_mut() {
                    if s.collapsed_dirs.contains(&p) {
                        s.collapsed_dirs.remove(&p);
                    } else {
                        s.collapsed_dirs.insert(p);
                    }
                }
                bump_version(&mut ver);
            };

            rsx! {
                div {
                    div {
                        class: "file-tree-node file-tree-dir",
                        style: format!("padding-left: {}px;", indent_px),
                        onclick: dir_onclick,
                        span { class: "{chevron_cls}", "\u{25be}" }
                        span { class: "file-tree-icon", "{file_icon(true, &name)}" }
                        span { class: "file-tree-label dir", "{name}" }
                    }
                    if !collapsed {
                        div { class: "file-tree-children",
                            {child_elements.into_iter()}
                        }
                    }
                }
            }
        }
        FileTreeNode::File { name, path } => {
            let indent_px = depth * 16;

            let ui = state.ui_state.clone();
            let mut ver = state.version;
            let rpc = state.rpc_client.clone();
            let mut tab = state.active_tab;
            let file_path = path.clone();
            let file_onclick = move |_: Event<MouseData>| {
                let p = file_path.clone();
                let rpc_clone = rpc.clone();
                let ui_clone = ui.clone();
                let ver_clone = ver.clone();

                if let Ok(mut s) = ui.try_borrow_mut() {
                    let existing = s.open_files.iter().position(|f| f.path == p);
                    match existing {
                        Some(idx) => {
                            s.selected_file_tab = Some(idx);
                        }
                        None => {
                            let new_idx = s.open_files.len();
                            s.open_files.push(OpenFileTab {
                                path: p.clone(),
                                content: None,
                                error: None,
                            });
                            s.selected_file_tab = Some(new_idx);

                            let ui2 = ui_clone.clone();
                            let mut ver2 = ver_clone.clone();
                            let read_path = p.clone();
                            rpc_clone.file_read(&p, move |result| {
                                if let Ok(mut st) = ui2.try_borrow_mut() {
                                    if let Some(idx) = st.open_files.iter().position(|f| f.path == read_path) {
                                        match result {
                                            Ok(c) => { st.open_files[idx].content = Some(c); }
                                            Err(e) => { st.open_files[idx].error = Some(e); }
                                        }
                                    }
                                }
                                bump_version(&mut ver2);
                            });
                        }
                    }
                }
                tab.set(ActiveTab::Workspace);
                bump_version(&mut ver);
            };

            rsx! {
                div {
                    class: "file-tree-node file-tree-file",
                    style: format!("padding-left: {}px;", indent_px),
                    onclick: file_onclick,
                    span { class: "file-tree-chevron hidden", "\u{25be}" }
                    span { class: "file-tree-icon", "{file_icon(false, &name)}" }
                    span { class: "file-tree-label file", "{name}" }
                }
            }
        }
    }
}

/// File tree component.
#[component]
pub fn FileTree() -> Element {
    let state: AppState = use_context();
    let tree = {
        let ui = state.ui_state.borrow();
        build_tree(&ui.workspace.entries)
    };

    if tree.is_empty() {
        return rsx! {
            div { class: "sidebar",
                div { class: "sidebar-header", "Explorer" }
                div { class: "file-tree",
                    div { class: "file-tree-empty", "No files loaded" }
                }
            }
        };
    }

    let elements = render_nodes(tree, state, 0);

    rsx! {
        div { class: "sidebar",
            div { class: "sidebar-header", "Explorer" }
            div { class: "file-tree",
                {elements.into_iter()}
            }
        }
    }
}
