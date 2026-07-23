//! Nodes dropdown — collapsible node selector for the status bar.

use dioxus::prelude::*;

use crate::web::client::NodeListEntry;
use crate::web::components::app::AppState;

/// Collapsible dropdown showing available DP nodes.
///
/// Displays a "▾ Nodes(N)" button when collapsed. When open, shows a list of
/// nodes with status indicator, name, version, and load info. Clicking a node
/// row calls `on_select` with the node_id. Clicking the node name (with
/// stop_propagation) enters Node Detail UI by setting `app_state.viewing_node_detail`.
#[component]
pub fn NodesDropdown(
    nodes: Vec<NodeListEntry>,
    selected_node_id: Signal<Option<String>>,
    on_select: EventHandler<String>,
    app_state: AppState,
) -> Element {
    let mut is_open = use_signal(|| false);

    let selected_id = selected_node_id.read().clone();
    let node_count = nodes.len();

    rsx! {
        div { class: "relative inline-block",
            button {
                class: "flex items-center gap-1 px-2 py-0.5 text-[11px] rounded hover:bg-[#3a3a55] cursor-pointer text-[#e0e0e0] bg-transparent border-none",
                onclick: move |e| {
                    e.stop_propagation();
                    is_open.toggle();
                },
                span { "▾ Nodes({node_count})" }
            }
            if *is_open.read() {
                // Transparent overlay — clicking outside the dropdown closes it.
                // Follows the same pattern as SkillDetailDialog.
                div {
                    class: "fixed inset-0 z-40",
                    onclick: move |_| is_open.set(false),
                }
                div {
                    class: "absolute right-0 mt-1 min-w-[280px] bg-[#1e1e36] border border-[#333355] rounded shadow-lg z-50 max-h-[400px] overflow-y-auto",
                    onclick: move |e| {
                        // Prevent clicks inside dropdown from closing it
                        e.stop_propagation();
                    },
                    if nodes.is_empty() {
                        div { class: "px-3 py-2 text-[#888] text-xs", "No nodes available" }
                    } else {
                        for node in nodes.iter() {
                            NodeRow {
                                key: "{node.node_id}",
                                node: node.clone(),
                                is_selected: selected_id.as_deref() == Some(&node.node_id),
                                on_select: {
                                    let node_id = node.node_id.clone();
                                    let on_select = on_select.clone();
                                    move |_| {
                                        on_select.call(node_id.clone());
                                        is_open.set(false);
                                    }
                                },
                                on_view_detail: {
                                    let node_id = node.node_id.clone();
                                    let mut app_state = app_state.clone();
                                    move |_| {
                                        app_state.viewing_node_detail.set(Some(node_id.clone()));
                                        is_open.set(false);
                                    }
                                },
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Individual node row in the dropdown.
///
/// Shows status indicator, node name, node_id, version, and load info.
/// Clicking the row calls `on_select`. Clicking the name area (with
/// stop_propagation) calls `on_view_detail` to enter Node Detail UI.
#[component]
fn NodeRow(
    node: NodeListEntry,
    is_selected: bool,
    on_select: EventHandler<()>,
    on_view_detail: EventHandler<()>,
) -> Element {
    let status_color = if node.status == "online" {
        "bg-green-500"
    } else {
        "bg-red-500"
    };

    let status_glow = if node.status == "online" {
        "box-shadow: 0 0 4px #40c040;"
    } else {
        "box-shadow: 0 0 4px #ff4040;"
    };

    let row_bg = if is_selected {
        "bg-[#2a2a55]"
    } else {
        "hover:bg-[#2a2a44]"
    };

    rsx! {
        div {
            class: "flex items-center gap-2 px-3 py-2 cursor-pointer border-b border-[#333355] last:border-b-0 {row_bg}",
            onclick: move |_| on_select.call(()),
            // Status indicator
            div {
                class: "w-2 h-2 rounded-full {status_color} flex-shrink-0",
                style: status_glow,
            }
            // Node info
            div { class: "flex-1 min-w-0",
                div {
                    class: "flex items-center gap-2",
                    // Node name (clickable for detail view)
                    span {
                        class: "text-[#e0e0e0] text-sm font-medium truncate cursor-pointer hover:text-[#80a0ff]",
                        onclick: move |e| {
                            e.stop_propagation();
                            on_view_detail.call(());
                        },
                        title: "Click to view node detail",
                        "{node.name}"
                    }
                    // Selected indicator
                    if is_selected {
                        span { class: "text-[#80c080] text-xs flex-shrink-0", "✓" }
                    }
                }
                div { class: "text-[#888] text-xs truncate",
                    "{node.node_id} · v{node.version}"
                }
            }
            // Load info
            div { class: "flex-shrink-0 text-right",
                div { class: "text-[#888] text-xs",
                    "R:{node.load.running} Q:{node.load.queued}"
                }
                if let Some(count) = node.agent_count {
                    div { class: "text-[#666] text-xs", "{count} agents" }
                }
            }
        }
    }
}
