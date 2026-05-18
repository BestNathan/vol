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
        let chevron_class = if collapsed {
            "inline-flex items-center justify-center w-4 h-4 flex-shrink-0 text-[10px] text-[#666] transition-transform duration-150 -rotate-90"
        } else {
            "inline-flex items-center justify-center w-4 h-4 flex-shrink-0 text-[10px] text-[#666] transition-transform duration-150"
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
                    class: "flex items-center py-0.5 pr-2 pl-0 cursor-pointer text-[13px] whitespace-nowrap select-none rounded-[3px] mx-1 hover:bg-[#2a2a44] active:bg-[#3a3a54]",
                    style: format!("padding-left: {}px;", indent_px),
                    onclick: dir_onclick,
                    span { class: "{chevron_class}", "\u{25be}" }
                    span { class: "inline-flex items-center justify-center w-[18px] h-[18px] flex-shrink-0 mr-1 text-[14px]", "{file_icon(true, &node.name)}" }
                    span { class: "overflow-hidden text-ellipsis text-[#8ab4ff] font-medium", "{node.name}" }
                    span { class: "text-[10px] text-[#666] ml-1 opacity-0 transition-opacity duration-150 cursor-pointer hover:text-[#aaa]", onclick: refresh_onclick, "\u{21bb}" }
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
                class: "flex items-center py-0.5 pr-2 pl-0 cursor-pointer text-[13px] whitespace-nowrap select-none rounded-[3px] mx-1 hover:bg-[#2a2a44] active:bg-[#3a3a54]",
                style: format!("padding-left: {}px;", indent_px),
                onclick: file_onclick,
                span { class: "inline-flex items-center justify-center w-4 h-4 flex-shrink-0 text-[10px] text-[#666] invisible", "\u{25be}" }
                span { class: "inline-flex items-center justify-center w-[18px] h-[18px] flex-shrink-0 mr-1 text-[14px]", "{file_icon(false, &node.name)}" }
                span { class: "overflow-hidden text-ellipsis text-[#ccc]", "{node.name}" }
            }
        }
    }
}

/// CSS classes for the desktop file tree sidebar (mobile: hidden).
const DESKTOP_SIDEBAR_CLASSES: &str = "hidden sm:block sm:w-[33.33%] md:w-[33.33%] lg:w-[240px] sm:min-w-[160px] md:min-w-[160px] lg:min-w-[180px] sm:border-r sm:flex sm:flex-col sm:overflow-hidden sm:flex-shrink-0 sm:bg-[#16162a]";

/// Build the outer wrapper classes for the file tree panel.
/// On desktop (`sm:`): always visible inline sidebar.
/// On mobile: hidden when closed, fixed overlay when open.
fn file_tree_outer_class(drawer_open: bool) -> &'static str {
    if drawer_open {
        "fixed inset-y-0 left-0 z-50 w-[80vw] max-w-[300px] flex flex-col overflow-hidden border-r border-[#2a2a44] bg-[#16162a]"
    } else {
        // Hidden on mobile, visible on desktop with sidebar classes
        DESKTOP_SIDEBAR_CLASSES
    }
}

/// File tree component.
#[component]
pub fn FileTree() -> Element {
    let ws: Signal<WorkspaceState> = use_context();

    let app = use_context::<crate::web::components::app::AppState>();
    let global: Signal<GlobalState> = use_context();
    let drawer_open = ws.read().file_tree_drawer_open;

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
            div { class: "",
                div { class: "{file_tree_outer_class(drawer_open)}",
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
                    div { class: "flex-1 overflow-y-auto py-1",
                        div { class: "flex items-center justify-center h-full text-[#666] p-5 text-center text-[12px]", "Loading files..." }
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
                    class: "sm:hidden fixed inset-0 z-40 bg-black/50",
                    onclick: move |_| {
                        let mut w = ws.write_unchecked();
                        w.file_tree_drawer_open = false;
                    },
                }
            }
            // Tree panel
            div { class: "{file_tree_outer_class(drawer_open)}",
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
                div { class: "flex-1 overflow-y-auto py-1",
                    for child in &workspace.children {
                        TreeNode { node: child.clone(), depth: 0, key: "{child.path}" }
                    }
                }
            }
        }
    }
}
