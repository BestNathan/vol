//! Left sidebar file tree with collapsible directories.

use dioxus::prelude::*;

use crate::state::{ActiveTab, OpenFileTab, WorkspaceTreeNode};
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

/// Render tree nodes recursively.
fn render_nodes(nodes: &[WorkspaceTreeNode], state: AppState, depth: usize) -> Vec<Element> {
    nodes
        .iter()
        .map(|node| render_node(node, state.clone(), depth))
        .collect()
}

fn render_node(node: &WorkspaceTreeNode, state: AppState, depth: usize) -> Element {
    if node.is_dir {
        let collapsed = state.signal.read().collapsed_dirs.contains(&node.path);

        let child_elements = if !collapsed {
            render_nodes(&node.children, state.clone(), depth + 1)
        } else {
            Vec::new()
        };

        let indent_px = depth * 16;
        let chevron_cls = if collapsed {
            "file-tree-chevron collapsed"
        } else {
            "file-tree-chevron"
        };

        let dir_sig = state.signal;
        let dir_path = node.path.clone();
        let rpc = state.rpc_client.clone();
        let dir_onclick = move |_: Event<MouseData>| {
            let p = dir_path.clone();
            let rpc_clone = rpc.clone();
            let mut sig = dir_sig.clone();

            let was_collapsed = sig.with_mut(|s| {
                if s.collapsed_dirs.contains(&p) {
                    s.collapsed_dirs.remove(&p);
                    false
                } else {
                    s.collapsed_dirs.insert(p.clone());
                    true
                }
            });

            if was_collapsed {
                let p_str = p.clone();
                rpc_clone.file_list(&p_str, move |result| {
                    let mut sig2 = sig.clone();
                    match result {
                        Ok(entries) => {
                            let flat_entries: Vec<(String, bool)> = entries
                                .into_iter()
                                .map(|e| (e.name, e.is_dir))
                                .collect();
                            sig2.with_mut(|s2| {
                                s2.workspace.replace_dir_children(&p, flat_entries);
                            });
                        }
                        Err(_) => {
                            sig2.with_mut(|s2| {
                                if let Some(nd) = s2.workspace.find_child_mut(&p) {
                                    nd.children.clear();
                                    nd.loaded = true;
                                    nd.load_error = true;
                                }
                            });
                        }
                    }
                });
            }
        };

        let refresh_sig = state.signal;
        let refresh_path = node.path.clone();
        let refresh_rpc = state.rpc_client.clone();
        let refresh_onclick = move |e: Event<MouseData>| {
            e.stop_propagation();
            let p = refresh_path.clone();
            let rpc_clone = refresh_rpc.clone();
            let mut sig = refresh_sig.clone();

            sig.with_mut(|s| {
                if let Some(nd) = s.workspace.find_child_mut(&p) {
                    nd.children.clear();
                    nd.loaded = false;
                    nd.load_error = false;
                }
            });

            let p_str = p.clone();
            rpc_clone.file_list(&p_str, move |result| {
                let mut sig2 = sig.clone();
                match result {
                    Ok(entries) => {
                        let flat_entries: Vec<(String, bool)> = entries
                            .into_iter()
                            .map(|e| (e.name, e.is_dir))
                            .collect();
                        sig2.with_mut(|s2| {
                            s2.workspace.replace_dir_children(&p, flat_entries);
                        });
                    }
                    Err(_) => {
                        sig2.with_mut(|s2| {
                            if let Some(nd) = s2.workspace.find_child_mut(&p) {
                                nd.children.clear();
                                nd.loaded = true;
                                nd.load_error = true;
                            }
                        });
                    }
                }
            });
        };

        rsx! {
            div {
                div {
                    class: "file-tree-node file-tree-dir",
                    style: format!("padding-left: {}px;", indent_px),
                    onclick: dir_onclick,
                    span { class: "{chevron_cls}", "\u{25be}" }
                    span { class: "file-tree-icon", "{file_icon(true, &node.name)}" }
                    span { class: "file-tree-label dir", "{node.name}" }
                    span { class: "file-tree-refresh", onclick: refresh_onclick, "\u{21bb}" }
                }
                if !collapsed {
                    div { class: "file-tree-children",
                        {child_elements.into_iter()}
                    }
                }
            }
        }
    } else {
        let indent_px = depth * 16;

        let mut sig = state.signal.clone();
        let rpc = state.rpc_client.clone();
        let mut tab = state.active_tab;
        let file_path = node.path.clone();
        let file_onclick = move |_: Event<MouseData>| {
            let p = file_path.clone();
            let rpc_clone = rpc.clone();
            let sig_clone = sig.clone();

            sig.with_mut(|s| {
                let existing = s.open_files.iter().position(|f| f.path == p.clone());
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

                        let mut sig2 = sig_clone.clone();
                        let read_path = p.clone();
                        rpc_clone.file_read(&p, move |result| {
                            sig2.with_mut(|st| {
                                if let Some(idx) = st.open_files.iter().position(|f| f.path == read_path) {
                                    match result {
                                        Ok(c) => { st.open_files[idx].content = Some(c); }
                                        Err(e) => { st.open_files[idx].error = Some(e); }
                                    }
                                }
                            });
                        });
                    }
                }
            });
            tab.set(ActiveTab::Workspace);
        };

        rsx! {
            div {
                class: "file-tree-node file-tree-file",
                style: format!("padding-left: {}px;", indent_px),
                onclick: file_onclick,
                span { class: "file-tree-chevron hidden", "\u{25be}" }
                span { class: "file-tree-icon", "{file_icon(false, &node.name)}" }
                span { class: "file-tree-label file", "{node.name}" }
            }
        }
    }
}

/// File tree component.
#[component]
pub fn FileTree() -> Element {
    let state: AppState = use_context();
    let workspace = state.signal.read().workspace.clone();

    if workspace.children.is_empty() && !workspace.loaded {
        return rsx! {
            div { class: "sidebar",
                div { class: "sidebar-header", "Explorer" }
                div { class: "file-tree",
                    div { class: "file-tree-empty", "No files loaded" }
                }
            }
        };
    }

    let elements = render_nodes(&workspace.children, state, 0);

    rsx! {
        div { class: "sidebar",
            div { class: "sidebar-header", "Explorer" }
            div { class: "file-tree",
                {elements.into_iter()}
            }
        }
    }
}
