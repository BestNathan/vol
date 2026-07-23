//! Left sidebar file tree with collapsible directories.

use dioxus::prelude::*;
use std::collections::HashSet;

use crate::state::{ActiveTab, OpenFileTab, WorkspaceState, WorkspaceTreeNode};

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

fn directory_is_collapsed(node: &WorkspaceTreeNode, collapsed_dirs: &HashSet<String>) -> bool {
    collapsed_dirs.contains(&node.path) || (!node.loaded && node.children.is_empty())
}

fn toggle_directory_for_click(
    node: &WorkspaceTreeNode,
    collapsed_dirs: &mut HashSet<String>,
) -> bool {
    if !node.loaded {
        collapsed_dirs.remove(&node.path);
        true
    } else if collapsed_dirs.contains(&node.path) {
        collapsed_dirs.remove(&node.path);
        true
    } else {
        collapsed_dirs.insert(node.path.clone());
        false
    }
}

#[component]
fn TreeNode(node: WorkspaceTreeNode, depth: usize) -> Element {
    if node.is_dir {
        let ws: Signal<WorkspaceState> = use_context();
        let collapsed = directory_is_collapsed(&node, &ws.read().collapsed_dirs);

        let indent_px = depth * 16;
        let chevron_class = if collapsed {
            "w-3 h-3 flex-shrink-0 origin-center transition-transform duration-150"
        } else {
            "w-3 h-3 flex-shrink-0 origin-center transition-transform duration-150 rotate-90"
        };

        let dir_ws = ws;
        let dir_node = node.clone();
        let dir_path = node.path.clone();
        let dir_app = use_context::<crate::web::components::app::AppState>();
        let dir_onclick = move |_: Event<MouseData>| {
            let p = dir_path.clone();
            let mut sig = dir_ws.clone();
            let click_node = dir_node.clone();
            let app_clone = dir_app.clone();
            // Read node_id inside closure (at click time, not render time)
            let nid = app_clone.active_node_id.read().clone();

            let should_load =
                sig.with_mut(|s| toggle_directory_for_click(&click_node, &mut s.collapsed_dirs));

            if should_load {
                // Prefer DP client for the active node, fall back to CP rpc_client
                let client = {
                    if let Some(ref node_id) = nid {
                        app_clone
                            .dp_pool
                            .read()
                            .get(node_id)
                            .map(|c| c.client.clone())
                            .unwrap_or_else(|| app_clone.rpc_client.clone())
                    } else {
                        app_clone.rpc_client.clone()
                    }
                };

                let p_str = p.clone();
                let mut cache = app_clone.node_data_cache;
                let cache_nid = nid.clone();
                let target_nid = nid.clone();
                client.file_list(&p_str, move |result| {
                    // Guard: discard response if user switched nodes
                    let current_nid = app_clone.active_node_id.read().clone();
                    if current_nid != target_nid {
                        log::warn!("Node switched, discarding stale file_list response");
                        return;
                    }
                    let mut sig2 = sig.clone();
                    match result {
                        Ok(entries) => {
                            let flat_entries: Vec<(String, bool)> =
                                entries.into_iter().map(|e| (e.name, e.is_dir)).collect();
                            sig2.with_mut(|s2| {
                                s2.workspace.replace_dir_children(&p, flat_entries);
                                s2.collapsed_dirs.remove(&p);
                            });
                            // Write back to cache for instant switching
                            if let Some(ref node_id) = cache_nid {
                                let ws_read = sig2.read();
                                let tree_value =
                                    serde_json::to_value(&ws_read.workspace).unwrap_or_default();
                                drop(ws_read);
                                let mut c = cache.write();
                                let node_data = c.get_or_insert(node_id);
                                node_data
                                    .data
                                    .insert("workspace_tree".to_string(), tree_value);
                            }
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
        let refresh_app = use_context::<crate::web::components::app::AppState>();
        let refresh_onclick = move |e: Event<MouseData>| {
            e.stop_propagation();
            let p = refresh_path.clone();
            let mut sig = refresh_ws.clone();
            let app_clone = refresh_app.clone();

            // Clear current children to indicate refresh
            sig.with_mut(|s| {
                if let Some(nd) = s.workspace.find_child_mut(&p) {
                    nd.children.clear();
                    nd.loaded = false;
                    nd.load_error = false;
                }
            });

            // Prefer DP client for the active node, fall back to CP rpc_client
            let client = {
                let nid = app_clone.active_node_id.read().clone();
                if let Some(ref node_id) = nid {
                    app_clone
                        .dp_pool
                        .read()
                        .get(node_id)
                        .map(|c| c.client.clone())
                        .unwrap_or_else(|| app_clone.rpc_client.clone())
                } else {
                    app_clone.rpc_client.clone()
                }
            };

            let p_str = p.clone();
            let mut cache = app_clone.node_data_cache;
            let cache_node_id = app_clone.active_node_id.read().clone();
            let target_nid = cache_node_id.clone();
            client.file_list(&p_str, move |result| {
                // Guard: discard response if user switched nodes
                let current_nid = app_clone.active_node_id.read().clone();
                if current_nid != target_nid {
                    log::warn!("Node switched, discarding stale file_list response");
                    return;
                }
                let mut sig2 = sig.clone();
                match result {
                    Ok(entries) => {
                        let flat_entries: Vec<(String, bool)> =
                            entries.into_iter().map(|e| (e.name, e.is_dir)).collect();
                        sig2.with_mut(|s2| {
                            s2.workspace.replace_dir_children(&p, flat_entries);
                        });
                        // Write back to cache for instant switching
                        if let Some(ref nid) = cache_node_id {
                            let ws_read = sig2.read();
                            let tree_value =
                                serde_json::to_value(&ws_read.workspace).unwrap_or_default();
                            drop(ws_read);
                            let mut c = cache.write();
                            let node_data = c.get_or_insert(nid);
                            node_data
                                .data
                                .insert("workspace_tree".to_string(), tree_value);
                        }
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
                    class: "group flex items-center gap-1 py-0.5 pr-1 pl-0 cursor-pointer text-[13px] whitespace-nowrap select-none rounded mx-1 hover:bg-[#2a2a44] active:bg-[#3a3a54]",
                    style: format!("padding-left: {}px;", indent_px),
                    onclick: dir_onclick,
                    span { class: "{chevron_class}",
                        span { class: "block h-1.5 w-1.5 origin-center border-r-2 border-t-2 border-[#8b8baa] rotate-45" }
                    }
                    span { class: "inline-flex items-center justify-center w-[18px] h-[18px] flex-shrink-0 text-[14px]", "{file_icon(true, &node.name)}" }
                    span { class: "min-w-0 flex-1 overflow-hidden text-ellipsis text-[#8ab4ff] font-medium", "{node.name}" }
                    span {
                        class: "ml-auto inline-flex h-5 w-5 flex-shrink-0 items-center justify-center rounded text-[12px] text-[#777799] opacity-0 transition-all duration-150 hover:bg-[#33334f] hover:text-[#e0e0e0] group-hover:opacity-100",
                        onclick: refresh_onclick,
                        "\u{21bb}"
                    }
                }
                if !collapsed {
                    div { class: "overflow-hidden",
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
        let file_node_id = app.active_node_id.read().clone();
        let file_path = node.path.clone();
        let file_onclick = move |_: Event<MouseData>| {
            let p = file_path.clone();
            let mut sig = ws_sig.clone();
            let app_clone = app.clone();
            let nid = file_node_id.clone();

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
                // Prefer DP client for the active node, fall back to CP rpc_client
                let client = {
                    if let Some(ref node_id) = nid {
                        app_clone
                            .dp_pool
                            .read()
                            .get(node_id)
                            .map(|c| c.client.clone())
                            .unwrap_or_else(|| app_clone.rpc_client.clone())
                    } else {
                        app_clone.rpc_client.clone()
                    }
                };
                let p2 = p.clone();
                client.file_read(&p, move |result| {
                    sig.with_mut(|st| {
                        if let Some(idx) = st.open_files.iter().position(|f| f.path == p2) {
                            match result {
                                Ok(c) => {
                                    st.open_files[idx].content = Some(c);
                                }
                                Err(e) => {
                                    st.open_files[idx].error = Some(e);
                                }
                            }
                        }
                    });
                });
            }
            tab.set(ActiveTab::Workspace);
        };

        rsx! {
            div {
                class: "flex items-center gap-1 py-0.5 pr-2 pl-0 cursor-pointer text-[13px] whitespace-nowrap select-none rounded mx-1 hover:bg-[#2a2a44] active:bg-[#3a3a54]",
                style: format!("padding-left: {}px;", indent_px),
                onclick: file_onclick,
                span { class: "inline-flex items-center justify-center w-5 h-5 flex-shrink-0 text-[10px] text-[#666] invisible", "\u{25be}" }
                span { class: "inline-flex items-center justify-center w-[18px] h-[18px] flex-shrink-0 text-[14px]", "{file_icon(false, &node.name)}" }
                span { class: "min-w-0 overflow-hidden text-ellipsis text-[#ccc]", "{node.name}" }
            }
        }
    }
}

/// Build the outer wrapper classes for the file tree panel.
/// On desktop (`sm:`): always visible inline sidebar.
/// On mobile: inline rail when closed, drawer scoped to the main content when open.
fn file_tree_outer_class(drawer_open: bool) -> &'static str {
    if drawer_open {
        "absolute inset-y-0 left-0 z-50 flex w-[80vw] max-w-[300px] flex-col overflow-hidden border-r border-[#2a2a44] bg-[#16162a] sm:static sm:z-auto sm:max-w-none sm:border-r sm:border-[#2a2a44] sm:flex-shrink-0 sm:transform-none sm:transition-none sm:h-full sm:min-h-0 sm:w-[33.33%] md:w-[33.33%] lg:w-[240px] sm:min-w-[160px] md:min-w-[160px] lg:min-w-[180px]"
    } else {
        "flex h-full min-h-0 w-10 flex-col flex-shrink-0 overflow-hidden border-r border-[#2a2a44] bg-[#16162a] sm:w-[33.33%] md:w-[33.33%] lg:w-[240px] sm:min-w-[160px] md:min-w-[160px] lg:min-w-[180px] sm:flex sm:h-full sm:min-h-0 sm:border-r sm:flex-col sm:overflow-hidden sm:flex-shrink-0 sm:bg-[#16162a]"
    }
}

fn file_tree_panel_content_class(drawer_open: bool) -> &'static str {
    if drawer_open {
        "flex min-h-0 flex-1 flex-col"
    } else {
        "hidden min-h-0 flex-1 flex-col sm:flex"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_sidebar_is_a_bounded_flex_column() {
        let class = file_tree_outer_class(false);

        assert!(class.contains("sm:flex"), "{class}");
        assert!(class.contains("sm:flex-col"), "{class}");
        assert!(class.contains("sm:h-full"), "{class}");
        assert!(class.contains("sm:min-h-0"), "{class}");
        assert!(!class.contains("sm:block"), "{class}");
    }

    #[test]
    fn closed_mobile_sidebar_is_an_inline_rail() {
        let class = file_tree_outer_class(false);

        assert!(class.contains("w-10"), "{class}");
        assert!(class.contains("flex-shrink-0"), "{class}");
        assert!(
            !class.split_whitespace().any(|part| part == "hidden"),
            "{class}"
        );
    }

    #[test]
    fn closed_mobile_sidebar_keeps_full_tree_for_desktop_only() {
        let class = file_tree_panel_content_class(false);

        assert!(class.contains("hidden"), "{class}");
        assert!(class.contains("sm:flex"), "{class}");
    }

    #[test]
    fn open_mobile_drawer_is_positioned_inside_main_content() {
        let class = file_tree_outer_class(true);

        assert!(class.contains("absolute"), "{class}");
        assert!(
            !class.split_whitespace().any(|part| part == "fixed"),
            "{class}"
        );
    }

    #[test]
    fn unloaded_empty_directory_is_visually_collapsed_by_default() {
        let node = WorkspaceTreeNode {
            name: "utils".into(),
            path: "src/utils".into(),
            is_dir: true,
            loaded: false,
            load_error: false,
            children: Vec::new(),
        };
        let collapsed_dirs = std::collections::HashSet::new();

        assert!(directory_is_collapsed(&node, &collapsed_dirs));
    }

    #[test]
    fn loaded_directory_uses_explicit_collapsed_state() {
        let node = WorkspaceTreeNode {
            name: "src".into(),
            path: "src".into(),
            is_dir: true,
            loaded: true,
            load_error: false,
            children: Vec::new(),
        };
        let mut collapsed_dirs = std::collections::HashSet::new();

        assert!(!directory_is_collapsed(&node, &collapsed_dirs));

        collapsed_dirs.insert("src".into());
        assert!(directory_is_collapsed(&node, &collapsed_dirs));
    }

    #[test]
    fn unloaded_directory_click_requests_load_without_collapsing() {
        let node = WorkspaceTreeNode {
            name: "utils".into(),
            path: "src/utils".into(),
            is_dir: true,
            loaded: false,
            load_error: false,
            children: Vec::new(),
        };
        let mut collapsed_dirs = std::collections::HashSet::new();

        assert!(toggle_directory_for_click(&node, &mut collapsed_dirs));
        assert!(!collapsed_dirs.contains("src/utils"));
    }

    #[test]
    fn directory_chevron_is_plain_symbol() {
        let source = include_str!("file_tree.rs");
        let old_button_shape = [
            "inline-flex items-center justify-center",
            "w-6 h-6",
            "flex-shrink-0 rounded",
        ]
        .join(" ");

        assert!(source
            .contains("w-3 h-3 flex-shrink-0 origin-center transition-transform duration-150"));
        assert!(!source.contains(&old_button_shape));
    }

    #[test]
    fn directory_chevron_is_drawn_with_css_borders() {
        let source = include_str!("file_tree.rs");

        assert!(source.contains(
            "block h-1.5 w-1.5 origin-center border-r-2 border-t-2 border-[#8b8baa] rotate-45"
        ));
        assert!(!source.contains("class: \"{chevron_class}\", \">\""));
        assert!(!source.contains("class: \"{chevron_class}\", \"\\u{203a}\""));
        assert!(!source.contains("class: \"{chevron_class}\", \"\\u{25be}\""));
    }

    #[test]
    fn open_mobile_drawer_backdrop_is_positioned_inside_main_content() {
        let source = include_str!("file_tree.rs");
        let viewport_backdrop = ["sm:hidden", "fixed", "inset-0", "z-40"].join(" ");

        assert!(!source.contains(&viewport_backdrop));
    }
}

/// File tree component.
#[component]
pub fn FileTree() -> Element {
    let ws: Signal<WorkspaceState> = use_context();

    let app = use_context::<crate::web::components::app::AppState>();
    let drawer_open = ws.read().file_tree_drawer_open;

    // Load root directory from cache (instant) or trigger load via DP.
    // Re-run when the active node changes.
    let loaded_ws_node = use_signal(|| Option::<String>::None);
    use_effect(move || {
        let active_node = app.active_node_id.read().clone();
        let mut cache = app.node_data_cache.clone();
        let dp_pool = app.dp_pool.clone();
        let ws_write = ws;
        let mut loaded_sig = loaded_ws_node;

        // If node changed, clear workspace to trigger proper rebuild
        if active_node != *loaded_sig.read() {
            ws_write.write_unchecked().workspace =
                crate::state::WorkspaceTreeNode::root(".".to_string(), ".".into());
        }

        let ws_loaded = ws_write.read().workspace.loaded;
        let loaded_node = loaded_sig.read().clone();

        // Only act if workspace needs loading or node changed
        if !ws_loaded || loaded_node != active_node {
            if let Some(ref node_id) = active_node {
                // Try cache first for instant rendering
                let cached_files = cache
                    .read()
                    .get(node_id)
                    .and_then(|d| d.data.get("files").cloned());

                if let Some(entries_json) = cached_files {
                    // Cache hit: rebuild workspace from cached entries
                    if let Ok(entries) =
                        serde_json::from_value::<Vec<crate::web::client::FileEntry>>(entries_json)
                    {
                        let flat_entries: Vec<(String, bool)> =
                            entries.into_iter().map(|e| (e.name, e.is_dir)).collect();
                        ws_write
                            .write_unchecked()
                            .workspace
                            .replace_dir_children(".", flat_entries);
                    }
                    loaded_sig.set(Some(node_id.clone()));

                    // Also restore full workspace tree if cached
                    let cached_tree = cache
                        .read()
                        .get(node_id)
                        .and_then(|d| d.data.get("workspace_tree").cloned());
                    if let Some(tree_json) = cached_tree {
                        if let Ok(tree) =
                            serde_json::from_value::<crate::state::WorkspaceTreeNode>(tree_json)
                        {
                            ws_write.write_unchecked().workspace = tree;
                        }
                    }
                } else {
                    // Cache miss: trigger load from DP
                    let dp_client = dp_pool.read().get(node_id).map(|c| c.client.clone());
                    if let Some(client) = dp_client {
                        let nid_clone = node_id.clone();
                        client.file_list(".", move |result| {
                            let mut sig = ws_write.clone();
                            let mut c = cache.write();
                            let node_data = c.get_or_insert(&nid_clone);
                            match result {
                                Ok(entries) => {
                                    let flat_entries: Vec<(String, bool)> = entries
                                        .iter()
                                        .map(|e| (e.name.clone(), e.is_dir))
                                        .collect();
                                    sig.with_mut(|s| {
                                        s.workspace.replace_dir_children(".", flat_entries);
                                    });
                                    // Store root entries for quick rebuild
                                    let entries_value =
                                        serde_json::to_value(&entries).unwrap_or_default();
                                    node_data.data.insert("files".to_string(), entries_value);
                                    // Also store full workspace tree
                                    let ws_read = sig.read();
                                    let tree_value = serde_json::to_value(&ws_read.workspace)
                                        .unwrap_or_default();
                                    drop(ws_read);
                                    node_data
                                        .data
                                        .insert("workspace_tree".to_string(), tree_value);
                                }
                                Err(e) => {
                                    log::warn!(
                                        "Failed to load files for node {}: {}",
                                        nid_clone,
                                        e
                                    );
                                    sig.with_mut(|s| {
                                        s.workspace.loaded = true;
                                        s.workspace.load_error = true;
                                    });
                                }
                            }
                        });
                    }
                    // Mark this node as being loaded (prevents duplicate loads)
                    loaded_sig.set(Some(node_id.clone()));
                }
            } else {
                // No active node
                loaded_sig.set(None);
            }
        }
    });

    let workspace = ws.read().workspace.clone();

    if workspace.children.is_empty() && !workspace.loaded {
        return rsx! {
            div { class: "",
                if drawer_open {
                    div {
                        class: "sm:hidden absolute inset-0 z-40 bg-black/50",
                        onclick: move |_| {
                            let mut w = ws.write_unchecked();
                            w.file_tree_drawer_open = false;
                        },
                    }
                }
                div { class: "{file_tree_outer_class(drawer_open)}",
                    if !drawer_open {
                        button {
                            class: "sm:hidden flex h-full w-full cursor-pointer flex-col items-center gap-2 border-0 bg-transparent px-0 py-3 text-[#8b8baa] hover:bg-[#20203a] hover:text-[#e0e0e0]",
                            onclick: move |_| {
                                let mut w = ws.write_unchecked();
                                w.file_tree_drawer_open = true;
                            },
                            span { class: "text-[16px] leading-none", "\u{1f4c2}" }
                            span { class: "text-[10px] font-semibold uppercase", style: "writing-mode: vertical-rl;", "Files" }
                        }
                    }
                    div { class: "{file_tree_panel_content_class(drawer_open)}",
                        div { class: "px-3 py-2 text-[11px] font-semibold uppercase tracking-[0.8px] text-[#6a6a9a] border-b border-[#2a2a44] flex-shrink-0 flex items-center justify-between",
                            span { "Explorer" }
                            button {
                                class: "sm:hidden text-[#888] hover:text-[#e0e0e0] text-[16px] cursor-pointer",
                                onclick: move |_| {
                                    let mut w = ws.write_unchecked();
                                    w.file_tree_drawer_open = false;
                                },
                                "\u{2715}"
                            }
                        }
                        div { class: "min-h-0 flex-1 overflow-y-auto py-1",
                            div { class: "flex items-center justify-center h-full text-[#666] p-5 text-center text-[12px]",
                                if app.active_node_id.read().is_none() {
                                    "No node selected"
                                } else {
                                    "Loading files..."
                                }
                            }
                        }
                    }
                }
            }
        };
    }

    rsx! {
        div { class: "",
            // Backdrop — mobile only, rendered as a sibling
            if drawer_open {
                div {
                    class: "sm:hidden absolute inset-0 z-40 bg-black/50",
                    onclick: move |_| {
                        let mut w = ws.write_unchecked();
                        w.file_tree_drawer_open = false;
                    },
                }
            }
            // Tree panel
            div { class: "{file_tree_outer_class(drawer_open)}",
                if !drawer_open {
                    button {
                        class: "sm:hidden flex h-full w-full cursor-pointer flex-col items-center gap-2 border-0 bg-transparent px-0 py-3 text-[#8b8baa] hover:bg-[#20203a] hover:text-[#e0e0e0]",
                        onclick: move |_| {
                            let mut w = ws.write_unchecked();
                            w.file_tree_drawer_open = true;
                        },
                        span { class: "text-[16px] leading-none", "\u{1f4c2}" }
                        span { class: "text-[10px] font-semibold uppercase", style: "writing-mode: vertical-rl;", "Files" }
                    }
                }
                div { class: "{file_tree_panel_content_class(drawer_open)}",
                    div { class: "px-3 py-2 text-[11px] font-semibold uppercase tracking-[0.8px] text-[#6a6a9a] border-b border-[#2a2a44] flex-shrink-0 flex items-center justify-between",
                        span { "Explorer" }
                        button {
                            class: "sm:hidden text-[#888] hover:text-[#e0e0e0] text-[16px] cursor-pointer",
                            onclick: move |_| {
                                let mut w = ws.write_unchecked();
                                w.file_tree_drawer_open = false;
                            },
                            "\u{2715}"
                        }
                    }
                    div { class: "min-h-0 flex-1 overflow-y-auto py-1",
                        for child in &workspace.children {
                            TreeNode { node: child.clone(), depth: 0, key: "{child.path}" }
                        }
                    }
                }
            }
        }
    }
}
