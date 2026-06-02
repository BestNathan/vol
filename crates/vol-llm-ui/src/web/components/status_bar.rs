//! Status bar showing connection status, build info, and session details.

use dioxus::prelude::*;

use crate::state::{DebugState, GlobalState};

const BUILD_TIME: &str = env!("BUILD_TIME");

fn format_elapsed(d: std::time::Duration) -> String {
    let secs = d.as_secs();
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

#[component]
pub fn StatusBar() -> Element {
    let g: Signal<GlobalState> = use_context();
    let debug = use_context::<Signal<DebugState>>();
    let gs = g.read();

    let elapsed = if gs.is_running() {
        gs.run_start.map(|s: web_time::Instant| s.elapsed()).unwrap_or_default()
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
fn ConnectionIndicator(connected: bool, error: Option<String>, reconnecting: bool, reconnect_delay: u32, reconnect_maxed: bool) -> Element {
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
