//! Left sidebar file tree with collapsible directories.

use dioxus::prelude::*;

use crate::state::{ActiveTab, GlobalState, OpenFileTab, SubscriptionSet, UiEventKind, WorkspaceState, WorkspaceTreeNode};

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

#[component]
fn TreeNode(node: WorkspaceTreeNode, depth: usize) -> Element {
    if node.is_dir {
        let ws: Signal<WorkspaceState> = use_context();
        let collapsed = ws.read().collapsed_dirs.contains(&node.path);

        let indent_px = depth * 16;
        let chevron_cls = if collapsed {
            "file-tree-chevron collapsed"
        } else {
            "file-tree-chevron"
        };

        let dir_ws = ws;
        let dir_path = node.path.clone();
        let rpc = use_context::<crate::web::components::app::AppState>().rpc_client.clone();
        let dir_onclick = move |_: Event<MouseData>| {
            let p = dir_path.clone();
            let rpc_clone = rpc.clone();
            let mut sig = dir_ws.clone();

            let was_collapsed = sig.with_mut(|s| {
                if s.collapsed_dirs.contains(&p) {
                    s.collapsed_dirs.remove(&p);
                    false
                } else {
                    s.collapsed_dirs.insert(p.clone());
                    true
                }
            });

            if !was_collapsed {
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

        let refresh_ws = ws;
        let refresh_path = node.path.clone();
        let refresh_rpc = use_context::<crate::web::components::app::AppState>().rpc_client.clone();
        let refresh_onclick = move |e: Event<MouseData>| {
            e.stop_propagation();
            let p = refresh_path.clone();
            let rpc_clone = refresh_rpc.clone();
            let mut sig = refresh_ws.clone();

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
                        for child in &node.children {
                            TreeNode { node: child.clone(), depth: depth + 1, key: "{child.path}" }
                        }
                    }
                }
            }
        }
    } else {
        let ws: Signal<WorkspaceState> = use_context();
        let app: crate::web::components::app::AppState = use_context();
        let indent_px = depth * 16;

        let ws_sig = ws;
        let mut tab = app.active_tab;
        let rpc = app.rpc_client.clone();
        let file_path = node.path.clone();
        let file_onclick = move |_: Event<MouseData>| {
            let p = file_path.clone();
            let rpc_clone = rpc.clone();
            let mut sig = ws_sig.clone();

            let is_new_file = sig.with_mut(|s| {
                let existing = s.open_files.iter().position(|f| f.path == p);
                match existing {
                    Some(idx) => {
                        s.selected_file_tab = Some(idx);
                        false
                    }
                    None => {
                        let new_idx = s.open_files.len();
                        s.open_files.push(OpenFileTab {
                            path: p.clone(),
                            content: None,
                            error: None,
                        });
                        s.selected_file_tab = Some(new_idx);
                        true
                    }
                }
            });

            if is_new_file {
                let p2 = p.clone();
                rpc_clone.file_read(&p, move |result| {
                    sig.with_mut(|st| {
                        if let Some(idx) = st.open_files.iter().position(|f| f.path == p2) {
                            match result {
                                Ok(c) => { st.open_files[idx].content = Some(c); }
                                Err(e) => { st.open_files[idx].error = Some(e); }
                            }
                        }
                    });
                });
            }
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
    let ws: Signal<WorkspaceState> = use_context();

    let app = use_context::<crate::web::components::app::AppState>();
    let global: Signal<GlobalState> = use_context();

    // Load root directory on mount — but wait for WebSocket connection first.
    use_hook(move || {
        let rpc = app.rpc_client.clone();
        let sig = ws;
        let is_connected = global.read().ws_connected;

        // If already connected, load immediately.
        if is_connected {
            let rpc_clone = rpc.clone();
            let sig2 = sig.clone();
            rpc_clone.file_list(".", move |result| {
                let mut sig3 = sig2.clone();
                match result {
                    Ok(entries) => {
                        let flat_entries: Vec<(String, bool)> = entries
                            .into_iter()
                            .map(|e| (e.name, e.is_dir))
                            .collect();
                        sig3.with_mut(|s2| {
                            s2.workspace.replace_dir_children(".", flat_entries);
                        });
                    }
                    Err(_) => {
                        sig3.with_mut(|s2| {
                            s2.workspace.loaded = true;
                            s2.workspace.load_error = true;
                        });
                    }
                }
            });
        } else {
            // Subscribe to WsConnected event, then load.
            let bus = app.event_bus.clone();
            let mut set = SubscriptionSet::new(bus.clone());
            set.subscribe(&bus, UiEventKind::WsConnected, move |_e| {
                let rpc_clone = rpc.clone();
                let sig2 = sig.clone();
                rpc_clone.file_list(".", move |result| {
                    let mut sig3 = sig2.clone();
                    match result {
                        Ok(entries) => {
                            let flat_entries: Vec<(String, bool)> = entries
                                .into_iter()
                                .map(|e| (e.name, e.is_dir))
                                .collect();
                            sig3.with_mut(|s2| {
                                s2.workspace.replace_dir_children(".", flat_entries);
                            });
                        }
                        Err(_) => {
                            sig3.with_mut(|s2| {
                                s2.workspace.loaded = true;
                                s2.workspace.load_error = true;
                            });
                        }
                    }
                });
            });
            // Keep subscription alive via a static-leaked Arc (Dioxus drops use_hook on unmount).
            // In practice the page never unmounts, so this is fine.
            std::mem::forget(Box::new(set));
        }
    });

    let workspace = ws.read().workspace.clone();

    if workspace.children.is_empty() && !workspace.loaded {
        return rsx! {
            div { class: "sidebar",
                div { class: "sidebar-header", "Explorer" }
                div { class: "file-tree",
                    div { class: "file-tree-loading", "Loading files..." }
                }
            }
        };
    }

    rsx! {
        div { class: "sidebar",
            div { class: "sidebar-header", "Explorer" }
            div { class: "file-tree",
                for child in &workspace.children {
                    TreeNode { node: child.clone(), depth: 0, key: "{child.path}" }
                }
            }
        }
    }
}
