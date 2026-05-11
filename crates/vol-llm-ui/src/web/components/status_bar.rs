//! Status bar showing connection status, build info, and session details.

use dioxus::prelude::*;

use crate::state::GlobalState;

const BUILD_TIME: &str = env!("BUILD_TIME");

fn format_elapsed(d: std::time::Duration) -> String {
    let secs = d.as_secs();
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

#[component]
pub fn StatusBar() -> Element {
    let g: Signal<GlobalState> = use_context();
    let gs = g.read();

    let elapsed = if gs.is_running {
        gs.run_start.map(|s: web_time::Instant| s.elapsed()).unwrap_or_default()
    } else {
        gs.run_elapsed
    };
    let time_str = format_elapsed(elapsed);
    let status = if gs.is_running { "Running" } else { "Idle" };
    let badge_cls = if gs.is_running { "status-badge badge-running" } else { "status-badge badge-idle" };
    let session_id = gs.session_id.clone();
    let run_count = gs.run_count;
    let iteration = gs.iteration;
    let tool_call_count = gs.tool_call_count;
    let is_running = gs.is_running;
    let is_exiting = gs.exiting;
    let unsafe_mode = gs.unsafe_mode;
    let ws_connected = gs.ws_connected;
    let ws_error = gs.ws_last_error.clone();
    drop(gs);

    let status_class = if is_running { "status-bar status-running" } else { "status-bar status-idle" };

    rsx! {
        div { class: status_class,
            div { class: "status-left",
                ConnectionIndicator { connected: ws_connected, error: ws_error.clone() }
                span { class: "status-item", "Session: {session_id}" }
                span { class: "status-divider" }
                span { class: "status-item", "Run: {run_count}" }
                span { class: "status-divider" }
                span { class: "status-item", "Iter: {iteration}" }
                span { class: "status-divider" }
                span { class: "status-item", "Tools: {tool_call_count}" }
                span { class: "status-divider" }
                span { class: "status-item", "Time: {time_str}" }
                span { class: "status-divider" }
                span { class: badge_cls, "{status}" }
                if unsafe_mode { span { class: "status-badge badge-unsafe", "!! UNSAFE" } }
                if is_exiting { span { class: "status-badge badge-exiting", "QUITTING" } }
            }
            div { class: "status-right",
                span { class: "build-info",
                    span { class: "build-label", "UI " }
                    span { class: "build-version", {env!("CARGO_PKG_VERSION")} }
                    span { class: "build-separator", " | " }
                    span { class: "build-time", {BUILD_TIME} }
                }
            }
        }
    }
}

#[component]
fn ConnectionIndicator(connected: bool, error: Option<String>) -> Element {
    if connected {
        rsx! {
            span { class: "conn-indicator", title: "Connected",
                span { class: "conn-dot conn-dot-connected", style: "background-color: #40c040;" }
                span { class: "conn-label", "Connected" }
            }
        }
    } else if let Some(ref err) = error {
        rsx! {
            span { class: "conn-indicator", title: "{err}",
                span { class: "conn-dot conn-dot-error", style: "background-color: #ff4040;" }
                span { class: "conn-label", "Error" }
            }
        }
    } else {
        rsx! {
            span { class: "conn-indicator", title: "Connecting...",
                span { class: "conn-dot conn-dot-connecting", style: "background-color: #f0c040;" }
                span { class: "conn-label", "Connecting" }
            }
        }
    }
}
