//! Text input for sending messages to the agent.

use dioxus::prelude::*;

use crate::state::UiEvent;
use crate::web::components::app::AppState;

/// Input area with text field and send button.
///
/// When the agent is running, the input is disabled.
/// On submit (Enter or button click), it sends an `AgentStart` event.
#[component]
pub fn InputArea() -> Element {
    let state: AppState = use_context();
    let mut input_text = use_signal(|| String::new());
    let is_running = state.ui_state.peek().is_running;
    let has_approval = state.ui_state.peek().approval_state.has_pending();

    let state_clone = state.clone();
    let on_submit = move |_| {
        let text = input_text.peek().clone();
        let text = text.trim().to_string();
        if text.is_empty() {
            return;
        }
        state_clone.apply_event(UiEvent::AgentStart { input: text.clone() });
        input_text.set(String::new());
        log::info!("Submitted: {}", text);
    };

    let on_input = move |evt: Event<FormData>| {
        let value = evt.value().clone();
        input_text.set(value);
    };

    let hint_content = if is_running {
        rsx! {
            span { class: "input-hint-running", " Running... (input disabled) " }
        }
    } else {
        rsx! {
            span {
                span { class: "input-hint-key", "Enter" }
                " Send  "
                span { class: "input-hint-key", "Esc" }
                " Clear"
            }
        }
    };

    rsx! {
        div { class: "input-area",
            if has_approval {
                div {
                    p { class: "input-hint-running", "Tool approval pending in the dialog above." }
                }
            } else {
                div {
                    div { class: "input-row",
                        textarea {
                            value: "{input_text}",
                            oninput: on_input,
                            disabled: is_running,
                            placeholder: "Type a message to the agent...",
                            rows: 2,
                        }
                        button {
                            onclick: on_submit,
                            disabled: is_running,
                            "Send"
                        }
                    }
                    div { class: "input-hint",
                        {hint_content}
                    }
                }
            }
        }
    }
}
