//! HITL approval dialog for tool calls.

use dioxus::prelude::*;
use crate::state::ApprovalUiState;

#[component]
pub fn ApprovalDialog() -> Element {
    let sig: Signal<ApprovalUiState> = use_context();
    let has_pending = sig.read().has_pending();
    if !has_pending { return rsx! {}; }

    let tool_name = sig.read().tool_name.clone().unwrap_or_default();
    let reason = sig.read().reason.clone().unwrap_or_default();
    let arguments = sig.read().arguments.clone().unwrap_or_default();

    let mut sig_clear = sig;
    let on_approve = move |_: Event<MouseData>| { sig_clear.with_mut(|s| s.clear()); };
    let mut sig_reject = sig;
    let on_reject = move |_: Event<MouseData>| { sig_reject.with_mut(|s| s.clear()); };

    rsx! {
        div { class: "modal-overlay",
            div { class: "modal-content",
                div { class: "modal-title", "Tool Approval Required" }
                div { class: "approval-tool-name", "[!] {tool_name}" }
                if !reason.is_empty() { div { class: "approval-reason", "Reason: {reason}" } }
                if !arguments.is_empty() { div { class: "approval-args", "{arguments}" } }
                div { class: "modal-actions",
                    button { class: "btn-approve", onclick: on_approve, "Approve" }
                    button { class: "btn-reject", onclick: on_reject, "Reject" }
                }
            }
        }
    }
}
