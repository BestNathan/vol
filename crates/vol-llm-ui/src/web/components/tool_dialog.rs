use dioxus::prelude::*;
use crate::web::components::app::AppState;
use super::schema_form::SchemaForm;

pub struct SystemToolDialogState {
    pub open: bool,
    pub tool_name: String,
    pub description: Option<String>,
    pub parameters: Option<serde_json::Value>,
    pub result: Option<String>,
    pub error: Option<String>,
    pub loading: bool,
}

impl SystemToolDialogState {
    pub fn new() -> Self {
        Self {
            open: false,
            tool_name: String::new(),
            description: None,
            parameters: None,
            result: None,
            error: None,
            loading: false,
        }
    }
}

#[component]
pub fn SystemToolDialog(mut signal: Signal<SystemToolDialogState>) -> Element {
    let app_state: AppState = use_context();
    let rpc_client = app_state.rpc_client.clone();

    let mut form_value: Signal<serde_json::Value> =
        use_signal(|| serde_json::Value::Object(serde_json::Map::new()));

    // Read current dialog state
    let s = signal.read();
    if !s.open {
        return rsx! {};
    }
    let tool_name = s.tool_name.clone();
    let description = s.description.clone();
    let parameters = s.parameters.clone();
    let result = s.result.clone();
    let error = s.error.clone();
    let loading = s.loading;
    drop(s);

    // Initialize form defaults when parameters change
    let params_for_effect = parameters.clone();
    use_effect(move || {
        if let Some(ref schema) = params_for_effect {
            let defaults = build_form_defaults(schema);
            form_value.set(defaults);
        } else {
            form_value.set(serde_json::Value::Object(serde_json::Map::new()));
        }
    });

    rsx! {
        div {
            class: "fixed inset-0 bg-black/50 flex items-center justify-center z-50",
            onclick: move |_| {
                signal.write_unchecked().open = false;
            },
            div {
                class: "w-[95vw] sm:w-[600px] max-h-[85vh] flex flex-col overflow-hidden bg-[#1a1a2e] border border-[#3a3a55] rounded-lg",
                onclick: move |evt: Event<MouseData>| { evt.stop_propagation(); },
                // Header
                div { class: "flex items-center justify-between flex-shrink-0 px-4 pt-3 pb-2 border-b border-[#3a3a55]",
                    div { class: "min-w-0",
                        div { class: "text-[14px] font-semibold text-[#e0e0e0] truncate", "{tool_name}" }
                        if let Some(ref desc) = description {
                            div { class: "text-[11px] text-[#888] truncate mt-0.5", "{desc}" }
                        }
                    }
                    button {
                        class: "text-[#888] hover:text-[#e0e0e0] text-[18px] flex-shrink-0 ml-2",
                        onclick: move |_| { signal.write_unchecked().open = false; },
                        "x"
                    }
                }
                // Content
                div { class: "flex-1 min-h-0 overflow-y-auto px-4 pb-4 space-y-2",
                    // Schema form
                    if let Some(ref schema) = parameters {
                        SchemaForm { schema: schema.clone(), value: form_value }
                    } else {
                        div { class: "text-[#888] text-[12px] mt-2", "No parameters required" }
                    }

                    // Execute button
                    if !loading {
                        button {
                            class: "mt-2 px-3 py-1 bg-[#4080ff] text-white rounded text-[13px] cursor-pointer hover:bg-[#5090ff]",
                            onclick: move |_| {
                                let name = tool_name.clone();
                                let sig = signal.clone();
                                let client = rpc_client.clone();
                                let json_str = serde_json::to_string(&*form_value.read())
                                    .unwrap_or_else(|_| "{}".to_string());
                                let parsed: serde_json::Value = match serde_json::from_str(&json_str) {
                                    Ok(v) => v,
                                    Err(e) => {
                                        sig.write_unchecked().error = Some(format!("Invalid JSON: {e}"));
                                        return;
                                    }
                                };
                                sig.write_unchecked().loading = true;
                                sig.write_unchecked().error = None;
                                sig.write_unchecked().result = None;
                                client.tool_call(&name, &parsed, move |r| {
                                    match r {
                                        Ok(val) => {
                                            let content = val
                                                .get("result")
                                                .and_then(|r| r.get("content"))
                                                .and_then(|c| c.as_str())
                                                .map(|s| s.to_string())
                                                .unwrap_or_else(|| {
                                                    serde_json::to_string_pretty(&val).unwrap_or_else(|_| "(no output)".to_string())
                                                });
                                            sig.write_unchecked().result = Some(content);
                                        }
                                        Err(e) => {
                                            sig.write_unchecked().error = Some(e);
                                        }
                                    }
                                    sig.write_unchecked().loading = false;
                                });
                            },
                            "Execute"
                        }
                    } else {
                        div { class: "mt-2 text-[#888] text-[13px]", "Running..." }
                    }

                    // Result
                    if let Some(ref result_text) = result {
                        div { class: "bg-[#1a2a1a] border border-[#40c040] rounded p-2 mt-2",
                            div { class: "text-[11px] text-[#40c040] font-semibold mb-1", "Result" }
                            pre { class: "text-[12px] text-[#e0e0e0] font-mono whitespace-pre-wrap break-words overflow-x-auto", "{result_text}" }
                        }
                    }

                    // Error
                    if let Some(ref error_text) = error {
                        div { class: "bg-[#2a1a1a] border border-[#c04040] rounded p-2 mt-2",
                            div { class: "text-[11px] text-[#c04040] font-semibold mb-1", "Error" }
                            div { class: "text-[12px] text-[#e0e0e0] break-words", "{error_text}" }
                        }
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
