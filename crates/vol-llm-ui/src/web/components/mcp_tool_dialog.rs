use dioxus::prelude::*;
use crate::state::McpDialogState;
use crate::web::components::app::AppState;
use super::schema_form::SchemaForm;

#[component]
pub fn ToolCallDialog(mut signal: Signal<McpDialogState>) -> Element {
    let app_state: AppState = use_context();
    let rpc_client = app_state.rpc_client.clone();

    let mut form_value: Signal<serde_json::Value> = use_signal(|| serde_json::Value::Object(serde_json::Map::new()));

    let maybe_dialog = {
        let s = signal.read();
        s.tool_call_dialog.as_ref().map(|d| (
            d.server.clone(),
            d.tool_name.clone(),
            d.input_schema.clone(),
        ))
    };

    let Some((server, tool_name, input_schema)) = maybe_dialog else {
        return rsx! {};
    };

    // Re-initialize form when schema changes
    let input_schema_for_effect = input_schema.clone();
    use_effect(move || {
        if let Some(ref schema) = input_schema_for_effect {
            let defaults = build_form_defaults(schema);
            form_value.set(defaults);
        } else {
            form_value.set(serde_json::Value::Object(serde_json::Map::new()));
        }
    });

    let (result, error, loading) = {
        let s = signal.read();
        (
            s.tool_call_dialog.as_ref().and_then(|d| d.result.clone()),
            s.tool_call_dialog.as_ref().and_then(|d| d.error.clone()),
            s.tool_call_dialog.as_ref().map(|d| d.loading).unwrap_or(false),
        )
    };

    rsx! {
        div { class: "fixed inset-0 bg-black/50 flex items-center justify-center z-50",
            div { class: "bg-[#1a1a2e] border border-[#3a3a55] rounded-lg p-4 w-[650px] max-w-[90vw] max-h-[85vh] flex flex-col",
                div { class: "flex items-center justify-between mb-3",
                    div { class: "text-[14px] font-semibold text-[#e0e0e0]", "{server} / {tool_name}" }
                    button {
                        class: "text-[#888] hover:text-[#e0e0e0] text-[18px]",
                        onclick: move |_| { signal.write_unchecked().tool_call_dialog = None; },
                        "x"
                    }
                }
                if let Some(ref schema) = input_schema {
                    SchemaForm { schema: schema.clone(), value: form_value }
                } else {
                    div { class: "text-[#888] text-[12px]", "No parameters required" }
                }
                if !loading {
                    button {
                        class: "mt-2 px-3 py-1 bg-[#4080ff] text-white rounded text-[13px] cursor-pointer hover:bg-[#5090ff]",
                        onclick: move |_| {
                            let s = signal.clone();
                            let client = rpc_client.clone();
                            let (srv, tool) = {
                                let r = s.read();
                                let d = r.tool_call_dialog.as_ref().unwrap();
                                (d.server.clone(), d.tool_name.clone())
                            };
                            let form_json = serde_json::to_string(&*form_value.read()).unwrap_or("{}".to_string());
                            let parsed: serde_json::Value = match serde_json::from_str(&form_json) {
                                Ok(v) => v,
                                Err(e) => {
                                    s.write_unchecked().tool_call_dialog.as_mut().unwrap().error = Some(format!("Invalid form data: {e}"));
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

fn build_form_defaults(schema: &serde_json::Value) -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    let Some(props) = schema.get("properties").and_then(|v| v.as_object()) else {
        return serde_json::Value::Object(obj);
    };
    for (key, prop) in props {
        let default = if let Some(d) = prop.get("default") {
            d.clone()
        } else {
            match prop.get("type").and_then(|t| t.as_str()) {
                Some("string") => serde_json::Value::String(String::new()),
                Some("number") | Some("integer") => serde_json::Value::Number(0.into()),
                Some("boolean") => serde_json::Value::Bool(false),
                Some("object") => build_form_defaults(prop),
                _ => serde_json::Value::Null,
            }
        };
        obj.insert(key.clone(), default);
    }
    serde_json::Value::Object(obj)
}
