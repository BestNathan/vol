//! Status bar showing connection status, build info, and session details.

use dioxus::prelude::*;

use super::nodes_dropdown::NodesDropdown;
use crate::state::{DebugState, GlobalState};
use crate::web::client::NodeListEntry;
use crate::web::components::app::AppState;

const BUILD_TIME: &str = env!("BUILD_TIME");

fn format_elapsed(d: std::time::Duration) -> String {
    let secs = d.as_secs();
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

#[component]
pub fn StatusBar() -> Element {
    let g: Signal<GlobalState> = use_context();
    let debug = use_context::<Signal<DebugState>>();
    let app_state: AppState = use_context();

    // Fetch nodes for the dropdown
    let mut nodes = use_signal(Vec::<NodeListEntry>::new);
    let app = app_state.clone();
    use_effect(move || {
        let cp = app.cp_client.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let (tx, rx) = futures_channel::oneshot::channel();
            cp.node_list(move |result| {
                let _ = tx.send(result);
            });
            if let Ok(Ok(n)) = rx.await {
                nodes.set(n);
            }
        });
    });

    let gs = g.read();

    let elapsed = if gs.is_running() {
        gs.run_start
            .map(|s: web_time::Instant| s.elapsed())
            .unwrap_or_default()
    } else {
        gs.run_elapsed
    };
    let time_str = format_elapsed(elapsed);
    let status = if gs.is_running() { "Running" } else { "Idle" };
    let badge_cls = if gs.is_running() {
        "px-1.5 py-0.5 rounded-[3px] text-[11px] font-bold bg-[#3a3a20] text-[#f0c040]"
    } else {
        "px-1.5 py-0.5 rounded-[3px] text-[11px] font-bold bg-[#203a20] text-[#80c080]"
    };
    let session_id = gs.session_id.clone();
    let run_count = gs.run_count;
    let iteration = gs.iteration;
    let tool_call_count = gs.tool_call_count;
    let is_running = gs.is_running();
    let is_exiting = gs.exiting;
    let unsafe_mode = gs.unsafe_mode;
    let ws_connected = gs.ws_connected;
    let ws_error = gs.ws_last_error.clone();
    let reconnecting = gs.reconnecting;
    let reconnect_delay = gs.reconnect_delay_secs;
    let reconnect_maxed = gs.reconnect_maxed;
    drop(gs);

    let status_cls = if is_running {
        "flex items-center justify-between px-3 py-1 bg-[#2d2d44] text-[#e0e0e0] text-[12px] font-mono flex-shrink-0 text-[#f0c040]"
    } else {
        "flex items-center justify-between px-3 py-1 bg-[#2d2d44] text-[#e0e0e0] text-[12px] font-mono flex-shrink-0 text-[#80c080]"
    };

    rsx! {
        div { class: status_cls,
            div { class: "flex items-center gap-1.5 overflow-hidden flex-nowrap sm:gap-1",
                ConnectionIndicator { connected: ws_connected, error: ws_error.clone(), reconnecting, reconnect_delay, reconnect_maxed }
                span { class: "flex items-center gap-1 mr-1",
                    span { class: "w-2 h-2 rounded-full inline-block flex-shrink-0", style: "background-color: #40c040; box-shadow: 0 0 4px #40c040;" }
                    span { class: "text-[10px] text-[#888]", "CP" }
                }
                if let Some(ref node_id) = *app_state.active_node_id.read() {
                    span { class: "flex items-center gap-1 mr-1",
                        span { class: "w-2 h-2 rounded-full inline-block flex-shrink-0", style: "background-color: #40c040; box-shadow: 0 0 4px #40c040;" }
                        span { class: "text-[10px] text-[#80a0ff]", "DP: {node_id}" }
                    }
                } else {
                    span { class: "flex items-center gap-1 mr-1",
                        span { class: "w-2 h-2 rounded-full inline-block flex-shrink-0", style: "background-color: #666;" }
                        span { class: "text-[10px] text-[#888]", "DP: —" }
                    }
                }
                NodesDropdown {
                    nodes: nodes.read().clone(),
                    selected_node_id: app_state.active_node_id,
                    on_select: {
                        let app_state = app_state.clone();
                        let cp_client = app_state.cp_client.clone();
                        move |node_id: String| {
                            // Fetch agent list to get ws_url for this node
                            let cp = cp_client.clone();
                            let mut dp_pool = app_state.dp_pool;
                            let mut active_node = app_state.active_node_id;
                            let target_node_id = node_id.clone();

                            wasm_bindgen_futures::spawn_local(async move {
                                let (tx, rx) = futures_channel::oneshot::channel();
                                cp.agent_list(move |result| {
                                    let _ = tx.send(result);
                                });

                                if let Ok(Ok(agents)) = rx.await {
                                    // Find first agent on this node with a ws_url
                                    let ws_url = agents.iter()
                                        .find(|a| a.node_id.as_deref() == Some(&target_node_id) && a.ws_url.is_some())
                                        .and_then(|a| a.ws_url.clone());

                                    if let Some(url) = ws_url {
                                        // Create DP connection in the pool
                                        dp_pool.write().get_or_create(&target_node_id, &url, vec![]);
                                        log::info!("Manually selected node {target_node_id} (ws_url={url})");
                                    } else {
                                        log::warn!("No ws_url found for node {target_node_id}");
                                    }
                                } else {
                                    log::warn!("Failed to fetch agent list for node {target_node_id}");
                                }

                                // Set as active node
                                active_node.set(Some(target_node_id));
                            });
                        }
                    },
                    app_state: app_state.clone(),
                }
                span { class: "whitespace-nowrap", "Session: {session_id}" }
                span { class: "hidden sm:inline text-[#555] select-none" }
                span { class: "hidden sm:inline whitespace-nowrap", "Run: {run_count}" }
                span { class: "hidden sm:inline text-[#555] select-none" }
                span { class: "hidden sm:inline whitespace-nowrap", "Iter: {iteration}" }
                span { class: "hidden sm:inline text-[#555] select-none" }
                span { class: "hidden sm:inline whitespace-nowrap", "Tools: {tool_call_count}" }
                span { class: "hidden sm:inline text-[#555] select-none" }
                span { class: "hidden sm:inline whitespace-nowrap", "Time: {time_str}" }
                span { class: "text-[#555] select-none" }
                span { class: badge_cls, "{status}" }
                if unsafe_mode {
                    span { class: "px-1.5 py-0.5 rounded-[3px] text-[11px] font-bold bg-[#3a2020] text-[#ff4040]", "!! UNSAFE" }
                }
                if is_exiting {
                    span { class: "px-1.5 py-0.5 rounded-[3px] text-[11px] font-bold bg-[#3a2020] text-[#ff8080]", "QUITTING" }
                }
            }
            div { class: "hidden sm:flex items-center flex-shrink-0",
                span { class: "flex items-center text-[11px] text-[#888] flex-shrink-0",
                    span { class: "text-[#666]", "UI " }
                    span { class: "text-[#a0a0c0] font-bold", {env!("CARGO_PKG_VERSION")} }
                    span { class: "text-[#555] mx-0.5", " | " }
                    span { class: "text-[#666]", {BUILD_TIME} }
                }
                div { class: "flex-shrink-0 ml-2",
                    button {
                        class: {
                            let d = debug.read();
                            if d.open {
                                "text-[11px] px-1.5 py-0.5 rounded-[3px] font-bold bg-[#2a2a44] text-[#c0c040] hover:bg-[#3a3a55] cursor-pointer"
                            } else {
                                "text-[11px] px-1.5 py-0.5 rounded-[3px] font-bold bg-transparent text-[#555] hover:text-[#888] hover:bg-[#2a2a44] cursor-pointer"
                            }
                        },
                        onclick: move |_| { debug.write_unchecked().toggle(); },
                        title: "Debug Panel",
                        "🐛"
                    }
                }
            }
        }
    }
}

