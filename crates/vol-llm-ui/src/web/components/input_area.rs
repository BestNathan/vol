//! Text input for sending messages to the agent.

use dioxus::prelude::*;
use crate::state::{ApprovalUiState, GlobalState};
use crate::web::components::app::AppState;

#[component]
pub fn InputArea() -> Element {
    let app_state: AppState = use_context();
    let global: Signal<GlobalState> = use_context();
    let approval: Signal<ApprovalUiState> = use_context();
    let is_running = global.read().is_running;
    let has_approval = approval.read().has_pending();

    let mut input_text = use_signal(|| String::new());
    let client = app_state.rpc_client.clone();
    let on_submit = move |_| {
        let text = input_text.peek().clone();
        let text = text.trim().to_string();
        if text.is_empty() { return; }
        match client.submit(&text) {
            Ok(req_id) => log::info!("Submitted via JSON-RPC: {}", req_id),
            Err(e) => log::error!("Failed to submit via JSON-RPC: {}", e),
        }
        input_text.set(String::new());
    };
    let on_input = move |evt: Event<FormData>| { input_text.set(evt.value()); };

    let hint = if is_running {
        rsx! { span { class: "input-hint-running", " Running... (input disabled) " } }
    } else {
        rsx! { span { span { class: "input-hint-key", "Enter" } " Send  " span { class: "input-hint-key", "Esc" } " Clear" } }
    };

    rsx! {
        div { class: "input-area",
            if has_approval {
                div { p { class: "input-hint-running", "Tool approval pending in the dialog above." } }
            } else {
                div {
                    div { class: "input-row",
                        textarea { value: input_text(), oninput: on_input, disabled: is_running, placeholder: "Type a message to the agent...", rows: 2 }
                        button { onclick: on_submit, disabled: is_running, "Send" }
                    }
                    div { class: "input-hint", {hint} }
                }
            }
        }
    }
}
