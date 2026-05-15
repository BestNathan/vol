use dioxus::prelude::*;
use crate::state::McpState;
use crate::web::components::app::AppState;

#[component]
pub fn ToolCallDialog(mut signal: Signal<McpState>, app_state: AppState) -> Element {
    let rpc_client = app_state.rpc_client.clone();

    let maybe_dialog = {
        let s = signal.read();
        s.tool_call_dialog.as_ref().map(|d| (
            d.server.clone(),
            d.tool_name.clone(),
            d.arguments_json.clone(),
            d.result.clone(),
            d.error.clone(),
            d.loading,
        ))
    };

    let Some((server, tool_name, args, result, error, loading)) = maybe_dialog else {
        return rsx! {};
    };

    rsx! {
        div { class: "fixed inset-0 bg-black/50 flex items-center justify-center z-50",
            div { class: "bg-[#1a1a2e] border border-[#3a3a55] rounded-lg p-4 w-[500px] max-w-[90vw] max-h-[80vh] flex flex-col",
                div { class: "flex items-center justify-between mb-3",
                    div { class: "text-[14px] font-semibold text-[#e0e0e0]", "{server} / {tool_name}" }
                    button {
                        class: "text-[#888] hover:text-[#e0e0e0] text-[18px]",
                        onclick: move |_| { signal.write_unchecked().tool_call_dialog = None; },
                        "x"
                    }
                }
                textarea {
                    class: "w-full h-32 bg-[#252540] border border-[#3a3a55] rounded p-2 text-[12px] text-[#e0e0e0] font-mono resize-none",
                    value: "{args}",
                    oninput: move |ev| {
                        signal.write_unchecked().tool_call_dialog.as_mut().unwrap().arguments_json = ev.value();
                    },
                }
                if !loading {
                    button {
                        class: "mt-2 px-3 py-1 bg-[#4080ff] text-white rounded text-[13px] cursor-pointer hover:bg-[#5090ff]",
                        onclick: move |_| {
                            let s = signal.clone();
                            let client = rpc_client.clone();
                            let (srv, tool, args) = {
                                let r = s.read();
                                let d = r.tool_call_dialog.as_ref().unwrap();
                                (d.server.clone(), d.tool_name.clone(), d.arguments_json.clone())
                            };
                            // Validate JSON first
                            let parsed: serde_json::Value = match serde_json::from_str(&args) {
                                Ok(v) => v,
                                Err(e) => {
                                    s.write_unchecked().tool_call_dialog.as_mut().unwrap().error = Some(format!("Invalid JSON: {e}"));
                                    return;
                                }
                            };
                            let sig = s;
                            sig.write_unchecked().tool_call_dialog.as_mut().unwrap().loading = true;
                            sig.write_unchecked().tool_call_dialog.as_mut().unwrap().error = None;
                            sig.write_unchecked().tool_call_dialog.as_mut().unwrap().result = None;
                            client.mcp_call_tool(&srv, &tool, parsed, move |r| {
                                match r {
                                    Ok(content) => {
                                        sig.write_unchecked().tool_call_dialog.as_mut().unwrap().result = Some(content);
                                    }
                                    Err(e) => {
                                        sig.write_unchecked().tool_call_dialog.as_mut().unwrap().error = Some(e);
                                    }
                                }
                                sig.write_unchecked().tool_call_dialog.as_mut().unwrap().loading = false;
                            });
                        },
                        "Call"
                    }
                } else {
                    div { class: "mt-2 text-[#888] text-[13px]", "Calling..." }
                }
                if let Some(ref result) = result {
                    div { class: "mt-3 bg-[#1a2a1a] border border-[#40c040] rounded p-2 max-h-48 overflow-y-auto",
                        div { class: "text-[11px] text-[#40c040] font-semibold mb-1", "Result" }
                        pre { class: "text-[12px] text-[#e0e0e0] font-mono whitespace-pre-wrap break-all", "{result}" }
                    }
                }
                if let Some(ref error) = error {
                    div { class: "mt-3 bg-[#2a1a1a] border border-[#c04040] rounded p-2",
                        div { class: "text-[11px] text-[#c04040] font-semibold mb-1", "Error" }
                        div { class: "text-[12px] text-[#e0e0e0]", "{error}" }
                    }
                }
            }
        }
    }
}