#[component]
fn ConnectionIndicator(
    connected: bool,
    error: Option<String>,
    reconnecting: bool,
    reconnect_delay: u32,
    reconnect_maxed: bool,
) -> Element {
    if reconnect_maxed {
        rsx! {
            span { class: "flex items-center gap-1 mr-1", title: "Connection lost. Please refresh.",
                span { class: "w-2 h-2 rounded-full inline-block flex-shrink-0", style: "background-color: #ff4040;" }
                span { class: "text-[10px] text-[#ff8080]", "No connection" }
            }
        }
    } else if reconnecting {
        rsx! {
            span { class: "flex items-center gap-1 mr-1", title: "Reconnecting...",
                span { class: "w-2 h-2 rounded-full inline-block flex-shrink-0 animate-pulse", style: "background-color: #f0c040;" }
                span { class: "text-[10px] text-[#f0c040]", "Reconnecting... ({reconnect_delay}s)" }
            }
        }
    } else if connected {
        rsx! {
            span { class: "flex items-center gap-1 mr-1", title: "Connected",
                span { class: "w-2 h-2 rounded-full inline-block flex-shrink-0", style: "background-color: #40c040; box-shadow: 0 0 4px #40c040;" }
                span { class: "text-[10px] text-[#888] max-w-[80px] overflow-hidden text-ellipsis whitespace-nowrap", "Connected" }
            }
        }
    } else if let Some(ref err) = error {
        rsx! {
            span { class: "flex items-center gap-1 mr-1", title: "{err}",
                span { class: "w-2 h-2 rounded-full inline-block flex-shrink-0", style: "background-color: #ff4040; animation: conn-blink 1s ease-in-out infinite;" }
                span { class: "text-[10px] text-[#888] max-w-[80px] overflow-hidden text-ellipsis whitespace-nowrap", "Error" }
            }
        }
    } else {
        rsx! {
            span { class: "flex items-center gap-1 mr-1", title: "Connecting...",
                span { class: "w-2 h-2 rounded-full inline-block flex-shrink-0 animate-pulse", style: "background-color: #f0c040;" }
                span { class: "text-[10px] text-[#888] max-w-[80px] overflow-hidden text-ellipsis whitespace-nowrap", "Connecting" }
            }
        }
    }
}
