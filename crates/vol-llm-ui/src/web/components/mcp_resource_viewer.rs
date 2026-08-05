use std::cell::RefCell;
use std::rc::Rc;

use super::animated_overlay::AnimatedOverlay;
use crate::state::{McpDialogState, McpResourceViewerState};
use crate::web::components::app::AppState;
use dioxus::prelude::*;

#[component]
pub fn ResourceViewer(mut signal: Signal<McpDialogState>) -> Element {
    let app_state: AppState = use_context();
    let rpc_client = app_state.agent_client();

    // Cache the last-open viewer so the card stays rendered while the
    // overlay plays its exit animation after the state is cleared.
    let cached: Rc<RefCell<Option<McpResourceViewerState>>> =
        use_hook(|| Rc::new(RefCell::new(None)));

    let (open, viewer) = {
        let s = signal.read();
        if let Some(v) = &s.resource_viewer {
            *cached.borrow_mut() = Some(v.clone());
        }
        (s.resource_viewer.is_some(), cached.borrow().clone())
    };

    rsx! {
        AnimatedOverlay {
            open,
            on_close: move |_| { signal.write_unchecked().resource_viewer = None; },
            if let Some(viewer) = viewer {
                div { class: "bg-[#1a1a2e] border border-[#3a3a55] rounded-lg p-4 w-[500px] max-w-[90vw] max-h-[80vh] flex flex-col",
                    div { class: "flex items-center justify-between mb-3",
                        div { class: "text-[14px] font-semibold text-[#e0e0e0] truncate", "Resource: {viewer.uri}" }
                        button {
                            class: "text-[#888] hover:text-[#e0e0e0] text-[18px] ml-2",
                            onclick: move |_| { signal.write_unchecked().resource_viewer = None; },
                            "x"
                        }
                    }
                    if !viewer.loading && viewer.content.is_none() {
                        button {
                            class: "px-3 py-1 bg-[#4080ff] text-white rounded text-[13px] cursor-pointer hover:bg-[#5090ff]",
                            onclick: move |_| {
                                let s = signal.clone();
                                let client = rpc_client.clone();
                                let u = { let r = s.read(); r.resource_viewer.as_ref().unwrap().uri.clone() };
                                let sig = s;
                                sig.write_unchecked().resource_viewer.as_mut().unwrap().loading = true;
                                sig.write_unchecked().resource_viewer.as_mut().unwrap().error = None;
                                client.mcp_read_resource(&u, move |r| {
                                    match r {
                                        Ok(c) => { sig.write_unchecked().resource_viewer.as_mut().unwrap().content = Some(c); }
                                        Err(e) => { sig.write_unchecked().resource_viewer.as_mut().unwrap().error = Some(e); }
                                    }
                                    sig.write_unchecked().resource_viewer.as_mut().unwrap().loading = false;
                                });
                            },
                            "Read"
                        }
                    } else if viewer.loading {
                        div { class: "text-[#888] text-[13px]", "Loading..." }
                    }
                    if let Some(ref content) = viewer.content {
                        div { class: "mt-3 bg-[#252540] border border-[#3a3a55] rounded p-2 max-h-64 overflow-y-auto flex-1",
                            pre { class: "text-[12px] text-[#e0e0e0] font-mono whitespace-pre-wrap break-all", "{content}" }
                        }
                    }
                    if let Some(ref error) = viewer.error {
                        div { class: "mt-3 bg-[#2a1a1a] border border-[#c04040] rounded p-2",
                            div { class: "text-[11px] text-[#c04040] font-semibold mb-1", "Error" }
                            div { class: "text-[12px] text-[#e0e0e0]", "{error}" }
                        }
                    }
                }
            }
        }
    }
}
