use dioxus::prelude::*;
use crate::state::McpDialogState;
use crate::web::components::app::AppState;

#[component]
pub fn PromptViewer(mut signal: Signal<McpDialogState>) -> Element {
    let app_state: AppState = use_context();
    let _rpc_client = app_state.rpc_client.clone();

    let viewer = {
        let s = signal.read();
        s.prompt_viewer.clone()
    };
    let Some(viewer) = viewer else {
        return rsx! {};
    };

    rsx! {
        div { class: "fixed inset-0 bg-black/50 flex items-center justify-center z-50",
            div { class: "bg-[#1a1a2e] border border-[#3a3a55] rounded-lg p-4 w-[500px] max-w-[90vw] max-h-[80vh] flex flex-col",
                div { class: "flex items-center justify-between mb-3",
                    div { class: "text-[14px] font-semibold text-[#e0e0e0]", "Prompt: {viewer.prompt_name}" }
                    button {
                        class: "text-[#888] hover:text-[#e0e0e0] text-[18px]",
                        onclick: move |_| { signal.write_unchecked().prompt_viewer = None; },
                        "x"
                    }
                }
                div { class: "text-[12px] text-[#888]", "Server: {viewer.server}" }
                textarea {
                    class: "w-full h-24 bg-[#252540] border border-[#3a3a55] rounded p-2 text-[12px] text-[#e0e0e0] font-mono resize-none mt-2",
                    value: "{viewer.args_json}",
                    oninput: move |ev| {
                        signal.write_unchecked().prompt_viewer.as_mut().unwrap().args_json = ev.value();
                    },
                }
                if !viewer.loading {
                    button {
                        class: "mt-2 px-3 py-1 bg-[#4080ff] text-white rounded text-[13px] cursor-pointer hover:bg-[#5090ff]",
                        onclick: move |_| {
                            signal.write_unchecked().prompt_viewer.as_mut().unwrap().error = Some("mcp.get_prompt not implemented yet".to_string());
                        },
                        "Get"
                    }
                } else {
                    div { class: "mt-2 text-[#888] text-[13px]", "Loading..." }
                }
                if let Some(ref result) = viewer.result {
                    div { class: "mt-3 bg-[#1a2a1a] border border-[#40c040] rounded p-2 max-h-48 overflow-y-auto",
                        pre { class: "text-[12px] text-[#e0e0e0] font-mono whitespace-pre-wrap break-all", "{result}" }
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
