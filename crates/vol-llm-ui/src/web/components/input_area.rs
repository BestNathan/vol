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
        rsx! { span { class: "text-[#f0c040]", " Running... (input disabled) " } }
    } else {
        rsx! { span { span { class: "text-[#80a0ff] font-bold", "Enter" } " Send  " span { class: "text-[#80a0ff] font-bold", "Esc" } " Clear" } }
    };

    rsx! {
        div { class: "border-t border-[#333355] p-2.5 bg-[#252540] flex-shrink-0 sm:px-2 sm:py-1.5",
            if has_approval {
                div { p { class: "text-[#f0c040]", "Tool approval pending in the dialog above." } }
            } else {
                div {
                    div { class: "flex gap-2",
                        textarea {
                            value: input_text(),
                            oninput: on_input,
                            disabled: is_running,
                            placeholder: "Type a message to the agent...",
                            rows: 2,
                            class: "flex-1 bg-[#1a1a2e] text-[#e0e0e0] border border-[#444466] rounded-md px-2 py-1.5 text-[14px] font-sans resize-none min-h-[40px] max-h-[120px] outline-none focus:border-[#80a0ff] disabled:opacity-50"
                        }
                        button {
                            onclick: on_submit,
                            disabled: is_running,
                            class: "px-4 py-1.5 bg-[#4060c0] text-[#e0e0e0] rounded-md cursor-pointer text-[14px] self-end hover:bg-[#5070d0] disabled:bg-[#333355] disabled:cursor-not-allowed"
                        }
                    }
                    div { class: "mt-1 text-[11px] text-[#666]", {hint} }
                }
            }
        }
    }
}
