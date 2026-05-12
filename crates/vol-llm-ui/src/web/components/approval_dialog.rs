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
        div { class: "fixed inset-0 bg-black/60 flex items-center justify-center z-[100]",
            div { class: "bg-[#252540] border border-[#444466] rounded-lg p-4 min-w-[400px] max-w-[600px] max-h-[80vh] overflow-y-auto md:min-w-auto md:w-[90vw] md:max-w-[500px]",
                div { class: "text-[16px] font-bold text-[#e0e0e0] mb-3 border-b border-[#333355] pb-2", "Tool Approval Required" }
                div { class: "text-[#f0c040] font-bold text-[15px]", "[!] {tool_name}" }
                if !reason.is_empty() { div { class: "text-[#ccc] my-1.5", "Reason: {reason}" } }
                if !arguments.is_empty() { div { class: "font-mono text-[12px] text-[#888] bg-[#1a1a2e] px-2 py-1.5 rounded-md my-2 max-h-[100px] overflow-y-auto whitespace-pre-wrap", "{arguments}" } }
                div { class: "mt-3 flex gap-2 pt-2 border-t border-[#333355]",
                    button { class: "px-3 py-1.5 border-none rounded-md cursor-pointer text-[13px] bg-[#408040] text-[#e0e0e0]", onclick: on_approve, "Approve" }
                    button { class: "px-3 py-1.5 border-none rounded-md cursor-pointer text-[13px] bg-[#804040] text-[#e0e0e0]", onclick: on_reject, "Reject" }
                }
            }
        }
    }
}
