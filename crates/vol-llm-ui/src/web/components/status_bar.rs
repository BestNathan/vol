//! Status bar component showing session info, run count, iteration, tool count, elapsed time.

use dioxus::prelude::*;

use crate::web::components::app::AppState;

/// Format a Duration as MM:SS.
fn format_elapsed(d: std::time::Duration) -> String {
    let secs = d.as_secs();
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

/// Status bar displayed at the top of the application.
///
/// Shows: session ID, run count, iteration, tool count, elapsed time,
/// running/idle status, and unsafe mode indicator.
#[component]
pub fn StatusBar() -> Element {
    let state: AppState = use_context();
    let ui = state.ui_state.peek();

    let elapsed = if ui.is_running {
        ui.run_start.map(|s| s.elapsed()).unwrap_or_default()
    } else {
        ui.run_elapsed
    };
    let time_str = format_elapsed(elapsed);
    let status = if ui.is_running { "Running" } else { "Idle" };

    let session_id = ui.session_id.clone();
    let run_count = ui.run_count;
    let iteration = ui.iteration;
    let tool_call_count = ui.tool_call_count;
    let is_running = ui.is_running;
    let is_exiting = ui.exiting;
    let unsafe_mode = ui.unsafe_mode;

    let status_class = if is_running {
        "status-bar status-running"
    } else {
        "status-bar status-idle"
    };

    rsx! {
        div { class: status_class,
            if unsafe_mode {
                span { class: "unsafe-mode", "!! " }
            }
            if is_exiting {
                span { "QUITTING " }
            }
            span { "Session: {session_id}" }
            span { " | Run: {run_count}" }
            span { " | Iter: {iteration}" }
            span { " | Tools: {tool_call_count}" }
            span { " | Time: {time_str}" }
            span { " | {status}" }
        }
    }
}
