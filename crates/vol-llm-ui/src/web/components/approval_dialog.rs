//! HITL approval dialog for tool calls.

use dioxus::prelude::*;

use crate::state::{ConversationEntry, UiEvent};
use crate::web::components::app::AppState;

/// Modal approval dialog shown when a tool call requires HITL approval.
#[component]
pub fn ApprovalDialog() -> Element {
    let state: AppState = use_context();
    let has_pending = state.signal.read().approval_state.has_pending();
    if !has_pending {
        return rsx! {};
    }

    let tool_name = state.signal.read().approval_state.tool_name.clone().unwrap_or_default();
    let reason = state.signal.read().approval_state.reason.clone().unwrap_or_default();
    let arguments = state.signal.read().approval_state.arguments.clone().unwrap_or_default();

    let mut sig_approve = state.signal;
    let on_approve = move |_: Event<MouseData>| {
        sig_approve.with_mut(|s| {
            s.apply(UiEvent::ApprovalResolved { approved: true });
        });
    };

    let mut sig_reject = state.signal;
    let on_reject = move |_: Event<MouseData>| {
        sig_reject.with_mut(|s| {
            s.apply(UiEvent::ApprovalResolved { approved: false });
        });
    };

    let mut sig_stop = state.signal;
    let on_stop = move |_: Event<MouseData>| {
        sig_stop.with_mut(|s| {
            s.apply(UiEvent::ApprovalResolved { approved: false });
            s.is_running = false;
            s.conversation.push(ConversationEntry::Error {
                message: "Agent stopped by user".to_string(),
            });
        });
    };

    rsx! {
        div { class: "modal-overlay",
            div { class: "modal-content",
                div { class: "modal-title", "Tool Approval Required" }
                div { class: "approval-tool-name", "[!] {tool_name}" }
                if !reason.is_empty() {
                    div { class: "approval-reason", "Reason: {reason}" }
                }
                if !arguments.is_empty() {
                    div { class: "approval-args", "{arguments}" }
                }
                div { class: "modal-actions",
                    button { class: "btn-approve", onclick: on_approve, "Approve" }
                    button { class: "btn-reject", onclick: on_reject, "Reject" }
                    button { class: "btn-stop", onclick: on_stop, "Stop" }
                }
            }
        }
    }
}
