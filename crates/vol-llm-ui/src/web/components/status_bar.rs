//! Status bar showing connection status, build info, and session details.

use dioxus::prelude::*;

use crate::web::components::app::AppState;

/// Frontend build timestamp, set at compile time.
const BUILD_TIME: &str = env!("BUILD_TIME");

/// Format a Duration as MM:SS.
fn format_elapsed(d: std::time::Duration) -> String {
    let secs = d.as_secs();
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

/// Status bar with connection indicator, build info, and session stats.
#[component]
pub fn StatusBar() -> Element {
    let state: AppState = use_context();
    let ui = state.signal.read();

    let elapsed = if ui.is_running {
        ui.run_start.map(|s: web_time::Instant| s.elapsed()).unwrap_or_default()
    } else {
        ui.run_elapsed
    };
    let time_str = format_elapsed(elapsed);
    let status = if ui.is_running { "Running" } else { "Idle" };
    let badge_cls = if ui.is_running {
        "status-badge badge-running"
    } else {
        "status-badge badge-idle"
    };

    let session_id = ui.session_id.clone();
    let run_count = ui.run_count;
    let iteration = ui.iteration;
    let tool_call_count = ui.tool_call_count;
    let is_running = ui.is_running;
    let is_exiting = ui.exiting;
    let unsafe_mode = ui.unsafe_mode;

    let ws_connected = ui.ws_connected;
    let ws_error = ui.ws_last_error.clone();
    drop(ui);

    let status_class = if is_running {
        "status-bar status-running"
    } else {
        "status-bar status-idle"
    };

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
                if unsafe_mode {
                    span { class: "status-badge badge-unsafe", "!! UNSAFE" }
                }
                if is_exiting {
                    span { class: "status-badge badge-exiting", "QUITTING" }
                }
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

/// Connection status indicator with animated dot.
#[component]
fn ConnectionIndicator(connected: bool, error: Option<String>) -> Element {
    if connected {
        rsx! {
            span {
                class: "conn-indicator",
                title: "Connected",
                span { class: "conn-dot conn-dot-connected", style: "background-color: #40c040;" }
                span { class: "conn-label", "Connected" }
            }
        }
    } else if let Some(ref err) = error {
        let err_text = err.clone();
        rsx! {
            span {
                class: "conn-indicator",
                title: "{err_text}",
                span { class: "conn-dot conn-dot-error", style: "background-color: #ff4040;" }
                span { class: "conn-label", "Error" }
            }
        }
    } else {
        rsx! {
            span {
                class: "conn-indicator",
                title: "Connecting...",
                span { class: "conn-dot conn-dot-connecting", style: "background-color: #f0c040;" }
                span { class: "conn-label", "Connecting" }
            }
        }
    }
}
